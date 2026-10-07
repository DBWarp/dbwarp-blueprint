use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const APPLE_FRAMEWORKS: &str =
    "-F/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/System/Library/Frameworks";

#[derive(Clone, Copy)]
enum Gssapi {
    Mit,
    Heimdal,
    Apple,
}

fn emit_link_line(implementation: &Gssapi) {
    match implementation {
        Gssapi::Mit => println!("cargo:rustc-link-lib=gssapi_krb5"),
        Gssapi::Heimdal => println!("cargo:rustc-link-lib=gssapi"),
        Gssapi::Apple => println!("cargo:rustc-link-lib=framework=GSS"),
    }
}

fn user_prefixes() -> Vec<PathBuf> {
    match env::var("LIBGSSAPI_PREFIX") {
        Err(_) => Vec::new(),
        Ok(value) => value
            .split(':')
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .collect(),
    }
}

fn krb5_config_prefix() -> Option<PathBuf> {
    Command::new("krb5-config")
        .arg("gssapi")
        .arg("--prefix")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| PathBuf::from(value.trim()))
        .filter(|path| !path.as_os_str().is_empty())
}

fn dir_has_lib(dir: &Path, stem: &str) -> bool {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };
    entries.flatten().any(|entry| {
        match entry.file_name().to_string_lossy().strip_prefix(stem) {
            Some(rest) => rest.starts_with(".so"),
            None => false,
        }
    })
}

fn builder_from_pkgconfig(lib: pkg_config::Library) -> bindgen::Builder {
    bindgen::Builder::default().clang_args(
        lib.include_paths
            .iter()
            .map(|path| format!("-I{}", path.to_string_lossy())),
    )
}

fn searched_builder(implementation: &Gssapi) -> bindgen::Builder {
    let builder = bindgen::Builder::default();
    match implementation {
        Gssapi::Mit | Gssapi::Heimdal => {
            let builder = user_prefixes().iter().fold(builder, |builder, prefix| {
                builder.clang_arg(format!("-I{}", prefix.join("include").display()))
            });
            match env::var("NIX_CFLAGS_COMPILE") {
                Err(_) => builder,
                Ok(flags) => builder.clang_args(flags.split(' ')),
            }
        }
        Gssapi::Apple => builder.clang_arg(APPLE_FRAMEWORKS),
    }
}

fn emit_user_prefix_link_search() {
    for prefix in user_prefixes() {
        println!(
            "cargo:rustc-link-search=native={}",
            prefix.join("lib").display()
        );
    }
}

fn probe_pkgconfig(name: &str, emit_link_metadata: bool) -> Option<pkg_config::Library> {
    pkg_config::Config::new()
        .cargo_metadata(emit_link_metadata)
        .probe(name)
        .ok()
}

fn try_pkgconfig(emit_link_metadata: bool) -> Option<(Gssapi, bindgen::Builder)> {
    if let Some(lib) = probe_pkgconfig("mit-krb5-gssapi", emit_link_metadata) {
        Some((Gssapi::Mit, builder_from_pkgconfig(lib)))
    } else {
        probe_pkgconfig("heimdal-gssapi", emit_link_metadata)
            .map(|lib| (Gssapi::Heimdal, builder_from_pkgconfig(lib)))
    }
}

fn forced_impl() -> Option<Gssapi> {
    let value = env::var("LIBGSSAPI_IMPL").ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "mit" => Some(Gssapi::Mit),
        "heimdal" => Some(Gssapi::Heimdal),
        "apple" => Some(Gssapi::Apple),
        other => panic!(
            "LIBGSSAPI_IMPL must be one of \"mit\", \"heimdal\", \"apple\"; got {:?}",
            other
        ),
    }
}

fn pkgconfig_name(implementation: Gssapi) -> Option<&'static str> {
    match implementation {
        Gssapi::Mit => Some("mit-krb5-gssapi"),
        Gssapi::Heimdal => Some("heimdal-gssapi"),
        Gssapi::Apple => None,
    }
}

fn select_forced(
    implementation: Gssapi,
    cross_compile: bool,
) -> (Gssapi, bindgen::Builder) {
    if !cross_compile {
        if let Some(name) = pkgconfig_name(implementation) {
            if let Some(lib) = probe_pkgconfig(name, true) {
                return (implementation, builder_from_pkgconfig(lib));
            }
        }
    }
    emit_link_line(&implementation);
    emit_user_prefix_link_search();
    (implementation, searched_builder(&implementation))
}

