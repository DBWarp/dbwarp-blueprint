# Descargar dbwarp-blueprint

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. El inglés es la fuente canónica y este texto no debe considerarse apto para uso contractual. Consulte el [documento canónico en inglés](../../binaries/README.md).

**Idiomas:** [English](../../binaries/README.md) | [Deutsch](../de/BINARIES.md) | [Français](../fr/BINARIES.md) | **Español** | [Polski](../pl/BINARIES.md) | [日本語](../ja/BINARIES.md) | [中文](../zh/BINARIES.md)

Los binarios precompilados de `dbwarp-blueprint` se publican en la página de
GitHub Releases:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

Puede descargar un binario, verificar su suma de comprobación, ejecutarlo
localmente e inspeccionar el archivo `blueprint.toml` generado antes de compartir
nada con DBWarp.

Elija una etiqueta de versión exacta, por ejemplo `https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0`, y descargue el archivo y `SHA256SUMS.txt` desde esa misma etiqueta. No utilice una URL mutable `releases/latest` para una ejecución reproducible o auditada.

## Archivos

| Plataforma | Archivo |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |
| Paquete de código fuente para auditoría sin conexión | `dbwarp-blueprint-source-vendored.tar.gz` |
| Sumas de comprobación | `SHA256SUMS.txt` |

Cada versión también incluye `SHA256SUMS.txt`.

## Verificar la descarga

Linux/macOS:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Compare el hash mostrado con la línea correspondiente de `SHA256SUMS.txt`.

## ¿Binario descargado o compilación local?

El binario descargable se ofrece por comodidad. La ruta que proporciona mayor
confianza sigue siendo compilar desde el código fuente:

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
git checkout <release-tag>
./build.sh
```

Ese clon normal del código fuente es deliberadamente pequeño y utiliza
`Cargo.lock` para fijar las versiones de las dependencias.

Si su política exige revisar cada archivo de código fuente de las dependencias
antes de compilar, descargue `dbwarp-blueprint-source-vendored.tar.gz` de la misma
versión y compile dentro del árbol extraído:

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Consulte [`../BUILD.md`](BUILD.md).

## Función de la herramienta

`dbwarp-blueprint` lee metadatos de la base de datos y mide opcionalmente la
compresión en una pequeña muestra local. Escribe un archivo de texto anonimizado
para la estimación de migraciones de DBWarp. No carga el archivo ni envía
telemetría.
