# dbwarp-blueprint herunterladen

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../binaries/README.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../binaries/README.md) | **Deutsch** | [Français](../fr/BINARIES.md) | [Español](../es/BINARIES.md) | [Polski](../pl/BINARIES.md) | [日本語](../ja/BINARIES.md) | [中文](../zh/BINARIES.md)

Vorkompilierte Binärdateien von `dbwarp-blueprint` werden auf der GitHub-Releases-Seite veröffentlicht:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

Sie können eine Binärdatei herunterladen, ihre Prüfsumme verifizieren, sie lokal ausführen und die erzeugte Datei `blueprint.toml` prüfen, bevor Sie irgendetwas an DBWarp weitergeben.

Wählen Sie einen exakten Release-Tag, zum Beispiel `https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0`, und laden Sie das Archiv sowie `SHA256SUMS.txt` von demselben Tag herunter. Verwenden Sie für einen reproduzierbaren oder auditierten Lauf keine veränderliche `releases/latest`-URL.

## Dateien

| Plattform | Datei |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |
| Offline-Quellcodeprüfpaket | `dbwarp-blueprint-source-vendored.tar.gz` |
| Prüfsummen | `SHA256SUMS.txt` |

Jedes Release enthält außerdem `SHA256SUMS.txt`.

## Download verifizieren

Linux/macOS:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Vergleichen Sie den ausgegebenen Hash mit der entsprechenden Zeile in `SHA256SUMS.txt`.

## Heruntergeladene Binärdatei oder lokaler Build?

Die herunterladbare Binärdatei dient der Bequemlichkeit. Der vertrauenswürdigste Weg bleibt die Erstellung aus dem Quellcode:

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
git checkout <release-tag>
./build.sh
```

Dieser normale Quellcode-Klon ist absichtlich klein und verwendet `Cargo.lock`, um Abhängigkeitsversionen festzuschreiben.

Wenn Ihre Richtlinie verlangt, vor dem Build jede Abhängigkeitsquelldatei zu prüfen, laden Sie `dbwarp-blueprint-source-vendored.tar.gz` aus demselben Release herunter und erstellen Sie das Programm in dem entpackten Verzeichnisbaum:

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Siehe [`../BUILD.md`](BUILD.md).

## Funktionsweise des Werkzeugs

`dbwarp-blueprint` liest Datenbankmetadaten und misst optional die Komprimierung anhand einer kleinen lokalen Stichprobe. Es schreibt für die DBWarp-Migrationsschätzung eine anonymisierte Textdatei. Es lädt die Datei nicht hoch und sendet keine Telemetrie.
