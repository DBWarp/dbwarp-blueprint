# dbwarp-blueprint aus dem Quellcode erstellen

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../BUILD.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../BUILD.md) | **Deutsch** | [Français](../fr/BUILD.md) | [Español](../es/BUILD.md) | [Polski](../pl/BUILD.md) | [日本語](../ja/BUILD.md) | [中文](../zh/BUILD.md)

Verwenden Sie diese Anleitung, wenn Sie das Werkzeug selbst erstellen möchten, bevor Sie es für eine Datenbank ausführen.

## Schneller Build

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
./build.sh
```

Die Binärdatei wird hierhin geschrieben:

```text
target/release/dbwarp-blueprint
```

Alle anderen Beispiele verwenden `./dbwarp-blueprint`. Führen Sie nach einem
Quell-Build entweder `target/release/dbwarp-blueprint` direkt aus oder kopieren
Sie die Datei nach `./dbwarp-blueprint`, bevor Sie den Beispielen folgen.

Wenn die festgeschriebene Rust-Toolchain noch nicht installiert ist und ein
geprüfter Netzwerkzugriff zulässig ist, stimmen Sie ausdrücklich zu:

```bash
ALLOW_NETWORK=1 ./build.sh
```

## Funktionsweise des Build-Skripts

`build.sh` ist absichtlich konservativ:

- liest die festgeschriebene Rust-Version aus `rust-toolchain.toml`;
- verwendet Ihr vorhandenes `rustc`, wenn es der festgeschriebenen Version entspricht;
- verweigert das Herunterladen von Rust, sofern `ALLOW_NETWORK=1` nicht gesetzt ist;
- fixiert die rustup-Bootstrap-Version und prüft vor der Verwendung den offiziellen SHA-256;
- hält den Toolchain-Zustand unter `./build/`;
- verwendet Cargo.lock für reproduzierbare Abhängigkeitsversionen;
- erstellt standardmäßig mit `cargo build --release --locked`;
- wechselt bei Ausführung aus einem gebündelten Quellcodepaket automatisch zu `--frozen --offline --locked`;
- verweigert `DBWARP_BLUEPRINT_OFFLINE=1`, wenn `vendor-crates/` nicht vorhanden ist;
- gibt den SHA256 der erzeugten Binärdatei aus;
- versieht das Audit mit der exakten Quellrevision und dem Zustand des Arbeitsbaums.

Es verwendet kein `sudo` und verändert Ihre systemweite Rust-Installation nicht.

## Herunterladbare Binärdateien

Vorkompilierte Binärdateien sind auf der Releases-Seite verfügbar:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

Sie werden der Einfachheit halber bereitgestellt. Fixieren Sie vor der Verwendung ein exaktes Release-Tag und prüfen Sie den SHA-256; verwenden Sie für einen reproduzierbaren Lauf keine veränderliche Download-URL. Wenn Ihre Richtlinie eine Quellcodeprüfung verlangt, erstellen Sie das Programm lokal aus demselben Tag.

Plattform-Binärarchive sind Betreiberpakete und keine Quellcodebäume; sie
können nicht an Ort und Stelle neu gebaut werden. Die enthaltene Kopie dieses
Leitfadens und `verify.sh` beschreiben den Verifizierungspfad mit passendem
Quellcode. Verwenden Sie einen Checkout des exakten Release-Tags oder das
Quellcodearchiv mit gebündelten Abhängigkeiten, wenn Sie `build.sh`, die
Cargo-Quellen oder einen lokalen Vergleichs-Build benötigen.

Release-Dateien:

| Plattform | Datei |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## Ein heruntergeladenes Archiv verifizieren

Linux:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS:

```bash
shasum -a 256 dbwarp-blueprint-macos-arm64.tar.gz
```

Vergleichen Sie den ausgegebenen Wert mit der passenden Zeile in `SHA256SUMS.txt`.

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Jedes Release veröffentlicht außerdem eine Datei
`dbwarp-blueprint-<platform>.binary.sha256` für die entpackte Programmdatei.
Den Prüfbefehl finden Sie unter [Binärdateien herunterladen](BINARIES.md).

## Authentifizierungsspezifische Builds

Der Standard-Build unterstützt Passwort-, Token-Datei-,
Token-Umgebungsvariablen- und TLS-Abläufe; mTLS mit Clientzertifikat ist für
PostgreSQL und MySQL verfügbar.

Die integrierte SQL-Server-Authentifizierung wird plattformspezifisch unterstützt:

| Plattform | Build-Befehl | Zweck |
|---|---|---|
| Linux | Linux-Binärdatei aus dem GitHub-Release oder `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Passwort-, Token- und TLS-Authentifizierung sowie Kerberos / GSSAPI bei Auswahl |
| Windows | GitHub-Release-Binärdatei für Windows oder `cargo build --release --locked --features winauth` | Windows Integrated Auth / SSPI |

