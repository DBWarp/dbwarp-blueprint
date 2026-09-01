# dbwarp-blueprint aus dem Quellcode erstellen

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../BUILD.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../BUILD.md) | **Deutsch** | [Français](../fr/BUILD.md) | [Español](../es/BUILD.md) | [Polski](../pl/BUILD.md) | [日本語](../ja/BUILD.md) | [中文](../zh/BUILD.md)

Dieser Leitfaden richtet sich an Kunden, die das Werkzeug lieber selbst erstellen möchten, bevor sie es für eine Datenbank ausführen.

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

Release-Dateien:

| Plattform | Datei |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## Ein heruntergeladenes Archiv verifizieren

Linux/macOS:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

## Authentifizierungsspezifische Builds

Der Standard-Build unterstützt Passwort-, Token-Datei-,
Token-Umgebungsvariablen- und TLS-Abläufe; mTLS mit Clientzertifikat ist für
PostgreSQL und MySQL verfügbar.

Für die integrierte SQL-Server-Authentifizierung gibt es plattformspezifische Builds:

| Plattform | Build-Befehl | Zweck |
|---|---|---|
| Linux | `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Kerberos / GSSAPI |
| Windows | GitHub-Release-Binärdatei für Windows oder `cargo build --release --features winauth` | Windows Integrated Auth / SSPI |

Linux-Kerberos benötigt die üblichen MIT-Kerberos-Laufzeitbibliotheken. Wenn `kinit` auf dem Host funktioniert, sind die erforderlichen Laufzeitkomponenten üblicherweise bereits vorhanden.

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

## Gebündelte Abhängigkeiten

Das normale Repository enthält unter `vendor/mysql_async` eine kleine gepatchte Abhängigkeit, damit MySQL `--tls-ca` dieselbe restriktive Vertrauenssemantik wie der Rest des Werkzeugs aufweist. Alle anderen Abhängigkeitsversionen sind durch `Cargo.lock` festgeschrieben.

Jedes GitHub Release veröffentlicht ein separates Paket `dbwarp-blueprint-source-vendored.tar.gz` für Sicherheitsteams, die jede Abhängigkeitsquelldatei offline prüfen und daraus erstellen möchten.

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Dieses Paket enthält das gepatchte `vendor/mysql_async`, einen erzeugten Baum `vendor-crates/` für alle anderen Abhängigkeiten sowie eine erzeugte `.cargo/config.toml`, die crates.io auf den lokalen Vendor-Baum umleitet. In diesem Modus verwendet `build.sh` den Befehl `cargo build --release --frozen --offline --locked`.
