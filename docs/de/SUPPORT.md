# Support und Problemmeldungen

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../SUPPORT.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../SUPPORT.md) | **Deutsch** | [Français](../fr/SUPPORT.md) | [Español](../es/SUPPORT.md) | [Polski](../pl/SUPPORT.md) | [日本語](../ja/SUPPORT.md) | [中文](../zh/SUPPORT.md)

Verwenden Sie den
[Issue-Tracker](https://github.com/DBWarp/dbwarp-blueprint/issues) für
Installationsfragen ohne sensible Angaben, reproduzierbare Fehler und
Funktionswünsche. Dieser Kanal verspricht weder eine Antwortzeit noch ein
Support-Service-Level.

Melden Sie vermutete Sicherheitslücken vertraulich über den in
[SECURITY.md](SECURITY.md) beschriebenen Weg, nicht in einem öffentlichen Issue.

## Hilfreiche Angaben

- Genauer Release-Tag, Prüfsumme der Binärdatei, Betriebssystem und Architektur.
- Datenbank-Engine und Version oder strukturiertes Dateiformat sowie die
  Angabe, ob die Quelle selbstverwaltet oder ein verwalteter Dienst ist.
- Befehlsoptionen, aus denen Anmeldedaten, Endpunkte, Pfade und identifizierende
  Selektoren entfernt oder durch eindeutig gekennzeichnete Beispiele ersetzt
  wurden.
- Der Diagnosecode `DBP`, das erwartete und das tatsächliche Verhalten.
- Nach Möglichkeit ein kleines synthetisches Beispiel zur Reproduktion.

Laden Sie keine Produktionsdaten, Anmeldedaten oder ungeprüften Auditprotokolle,
Bundles, Blueprints oder Präsentationen hoch. Auditprotokolle können
Identitäten und Endpunkte enthalten; anonyme Blueprints können weiterhin eine
charakteristische Workload-Struktur offenlegen. Beginnen Sie mit einer
möglichst knappen sicheren Beschreibung und prüfen Sie jeden Anhang vor der
Weitergabe.

## Unterstützte Konfigurationen

[STATUS.md](../../STATUS.md) beschreibt die Fähigkeiten und die unterstützten Datenbankversionen. [BUILD.md](BUILD.md) beschreibt plattform- und authentifizierungs-spezifische Build-Anforderungen. Rust ist auf die exakte Version in `rust-toolchain.toml` festgelegt; andere Toolchains wurden möglicherweise nicht getestet.

Anleitungen zu Berechtigungen für Managed-Services sind keine Aussage darüber, dass jeder Dienst oder jede Konfiguration getestet wurde. Verwenden Sie die entsprechenden [Berechtigungsanforderungen](../../sql/grants/DATABASE_PERMISSIONS.md) und testen Sie die genaue Konfiguration, bevor Sie sie in der Produktion einsetzen.

Änderungen zwischen Collector-Versionen finden Sie in [CHANGELOG.md](CHANGELOG.md).