fn which() -> Gssapi {
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("target OS");
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").expect("target family");

    if target_os == "macos" {
        emit_link_line(&Gssapi::Apple);
        return Gssapi::Apple;
    } else if target_os == "windows" {
        panic!("use SSPI on windows")
    } else if target_family != "unix" {
        panic!("libgssapi isn't ported to this platform yet")
    }

    emit_user_prefix_link_search();
    let mut lib_dirs: Vec<PathBuf> = user_prefixes()
        .into_iter()
        .map(|prefix| prefix.join("lib"))
        .collect();
    if let Some(prefix) = krb5_config_prefix() {
        lib_dirs.push(prefix.join("lib"));
    }
    if let Ok(path) = env::var("LD_LIBRARY_PATH") {
        lib_dirs.extend(
            path.split(':')
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
        );
    }
    lib_dirs.extend(["/lib", "/lib64", "/usr/lib", "/usr/lib64"].map(PathBuf::from));

    for dir in &lib_dirs {
        if dir_has_lib(dir, "libgssapi_krb5") {
            println!("cargo:rustc-link-search=native={}", dir.display());
            emit_link_line(&Gssapi::Mit);
            return Gssapi::Mit;
        }
        if dir_has_lib(dir, "libgssapi") {
            println!("cargo:rustc-link-search=native={}", dir.display());
            emit_link_line(&Gssapi::Heimdal);
            return Gssapi::Heimdal;
        }
    }
    panic!(
        "no MIT or Heimdal gssapi library found. Set LIBGSSAPI_IMPL (\"mit\" or \
         \"heimdal\") to pick an implementation, and/or LIBGSSAPI_PREFIX to the \
         install prefix(es) to search, colon-separated"
    );
}

fn select_linux_headers(cross_compile: bool) -> (Gssapi, bindgen::Builder) {
    if let Some(implementation) = forced_impl() {
        assert!(
            !matches!(implementation, Gssapi::Apple),
            "LIBGSSAPI_IMPL=apple is not valid for Linux"
        );
        if !cross_compile {
            if let Some(name) = pkgconfig_name(implementation) {
                if let Some(lib) = probe_pkgconfig(name, false) {
                    return (implementation, builder_from_pkgconfig(lib));
                }
            }
        }
        return (implementation, searched_builder(&implementation));
    }

    if !cross_compile {
        if let Some(found) = try_pkgconfig(false) {
            return found;
        }
    }
    (Gssapi::Mit, searched_builder(&Gssapi::Mit))
}

fn main() {
    println!("cargo:rerun-if-env-changed=LIBGSSAPI_IMPL");
    println!("cargo:rerun-if-env-changed=LIBGSSAPI_PREFIX");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/consts.h");
    println!("cargo:rerun-if-changed=src/wrapper_mit.h");
    println!("cargo:rerun-if-changed=src/wrapper_heimdal.h");
    println!("cargo:rerun-if-changed=src/wrapper_apple.h");
    println!("cargo:rerun-if-changed=src/lazy_gssapi.c");

    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("target OS");
    let cross_compile = env::var("HOST").expect("host") != env::var("TARGET").expect("target");

    let (implementation, builder) = if target_os == "linux" {
        let selected = select_linux_headers(cross_compile);
        let mut shim = cc::Build::new();
        shim.file("src/lazy_gssapi.c");
        if !cross_compile {
            if let Some(name) = pkgconfig_name(selected.0) {
                if let Some(lib) = probe_pkgconfig(name, false) {
                    for path in lib.include_paths {
                        shim.include(path);
                    }
                }
            }
        }
        for prefix in user_prefixes() {
            shim.include(prefix.join("include"));
        }
        shim.flag_if_supported("-std=c11");
        shim.compile("gssapi_lazy");
        println!("cargo:rustc-link-lib=dl");
        selected
    } else {
        match forced_impl() {
            Some(implementation) => select_forced(implementation, cross_compile),
            None => match (cross_compile, try_pkgconfig(true)) {
                (false, Some(found)) => found,
                _ => {
                    let implementation = which();
                    (implementation, searched_builder(&implementation))
                }
            },
        }
    };

    let bindings = builder
        .allowlist_type("(OM_.+|gss_.+)")
        .allowlist_var("_?GSS_.+|gss_.+")
        .allowlist_function("gss_.*")
        .header(match implementation {
            Gssapi::Mit => "src/wrapper_mit.h",
            Gssapi::Heimdal => "src/wrapper_heimdal.h",
            Gssapi::Apple => "src/wrapper_apple.h",
        })
        .generate()
        .expect("failed to generate GSSAPI bindings");
    let output = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    bindings
        .write_to_file(output.join("bindings.rs"))
        .expect("failed to write GSSAPI bindings");
}