Linux-Release-Binärdateien benötigen keine Kerberos-Bibliotheken zum Starten.
Sie laden die GSSAPI-Laufzeitumgebung der Plattform nur, wenn
`--auth-mode integrated` ausgewählt ist. Wenn `kinit` funktioniert, sind die
erforderlichen Laufzeitkomponenten üblicherweise bereits vorhanden. Builds aus
dem Quellcode aktivieren Kerberos/GSSAPI wie oben gezeigt mit
`integrated-auth-gssapi`.

## Ohne das Skript erstellen

Wenn Ihre Richtlinie direkte Cargo-Befehle bevorzugt:

```bash
cargo build --release --locked
```

Windows-SSPI-Build:

```powershell
cargo build --release --locked --features winauth
```

Linux-Kerberos-Build:

```bash
cargo build --release --locked --features integrated-auth-gssapi
```

## Eine Release-Binärdatei reproduzieren

`./build.sh` beweist, dass die überprüfte Quelle kompiliert; die Byte-Identität erfordert zusätzlich die vollständigen nativen Build-Eingaben der jeweiligen Version. Überprüfen Sie die genaue Quellversion, die in `PROVENANCE.json` dokumentiert ist und mit jeder Version veröffentlicht wird. Verwenden Sie die Zielplattform und die Liste der Funktionen, die festgelegte Rust-Toolchain, die aufgezeichneten nativen compiler/linker, den Commit-Zeitstempel als `SOURCE_DATE_EPOCH` sowie die Pfad-Remapping- und Linker-Flags des Versions-Workflows. Windows-Versionen verwenden außerdem `clang-cl` und `/Brepro`.

Vergleichen Sie nach der Reproduktion dieser Eingaben die extrahierte
Release-Binärdatei mit dem lokalen Ergebnis:

```bash
SOURCE_BIN=target/release/dbwarp-blueprint \
  ./verify.sh /path/to/extracted/dbwarp-blueprint
```

Wenn die Hash-Werte unterschiedlich sind, sollten die Binärdateien nicht als äquivalent betrachtet werden. Jede Version wird zweimal erstellt, und eine Abweichung eines Bytes führt zum Fehlschlagen der Version. `PROVENANCE.json` speichert die Quellversion, das Ziel, die Funktionen, die Toolchain, das Datum der Quellversion (als Epoche), den nativen Compiler, die Größe der Binärdatei und den Hash-Wert, die zur Bewertung einer lokalen Reproduktion benötigt werden.

## Gebündelte Abhängigkeiten

Das Repository enthält gepatchte Abhängigkeiten unter `vendor/`. Sie bewahren die restriktiven `--tls-ca`-Vertrauensregeln für MySQL und SQL Server. Die integrierte Linux-Authentifizierung lädt GSSAPI nur bei Bedarf. Die integrierte Windows-Authentifizierung verwendet eine gepflegte Abhängigkeit für die Zufallszahlenerzeugung. Alle anderen Abhängigkeitsversionen sind durch `Cargo.lock` festgeschrieben.

Jedes GitHub Release veröffentlicht ein separates Paket `dbwarp-blueprint-source-vendored.tar.gz` für Sicherheitsteams, die jede Abhängigkeitsquelldatei offline prüfen und daraus erstellen möchten.

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Dieses Paket enthält die gepatchten Abhängigkeiten unter `vendor/`, einen erzeugten Baum `vendor-crates/` für alle anderen Abhängigkeiten sowie eine erzeugte `.cargo/config.toml`, die crates.io auf den lokalen Vendor-Baum umleitet. In diesem Modus verwendet `build.sh` den Befehl `cargo build --release --frozen --offline --locked`.
