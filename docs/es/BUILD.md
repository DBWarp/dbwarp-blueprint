# Compilar dbwarp-blueprint desde el código fuente

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. El inglés es la fuente canónica y este texto no debe considerarse apto para uso contractual. Consulte el [documento canónico en inglés](../../BUILD.md).

**Idiomas:** [English](../../BUILD.md) | [Deutsch](../de/BUILD.md) | [Français](../fr/BUILD.md) | **Español** | [Polski](../pl/BUILD.md) | [日本語](../ja/BUILD.md) | [中文](../zh/BUILD.md)

Esta guía está destinada a clientes que prefieren compilar la herramienta por
sí mismos antes de ejecutarla contra una base de datos.

## Compilación rápida

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
./build.sh
```

El binario se escribe en:

```text
target/release/dbwarp-blueprint
```

## Qué hace el script de compilación

`build.sh` es deliberadamente conservador:

- lee la versión fijada de Rust desde `rust-toolchain.toml`;
- utiliza el `rustc` existente si coincide con la versión fijada;
- se niega a descargar Rust salvo que se establezca `ALLOW_NETWORK=1`;
- fija la versión de arranque de rustup y verifica su SHA-256 oficial antes de usarla;
- mantiene el estado de la cadena de herramientas bajo `./build/`;
- utiliza Cargo.lock para obtener versiones de dependencias reproducibles;
- compila de forma predeterminada con `cargo build --release --locked`;
- cambia automáticamente a `--frozen --offline --locked` cuando se ejecuta
  desde un paquete de código fuente con dependencias incluidas;
- rechaza `DBWARP_BLUEPRINT_OFFLINE=1` salvo que exista `vendor-crates/`;
- muestra el SHA256 del binario resultante;
- incorpora en la auditoría la revisión exacta del código fuente y el estado del árbol de trabajo.

No utiliza `sudo` ni modifica la instalación de Rust del sistema.

## Binarios descargables

Hay binarios precompilados disponibles en la página Releases:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

Se proporcionan por comodidad. Fije una etiqueta de versión exacta y verifique su SHA-256 antes de usarlos; no utilice una URL de descarga mutable para una ejecución reproducible. Si su política exige revisar el código fuente, compile localmente a partir de la misma etiqueta.

Archivos de la versión:

| Plataforma | Archivo |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## Verificar un archivo descargado

Linux/macOS:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

## Compilaciones específicas de autenticación

La compilación predeterminada admite flujos de contraseña, archivo de token,
token de entorno y TLS; mTLS mediante certificado de cliente está disponible
para PostgreSQL y MySQL.

La autenticación integrada de SQL Server tiene compilaciones específicas de la
plataforma:

| Plataforma | Comando de compilación | Finalidad |
|---|---|---|
| Linux | `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Kerberos / GSSAPI |
| Windows | Binario de Windows de GitHub Release, o `cargo build --release --features winauth` | Windows Integrated Auth / SSPI |

Kerberos en Linux requiere las bibliotecas de ejecución habituales de MIT
Kerberos. Si `kinit` funciona en el host, normalmente ya estarán presentes los
componentes de ejecución necesarios.

## Compilar sin el script

Si su política prefiere comandos directos de Cargo:

```bash
cargo build --release --locked
```

Compilación SSPI para Windows:

```powershell
cargo build --release --locked --features winauth
```

Compilación Kerberos para Linux:

```bash
cargo build --release --locked --features integrated-auth-gssapi
```

## Dependencias incluidas

El repositorio normal incluye una pequeña dependencia corregida bajo
`vendor/mysql_async` para que `--tls-ca` de MySQL tenga la misma semántica
restrictiva de confianza que el resto de la herramienta. Todas las demás
versiones de dependencias están fijadas mediante `Cargo.lock`.

Cada GitHub Release publica un paquete independiente
`dbwarp-blueprint-source-vendored.tar.gz` para los equipos de seguridad que deseen
inspeccionar y compilar sin conexión a partir de todos los archivos de código
fuente de las dependencias.

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Ese paquete contiene la versión corregida de `vendor/mysql_async`, un árbol
`vendor-crates/` generado con todas las demás dependencias y un archivo
`.cargo/config.toml` generado que redirige crates.io al árbol local de
dependencias. En ese modo, `build.sh` utiliza
`cargo build --release --frozen --offline --locked`.
