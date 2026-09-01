# Third-party notices

This release's all-feature Cargo graph contains 399 packages: 2 first-party DBWarp crates and 397 third-party crates. Every third-party name, version, declared licence, and copied notice file is recorded in `third-party-licenses/MANIFEST.json`. The complete copied texts are distributed in `third-party-licenses/crates/`.

## Required and modified components

### ring

The `ring` distribution contains ISC code, BoringSSL-derived code, once_cell-derived code, and Fiat Cryptography code. Its top-level licence and every licence file it references are reproduced under `third-party-licenses/crates/ring-0.17.14/`.

### mysql_async

DBWarp Blueprint uses a modified `mysql_async` 0.34.2. The Apache-2.0 and MIT texts and the required modification notice are in `vendor/mysql_async/`; the same licence texts are also present in the generated third-party bundle.

### mimalloc

The binary links the mimalloc allocator through the `mimalloc` and `libmimalloc-sys` Rust crates. Their package and embedded native-source licence files are included in the generated third-party bundle.

## DM Sans

Generated PowerPoint decks embed DM Sans static font faces. Copyright 2014 The DM Sans Project Authors. DM Sans is distributed under the SIL Open Font License, Version 1.1; see `assets/fonts/dm-sans/OFL.txt`. No Reserved Font Name is declared for these bundled files.

## Locked Cargo inventory

