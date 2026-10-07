# Compilar dbwarp-blueprint desde el código fuente

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. El inglés es la fuente canónica y este texto no debe considerarse apto para uso contractual. Consulte el [documento canónico en inglés](../../BUILD.md).

**Idiomas:** [English](../../BUILD.md) | [Deutsch](../de/BUILD.md) | [Français](../fr/BUILD.md) | **Español** | [Polski](../pl/BUILD.md) | [日本語](../ja/BUILD.md) | [中文](../zh/BUILD.md)

Utilice esta guía si prefiere compilar la herramienta usted mismo antes de
ejecutarla contra una base de datos.

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

Los demás ejemplos utilizan `./dbwarp-blueprint`. Después de compilar desde el
código fuente, ejecute directamente `target/release/dbwarp-blueprint` o copie
ese archivo a `./dbwarp-blueprint` antes de seguirlos.

Si la cadena de herramientas Rust fijada aún no está instalada y se permite el
acceso de red revisado, autorícelo explícitamente:

```bash
ALLOW_NETWORK=1 ./build.sh
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

Los archivos binarios de plataforma son paquetes para el operador, no árboles
de código fuente, y no se pueden reconstruir en el mismo lugar. La copia de
esta guía y `verify.sh` que contienen describe la ruta de verificación con el
código fuente coincidente. Utilice una copia de la etiqueta exacta de la
versión o el archivo de código fuente con dependencias de la versión cuando
necesite `build.sh`, el código fuente de Cargo o una compilación local para
comparar.

Archivos de la versión:

| Plataforma | Archivo |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## Verificar un archivo descargado

Linux:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS:

```bash
shasum -a 256 dbwarp-blueprint-macos-arm64.tar.gz
```

Compare el valor mostrado con la línea correspondiente de `SHA256SUMS.txt`.

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Cada versión también publica un archivo
`dbwarp-blueprint-<platform>.binary.sha256` para el ejecutable extraído. Consulte
[Descargar binarios](BINARIES.md) para ver el comando de verificación.

## Compilaciones específicas de autenticación

La compilación predeterminada admite flujos de contraseña, archivo de token,
token de entorno y TLS; mTLS mediante certificado de cliente está disponible
para PostgreSQL y MySQL.

La autenticación integrada de SQL Server tiene compatibilidad específica para
cada plataforma:

| Plataforma | Comando de compilación | Finalidad |
|---|---|---|
| Linux | Binario de Linux de la versión de GitHub, o `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Autenticación por contraseña, token y TLS, además de Kerberos / GSSAPI cuando se selecciona |
| Windows | Binario de Windows de GitHub Release, o `cargo build --release --locked --features winauth` | Windows Integrated Auth / SSPI |

Los binarios de la versión de Linux no necesitan bibliotecas Kerberos para
iniciarse. Cargan el entorno de ejecución GSSAPI de la plataforma solo cuando
se selecciona `--auth-mode integrated`. Si `kinit` funciona, normalmente ya
estarán presentes los componentes de ejecución necesarios. Las compilaciones
desde el código fuente habilitan Kerberos/GSSAPI con `integrated-auth-gssapi`,
como se muestra arriba.

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

## Reproducir un binario de versión

`./build.sh` demuestra que el origen revisado se compila correctamente; la identidad de bytes requiere, además, todas las entradas de compilación nativa completas de la versión. Consulte la revisión exacta del origen registrada en `PROVENANCE.json`, publicada con cada versión, utilice su lista de destinos y características, la cadena de herramientas Rust especificada, la compilación nativa registrada compiler/linker, la marca de tiempo del commit como `SOURCE_DATE_EPOCH` y las rutas de remapeo y las banderas del enlazador del flujo de trabajo de la versión. Las versiones de Windows también utilizan `clang-cl` y `/Brepro`.

Tras reproducir esas entradas, compare el binario extraído con el resultado local:

```bash
SOURCE_BIN=target/release/dbwarp-blueprint \
  ./verify.sh /path/to/extracted/dbwarp-blueprint
```

Si los hashes son diferentes, no considere que los binarios son equivalentes. Cada versión se construye dos veces y una discrepancia de bytes hace que la versión falle. `PROVENANCE.json` registra la revisión de origen, el destino, las características, la cadena de herramientas, la época de la fecha de origen, el compilador nativo, el tamaño del binario y el hash necesarios para evaluar una reproducción local.

## Dependencias incluidas

El repositorio incluye dependencias parcheadas en `vendor/`. Conservan las
reglas restrictivas de confianza de `--tls-ca` para MySQL y SQL Server. La
autenticación integrada de Linux carga GSSAPI solo cuando se solicita. La
autenticación integrada de Windows utiliza una dependencia mantenida para la
generación de números aleatorios. Todas las demás versiones de dependencias
están fijadas mediante `Cargo.lock`.

Cada GitHub Release publica un paquete independiente
`dbwarp-blueprint-source-vendored.tar.gz` para los equipos de seguridad que deseen
inspeccionar y compilar sin conexión a partir de todos los archivos de código
fuente de las dependencias.

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Ese paquete contiene las dependencias corregidas en `vendor/`, un árbol
`vendor-crates/` generado con todas las demás dependencias y un archivo
`.cargo/config.toml` generado que redirige crates.io al árbol local de
dependencias. En ese modo, `build.sh` utiliza
`cargo build --release --frozen --offline --locked`.