| Crate | Version | Declared licence |
|---|---:|---|
| `adler2` | `2.0.1` | `0BSD OR MIT OR Apache-2.0` |
| `ahash` | `0.8.12` | `MIT OR Apache-2.0` |
| `aho-corasick` | `1.1.4` | `Unlicense OR MIT` |
| `alloc-no-stdlib` | `2.0.4` | `BSD-3-Clause` |
| `alloc-stdlib` | `0.2.4` | `BSD-3-Clause` |
| `allocator-api2` | `0.2.21` | `MIT OR Apache-2.0` |
| `android_system_properties` | `0.1.5` | `MIT/Apache-2.0` |
| `ansi_term` | `0.12.1` | `MIT` |
| `anstream` | `1.0.0` | `MIT OR Apache-2.0` |
| `anstyle` | `1.0.14` | `MIT OR Apache-2.0` |
| `anstyle-parse` | `1.0.0` | `MIT OR Apache-2.0` |
| `anstyle-query` | `1.1.5` | `MIT OR Apache-2.0` |
| `anstyle-wincon` | `3.0.11` | `MIT OR Apache-2.0` |
| `anyhow` | `1.0.102` | `MIT OR Apache-2.0` |
| `apache-avro` | `0.21.0` | `Apache-2.0` |
| `arrow-array` | `58.3.0` | `Apache-2.0 AND MIT` |
| `arrow-buffer` | `58.3.0` | `Apache-2.0` |
| `arrow-data` | `58.3.0` | `Apache-2.0` |
| `arrow-ipc` | `58.3.0` | `Apache-2.0` |
| `arrow-schema` | `58.3.0` | `Apache-2.0` |
| `arrow-select` | `58.3.0` | `Apache-2.0` |
| `async-trait` | `0.1.89` | `MIT OR Apache-2.0` |
| `asynchronous-codec` | `0.6.2` | `MIT` |
| `atty` | `0.2.14` | `MIT` |
| `autocfg` | `1.5.0` | `Apache-2.0 OR MIT` |
| `aws-lc-rs` | `1.16.3` | `ISC AND (Apache-2.0 OR ISC)` |
| `aws-lc-sys` | `0.40.0` | `ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0)` |
| `base64` | `0.21.7` | `MIT OR Apache-2.0` |
| `base64` | `0.22.1` | `MIT OR Apache-2.0` |
| `base64ct` | `1.8.3` | `Apache-2.0 OR MIT` |
| `bigdecimal` | `0.4.10` | `MIT/Apache-2.0` |
| `bindgen` | `0.59.2` | `BSD-3-Clause` |
| `bindgen` | `0.72.1` | `BSD-3-Clause` |
| `bitflags` | `1.3.2` | `MIT/Apache-2.0` |
| `bitflags` | `2.11.1` | `MIT OR Apache-2.0` |
| `block-buffer` | `0.10.4` | `MIT OR Apache-2.0` |
| `block-buffer` | `0.12.0` | `MIT OR Apache-2.0` |
| `bon` | `3.9.3` | `MIT OR Apache-2.0` |
| `bon-macros` | `3.9.3` | `MIT OR Apache-2.0` |
| `brotli` | `8.0.4` | `BSD-3-Clause AND MIT` |
| `brotli-decompressor` | `5.0.3` | `BSD-3-Clause/MIT` |
| `btoi` | `0.4.3` | `MIT OR Apache-2.0` |
| `bumpalo` | `3.20.2` | `MIT OR Apache-2.0` |
| `byteorder` | `1.5.0` | `Unlicense OR MIT` |
| `bytes` | `1.11.1` | `MIT` |
| `cc` | `1.2.61` | `MIT OR Apache-2.0` |
| `cexpr` | `0.6.0` | `Apache-2.0/MIT` |
| `cfg-if` | `1.0.4` | `MIT OR Apache-2.0` |
| `chacha20` | `0.10.0` | `MIT OR Apache-2.0` |
| `chrono` | `0.4.44` | `MIT OR Apache-2.0` |
| `clang-sys` | `1.8.1` | `Apache-2.0` |
| `clap` | `2.34.0` | `MIT` |
| `clap` | `4.6.1` | `MIT OR Apache-2.0` |
| `clap_builder` | `4.6.0` | `MIT OR Apache-2.0` |
| `clap_derive` | `4.6.1` | `MIT OR Apache-2.0` |
| `clap_lex` | `1.1.0` | `MIT OR Apache-2.0` |
| `cmake` | `0.1.58` | `MIT OR Apache-2.0` |
| `cmov` | `0.5.3` | `Apache-2.0 OR MIT` |
| `colorchoice` | `1.0.5` | `MIT OR Apache-2.0` |
| `connection-string` | `0.2.0` | `MIT OR Apache-2.0` |
| `const-oid` | `0.10.2` | `Apache-2.0 OR MIT` |
| `const-oid` | `0.9.6` | `Apache-2.0 OR MIT` |
| `const-random` | `0.1.18` | `MIT OR Apache-2.0` |
| `const-random-macro` | `0.1.16` | `MIT OR Apache-2.0` |
| `core-foundation` | `0.9.4` | `MIT OR Apache-2.0` |
| `core-foundation-sys` | `0.8.7` | `MIT OR Apache-2.0` |
| `cpufeatures` | `0.2.17` | `MIT OR Apache-2.0` |
| `cpufeatures` | `0.3.0` | `MIT OR Apache-2.0` |
| `crc32fast` | `1.5.0` | `MIT OR Apache-2.0` |
| `crossbeam` | `0.8.4` | `MIT OR Apache-2.0` |
| `crossbeam-channel` | `0.5.15` | `MIT OR Apache-2.0` |
| `crossbeam-deque` | `0.8.6` | `MIT OR Apache-2.0` |
| `crossbeam-epoch` | `0.9.18` | `MIT OR Apache-2.0` |
| `crossbeam-queue` | `0.3.12` | `MIT OR Apache-2.0` |
| `crossbeam-utils` | `0.8.21` | `MIT OR Apache-2.0` |
| `crunchy` | `0.2.4` | `MIT` |
| `crypto-common` | `0.1.7` | `MIT OR Apache-2.0` |
| `crypto-common` | `0.2.1` | `MIT OR Apache-2.0` |
| `ctutils` | `0.4.2` | `Apache-2.0 OR MIT` |
| `darling` | `0.23.0` | `MIT` |
| `darling_core` | `0.23.0` | `MIT` |
| `darling_macro` | `0.23.0` | `MIT` |
| `der` | `0.7.10` | `Apache-2.0 OR MIT` |
| `der_derive` | `0.7.3` | `Apache-2.0 OR MIT` |
| `digest` | `0.10.7` | `MIT OR Apache-2.0` |
| `digest` | `0.11.2` | `MIT OR Apache-2.0` |
| `displaydoc` | `0.2.5` | `MIT OR Apache-2.0` |
| `dunce` | `1.0.5` | `CC0-1.0 OR MIT-0 OR Apache-2.0` |
| `either` | `1.15.0` | `MIT OR Apache-2.0` |
| `encoding_rs` | `0.8.35` | `(Apache-2.0 OR MIT) AND BSD-3-Clause` |
| `enumflags2` | `0.7.12` | `MIT OR Apache-2.0` |
| `enumflags2_derive` | `0.7.12` | `MIT OR Apache-2.0` |
| `env_logger` | `0.9.3` | `MIT OR Apache-2.0` |
| `equivalent` | `1.0.2` | `Apache-2.0 OR MIT` |
| `errno` | `0.3.14` | `MIT OR Apache-2.0` |
| `fallible-iterator` | `0.2.0` | `MIT/Apache-2.0` |
| `find-msvc-tools` | `0.1.9` | `MIT OR Apache-2.0` |
| `flagset` | `0.4.7` | `Apache-2.0` |
| `flatbuffers` | `25.12.19` | `Apache-2.0` |
| `flate2` | `1.1.9` | `MIT OR Apache-2.0` |
| `foldhash` | `0.1.5` | `Zlib` |
| `form_urlencoded` | `1.2.2` | `MIT OR Apache-2.0` |
| `fs_extra` | `1.3.0` | `MIT` |
| `futures-channel` | `0.3.32` | `MIT OR Apache-2.0` |
| `futures-core` | `0.3.32` | `MIT OR Apache-2.0` |
| `futures-io` | `0.3.32` | `MIT OR Apache-2.0` |
| `futures-macro` | `0.3.32` | `MIT OR Apache-2.0` |
| `futures-sink` | `0.3.32` | `MIT OR Apache-2.0` |
| `futures-task` | `0.3.32` | `MIT OR Apache-2.0` |
| `futures-util` | `0.3.32` | `MIT OR Apache-2.0` |
| `generic-array` | `0.14.7` | `MIT` |
| `getrandom` | `0.1.16` | `MIT OR Apache-2.0` |
| `getrandom` | `0.2.17` | `MIT OR Apache-2.0` |
| `getrandom` | `0.3.4` | `MIT OR Apache-2.0` |
| `getrandom` | `0.4.2` | `MIT OR Apache-2.0` |
| `glob` | `0.3.3` | `MIT OR Apache-2.0` |
| `half` | `2.7.1` | `MIT OR Apache-2.0` |
| `hashbrown` | `0.15.5` | `MIT OR Apache-2.0` |
| `hashbrown` | `0.17.0` | `MIT OR Apache-2.0` |
| `heck` | `0.5.0` | `MIT OR Apache-2.0` |
| `hermit-abi` | `0.1.19` | `MIT/Apache-2.0` |
| `hex` | `0.4.3` | `MIT OR Apache-2.0` |
| `hmac` | `0.12.1` | `MIT OR Apache-2.0` |
| `hmac` | `0.13.0` | `MIT OR Apache-2.0` |
| `home` | `0.5.12` | `MIT OR Apache-2.0` |
| `humantime` | `2.3.0` | `MIT OR Apache-2.0` |
| `hybrid-array` | `0.4.11` | `MIT OR Apache-2.0` |
| `iana-time-zone` | `0.1.65` | `MIT OR Apache-2.0` |
| `iana-time-zone-haiku` | `0.1.2` | `MIT OR Apache-2.0` |
| `icu_collections` | `2.2.0` | `Unicode-3.0` |
| `icu_locale_core` | `2.2.0` | `Unicode-3.0` |
| `icu_normalizer` | `2.2.0` | `Unicode-3.0` |
| `icu_normalizer_data` | `2.2.0` | `Unicode-3.0` |
| `icu_properties` | `2.2.0` | `Unicode-3.0` |
| `icu_properties_data` | `2.2.0` | `Unicode-3.0` |
| `icu_provider` | `2.2.0` | `Unicode-3.0` |
| `id-arena` | `2.3.0` | `MIT/Apache-2.0` |
| `ident_case` | `1.0.1` | `MIT/Apache-2.0` |
| `idna` | `1.1.0` | `MIT OR Apache-2.0` |
| `idna_adapter` | `1.2.1` | `Apache-2.0 OR MIT` |
| `indexmap` | `2.14.0` | `Apache-2.0 OR MIT` |
| `instant` | `0.1.13` | `BSD-3-Clause` |
| `integer-encoding` | `3.0.4` | `MIT` |
| `is_terminal_polyfill` | `1.70.2` | `MIT OR Apache-2.0` |
| `itertools` | `0.13.0` | `MIT OR Apache-2.0` |
| `itoa` | `1.0.18` | `MIT OR Apache-2.0` |
| `jobserver` | `0.1.34` | `MIT OR Apache-2.0` |
| `js-sys` | `0.3.95` | `MIT OR Apache-2.0` |
| `keyed_priority_queue` | `0.4.2` | `MIT` |
| `lazy_static` | `1.5.0` | `MIT OR Apache-2.0` |
| `lazycell` | `1.3.0` | `MIT/Apache-2.0` |
| `leb128fmt` | `0.1.0` | `MIT OR Apache-2.0` |
| `libc` | `0.2.186` | `MIT OR Apache-2.0` |
| `libgssapi` | `0.4.6` | `MIT` |
| `libgssapi-sys` | `0.2.4` | `MIT` |
| `libloading` | `0.8.9` | `ISC` |
| `libm` | `0.2.16` | `MIT` |
| `libmimalloc-sys` | `0.1.49` | `MIT` |
| `libredox` | `0.1.16` | `MIT` |
| `libz-sys` | `1.1.28` | `MIT OR Apache-2.0` |
| `linux-raw-sys` | `0.4.15` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `litemap` | `0.8.2` | `Unicode-3.0` |
| `lock_api` | `0.4.14` | `MIT OR Apache-2.0` |
| `log` | `0.4.29` | `MIT OR Apache-2.0` |
| `lru` | `0.12.5` | `MIT` |
| `lz4_flex` | `0.13.1` | `MIT` |
| `md-5` | `0.11.0` | `MIT OR Apache-2.0` |
| `md5` | `0.6.1` | `Apache-2.0/MIT` |
| `memchr` | `2.8.0` | `Unlicense OR MIT` |
| `mimalloc` | `0.1.52` | `MIT` |
| `minimal-lexical` | `0.2.1` | `MIT/Apache-2.0` |
| `miniz_oxide` | `0.8.9` | `MIT OR Zlib OR Apache-2.0` |
| `mio` | `1.2.0` | `MIT` |
| `mysql_async` | `0.34.2` | `MIT/Apache-2.0` |
| `mysql_common` | `0.32.4` | `MIT/Apache-2.0` |
| `nom` | `7.1.3` | `MIT` |
| `num-bigint` | `0.4.6` | `MIT OR Apache-2.0` |
| `num-complex` | `0.4.6` | `MIT OR Apache-2.0` |
| `num-integer` | `0.1.46` | `MIT OR Apache-2.0` |
| `num-traits` | `0.2.19` | `MIT OR Apache-2.0` |
| `objc2-core-foundation` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` |
| `objc2-system-configuration` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` |
| `once_cell` | `1.21.4` | `MIT OR Apache-2.0` |
| `once_cell_polyfill` | `1.70.2` | `MIT OR Apache-2.0` |
| `openssl-probe` | `0.1.6` | `MIT/Apache-2.0` |
| `ordered-float` | `2.10.1` | `MIT` |
| `parking_lot` | `0.11.2` | `Apache-2.0/MIT` |
| `parking_lot` | `0.12.5` | `MIT OR Apache-2.0` |
| `parking_lot_core` | `0.8.6` | `Apache-2.0/MIT` |
| `parking_lot_core` | `0.9.12` | `MIT OR Apache-2.0` |
| `parquet` | `58.3.0` | `Apache-2.0` |
| `paste` | `1.0.15` | `MIT OR Apache-2.0` |
| `peeking_take_while` | `0.1.2` | `Apache-2.0/MIT` |
| `pem` | `3.0.6` | `MIT` |
| `percent-encoding` | `2.3.2` | `MIT OR Apache-2.0` |
| `phf` | `0.13.1` | `MIT` |
| `phf_shared` | `0.13.1` | `MIT` |
| `pin-project` | `1.1.11` | `Apache-2.0 OR MIT` |
| `pin-project-internal` | `1.1.11` | `Apache-2.0 OR MIT` |
| `pin-project-lite` | `0.2.17` | `Apache-2.0 OR MIT` |
| `pkg-config` | `0.3.33` | `MIT OR Apache-2.0` |
| `postgres-protocol` | `0.6.11` | `MIT OR Apache-2.0` |
| `postgres-types` | `0.2.13` | `MIT OR Apache-2.0` |
| `potential_utf` | `0.1.5` | `Unicode-3.0` |
| `ppv-lite86` | `0.2.21` | `MIT OR Apache-2.0` |
| `pretty-hex` | `0.3.0` | `MIT` |
| `prettyplease` | `0.2.37` | `MIT OR Apache-2.0` |
| `proc-macro2` | `1.0.106` | `MIT OR Apache-2.0` |
| `quad-rand` | `0.2.3` | `MIT` |
| `quote` | `1.0.45` | `MIT OR Apache-2.0` |
| `r-efi` | `5.3.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` |
| `r-efi` | `6.0.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` |
| `rand` | `0.10.1` | `MIT OR Apache-2.0` |
| `rand` | `0.7.3` | `MIT OR Apache-2.0` |
| `rand` | `0.8.6` | `MIT OR Apache-2.0` |
| `rand` | `0.9.4` | `MIT OR Apache-2.0` |
| `rand_chacha` | `0.2.2` | `MIT OR Apache-2.0` |
| `rand_chacha` | `0.3.1` | `MIT OR Apache-2.0` |
| `rand_chacha` | `0.9.0` | `MIT OR Apache-2.0` |
| `rand_core` | `0.10.1` | `MIT OR Apache-2.0` |
| `rand_core` | `0.5.1` | `MIT OR Apache-2.0` |
| `rand_core` | `0.6.4` | `MIT OR Apache-2.0` |
| `rand_core` | `0.9.5` | `MIT OR Apache-2.0` |
| `rand_hc` | `0.2.0` | `MIT/Apache-2.0` |
| `redox_syscall` | `0.2.16` | `MIT` |
| `redox_syscall` | `0.5.18` | `MIT` |
| `regex` | `1.12.3` | `MIT OR Apache-2.0` |
| `regex-automata` | `0.4.14` | `MIT OR Apache-2.0` |
| `regex-lite` | `0.1.9` | `MIT OR Apache-2.0` |
| `regex-syntax` | `0.8.10` | `MIT OR Apache-2.0` |
| `ring` | `0.17.14` | `Apache-2.0 AND ISC` |
| `rpassword` | `7.4.0` | `Apache-2.0` |
| `rtoolbox` | `0.0.5` | `Apache-2.0` |
| `rustc-hash` | `1.1.0` | `Apache-2.0/MIT` |
| `rustc-hash` | `2.1.2` | `Apache-2.0 OR MIT` |
| `rustc_version` | `0.4.1` | `MIT OR Apache-2.0` |
| `rustix` | `0.38.44` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `rustls` | `0.21.12` | `Apache-2.0 OR ISC OR MIT` |
| `rustls` | `0.23.39` | `Apache-2.0 OR ISC OR MIT` |
| `rustls-native-certs` | `0.6.3` | `Apache-2.0 OR ISC OR MIT` |
| `rustls-pemfile` | `1.0.4` | `Apache-2.0 OR ISC OR MIT` |
| `rustls-pemfile` | `2.2.0` | `Apache-2.0 OR ISC OR MIT` |
| `rustls-pki-types` | `1.14.1` | `MIT OR Apache-2.0` |
| `rustls-webpki` | `0.101.7` | `ISC` |
| `rustls-webpki` | `0.103.13` | `ISC` |
| `rustversion` | `1.0.22` | `MIT OR Apache-2.0` |
| `saturating` | `0.1.0` | `MIT` |
| `schannel` | `0.1.29` | `MIT` |
| `scopeguard` | `1.2.0` | `MIT OR Apache-2.0` |
| `sct` | `0.7.1` | `Apache-2.0 OR ISC OR MIT` |
| `security-framework` | `2.11.1` | `MIT OR Apache-2.0` |
| `security-framework-sys` | `2.17.0` | `MIT OR Apache-2.0` |
| `semver` | `1.0.28` | `MIT OR Apache-2.0` |
| `seq-macro` | `0.3.6` | `MIT OR Apache-2.0` |
| `serde` | `1.0.228` | `MIT OR Apache-2.0` |
| `serde_bytes` | `0.11.19` | `MIT OR Apache-2.0` |
| `serde_core` | `1.0.228` | `MIT OR Apache-2.0` |
| `serde_derive` | `1.0.228` | `MIT OR Apache-2.0` |
| `serde_json` | `1.0.149` | `MIT OR Apache-2.0` |
| `serde_spanned` | `0.6.9` | `MIT OR Apache-2.0` |
| `sha1` | `0.10.6` | `MIT OR Apache-2.0` |
| `sha2` | `0.10.9` | `MIT OR Apache-2.0` |
| `sha2` | `0.11.0` | `MIT OR Apache-2.0` |
| `shlex` | `1.3.0` | `MIT OR Apache-2.0` |
| `simd-adler32` | `0.3.9` | `MIT` |
| `simdutf8` | `0.1.5` | `MIT OR Apache-2.0` |
| `siphasher` | `1.0.2` | `MIT/Apache-2.0` |
| `slab` | `0.4.12` | `MIT` |
| `smallvec` | `1.15.1` | `MIT OR Apache-2.0` |
| `snap` | `1.1.1` | `BSD-3-Clause` |
| `socket2` | `0.5.10` | `MIT OR Apache-2.0` |
| `socket2` | `0.6.3` | `MIT OR Apache-2.0` |
| `spki` | `0.7.3` | `Apache-2.0 OR MIT` |
| `stable_deref_trait` | `1.2.1` | `MIT OR Apache-2.0` |
| `static_assertions` | `1.1.0` | `MIT OR Apache-2.0` |
| `stringprep` | `0.1.5` | `MIT/Apache-2.0` |
| `strsim` | `0.11.1` | `MIT` |
| `strsim` | `0.8.0` | `MIT` |
| `strum` | `0.27.2` | `MIT` |
| `strum_macros` | `0.27.2` | `MIT` |
| `subprocess` | `0.2.15` | `Apache-2.0/MIT` |
| `subtle` | `2.6.1` | `BSD-3-Clause` |
| `syn` | `2.0.117` | `MIT OR Apache-2.0` |
| `synstructure` | `0.13.2` | `MIT` |
| `termcolor` | `1.4.1` | `Unlicense OR MIT` |
| `textwrap` | `0.11.0` | `MIT` |
| `thiserror` | `1.0.69` | `MIT OR Apache-2.0` |
| `thiserror` | `2.0.18` | `MIT OR Apache-2.0` |
| `thiserror-impl` | `1.0.69` | `MIT OR Apache-2.0` |
| `thiserror-impl` | `2.0.18` | `MIT OR Apache-2.0` |
| `thrift` | `0.17.0` | `Apache-2.0` |
| `tiberius` | `0.12.3` | `MIT/Apache-2.0` |
| `tiny-keccak` | `2.0.2` | `CC0-1.0` |
| `tinystr` | `0.8.3` | `Unicode-3.0` |
| `tinyvec` | `1.11.0` | `Zlib OR Apache-2.0 OR MIT` |
| `tinyvec_macros` | `0.1.1` | `MIT OR Apache-2.0 OR Zlib` |
| `tls_codec` | `0.4.2` | `Apache-2.0 OR MIT` |
| `tls_codec_derive` | `0.4.2` | `Apache-2.0 OR MIT` |
| `tokio` | `1.52.1` | `MIT` |
| `tokio-macros` | `2.7.0` | `MIT` |
| `tokio-postgres` | `0.7.17` | `MIT OR Apache-2.0` |
| `tokio-postgres-rustls` | `0.13.0` | `MIT` |
| `tokio-rustls` | `0.24.1` | `MIT/Apache-2.0` |
| `tokio-rustls` | `0.26.4` | `MIT OR Apache-2.0` |
| `tokio-util` | `0.7.18` | `MIT` |
| `toml` | `0.8.23` | `MIT OR Apache-2.0` |
| `toml_datetime` | `0.6.11` | `MIT OR Apache-2.0` |
| `toml_edit` | `0.22.27` | `MIT OR Apache-2.0` |
| `toml_write` | `0.1.2` | `MIT OR Apache-2.0` |
| `tracing` | `0.1.44` | `MIT` |
| `tracing-attributes` | `0.1.31` | `MIT` |
| `tracing-core` | `0.1.36` | `MIT` |
| `twox-hash` | `1.6.3` | `MIT` |
| `twox-hash` | `2.1.2` | `MIT` |
| `typenum` | `1.20.0` | `MIT OR Apache-2.0` |
| `unicode-bidi` | `0.3.18` | `MIT OR Apache-2.0` |
| `unicode-ident` | `1.0.24` | `(MIT OR Apache-2.0) AND Unicode-3.0` |
| `unicode-normalization` | `0.1.25` | `MIT OR Apache-2.0` |
| `unicode-properties` | `0.1.4` | `MIT/Apache-2.0` |
| `unicode-width` | `0.1.14` | `MIT OR Apache-2.0` |
| `unicode-xid` | `0.2.6` | `MIT OR Apache-2.0` |
| `untrusted` | `0.9.0` | `ISC` |
| `url` | `2.5.8` | `MIT OR Apache-2.0` |
| `utf8_iter` | `1.0.4` | `Apache-2.0 OR MIT` |
| `utf8parse` | `0.2.2` | `Apache-2.0 OR MIT` |
| `uuid` | `1.23.1` | `Apache-2.0 OR MIT` |
| `vcpkg` | `0.2.15` | `MIT/Apache-2.0` |
| `vec_map` | `0.8.2` | `MIT/Apache-2.0` |
| `version_check` | `0.9.5` | `MIT/Apache-2.0` |
| `wasi` | `0.11.1+wasi-snapshot-preview1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wasi` | `0.14.7+wasi-0.2.4` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wasi` | `0.9.0+wasi-snapshot-preview1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wasip2` | `1.0.3+wasi-0.2.9` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wasip3` | `0.4.0+wasi-0.3.0-rc-2026-01-06` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wasite` | `1.0.2` | `Apache-2.0 OR BSL-1.0 OR MIT` |
| `wasm-bindgen` | `0.2.118` | `MIT OR Apache-2.0` |
| `wasm-bindgen-macro` | `0.2.118` | `MIT OR Apache-2.0` |
| `wasm-bindgen-macro-support` | `0.2.118` | `MIT OR Apache-2.0` |
| `wasm-bindgen-shared` | `0.2.118` | `MIT OR Apache-2.0` |
| `wasm-encoder` | `0.244.0` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wasm-metadata` | `0.244.0` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wasmparser` | `0.244.0` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `web-sys` | `0.3.95` | `MIT OR Apache-2.0` |
| `webpki` | `0.22.4` | `NOASSERTION` |
| `webpki-roots` | `0.26.11` | `CDLA-Permissive-2.0` |
| `webpki-roots` | `1.0.7` | `CDLA-Permissive-2.0` |
| `which` | `4.4.2` | `MIT` |
| `whoami` | `2.1.1` | `Apache-2.0 OR BSL-1.0 OR MIT` |
| `winapi` | `0.3.9` | `MIT/Apache-2.0` |
| `winapi-i686-pc-windows-gnu` | `0.4.0` | `MIT/Apache-2.0` |
| `winapi-util` | `0.1.11` | `Unlicense OR MIT` |
| `winapi-x86_64-pc-windows-gnu` | `0.4.0` | `MIT/Apache-2.0` |
| `winauth` | `0.0.4` | `MIT/Apache-2.0` |
| `windows-core` | `0.62.2` | `MIT OR Apache-2.0` |
| `windows-implement` | `0.60.2` | `MIT OR Apache-2.0` |
| `windows-interface` | `0.59.3` | `MIT OR Apache-2.0` |
| `windows-link` | `0.2.1` | `MIT OR Apache-2.0` |
| `windows-result` | `0.4.1` | `MIT OR Apache-2.0` |
| `windows-strings` | `0.5.1` | `MIT OR Apache-2.0` |
| `windows-sys` | `0.52.0` | `MIT OR Apache-2.0` |
| `windows-sys` | `0.59.0` | `MIT OR Apache-2.0` |
| `windows-sys` | `0.61.2` | `MIT OR Apache-2.0` |
| `windows-targets` | `0.52.6` | `MIT OR Apache-2.0` |
| `windows_aarch64_gnullvm` | `0.52.6` | `MIT OR Apache-2.0` |
| `windows_aarch64_msvc` | `0.52.6` | `MIT OR Apache-2.0` |
| `windows_i686_gnu` | `0.52.6` | `MIT OR Apache-2.0` |
| `windows_i686_gnullvm` | `0.52.6` | `MIT OR Apache-2.0` |
| `windows_i686_msvc` | `0.52.6` | `MIT OR Apache-2.0` |
| `windows_x86_64_gnu` | `0.52.6` | `MIT OR Apache-2.0` |
| `windows_x86_64_gnullvm` | `0.52.6` | `MIT OR Apache-2.0` |
| `windows_x86_64_msvc` | `0.52.6` | `MIT OR Apache-2.0` |
| `winnow` | `0.7.15` | `MIT` |
| `wit-bindgen` | `0.51.0` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wit-bindgen` | `0.57.1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wit-bindgen-core` | `0.51.0` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wit-bindgen-rust` | `0.51.0` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wit-bindgen-rust-macro` | `0.51.0` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wit-component` | `0.244.0` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `wit-parser` | `0.244.0` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| `writeable` | `0.6.3` | `Unicode-3.0` |
| `x509-cert` | `0.2.5` | `Apache-2.0 OR MIT` |
| `yoke` | `0.8.2` | `Unicode-3.0` |
| `yoke-derive` | `0.8.2` | `Unicode-3.0` |
| `zerocopy` | `0.8.48` | `BSD-2-Clause OR Apache-2.0 OR MIT` |
| `zerocopy-derive` | `0.8.48` | `BSD-2-Clause OR Apache-2.0 OR MIT` |
| `zerofrom` | `0.1.7` | `Unicode-3.0` |
| `zerofrom-derive` | `0.1.7` | `Unicode-3.0` |
| `zeroize` | `1.8.2` | `Apache-2.0 OR MIT` |
| `zeroize_derive` | `1.4.3` | `Apache-2.0 OR MIT` |
| `zerotrie` | `0.2.4` | `Unicode-3.0` |
| `zerovec` | `0.11.6` | `Unicode-3.0` |
| `zerovec-derive` | `0.11.3` | `Unicode-3.0` |
| `zlib-rs` | `0.6.5` | `Zlib` |
| `zmij` | `1.0.21` | `MIT` |
| `zstd` | `0.13.3` | `MIT` |
| `zstd-safe` | `7.2.4` | `MIT OR Apache-2.0` |
| `zstd-sys` | `2.0.16+zstd.1.5.7` | `MIT/Apache-2.0` |
