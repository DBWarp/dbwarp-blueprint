# Beiträge leisten

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../CONTRIBUTING.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../CONTRIBUTING.md) | **Deutsch** | [Français](../fr/CONTRIBUTING.md) | [Español](../es/CONTRIBUTING.md) | [Polski](../pl/CONTRIBUTING.md) | [日本語](../ja/CONTRIBUTING.md) | [中文](../zh/CONTRIBUTING.md)

Beginnen Sie mit einem Issue ohne sensible Angaben, das das Problem beschreibt
und ein kleines synthetisches Beispiel zur Reproduktion enthält. Besprechen
Sie bei umfangreichen Änderungen den Ansatz, bevor Sie einen Patch erstellen.
Die Maintainer entscheiden, ob eine vorgeschlagene Änderung zum Produkt und
seinen Sicherheitsgrenzen passt; das Eröffnen eines Issues oder Pull Requests
bedeutet keine Zusage zur Annahme.

Beachten Sie [SUPPORT.md](SUPPORT.md) für sichere Problemmeldungen und
[SECURITY.md](SECURITY.md) für vertrauliche Meldungen von Sicherheitslücken.
Führen Sie Diskussionen respektvoll und konzentrieren Sie sich auf
reproduzierbares Verhalten.

## Eine Änderung vorbereiten

- Verwenden Sie die in [BUILD.md](BUILD.md) beschriebene festgeschriebene
  Toolchain und die festgeschriebenen Abhängigkeiten.
- Halten Sie Änderungen eng begrenzt und ergänzen Sie Regressionstests für
  geändertes Verhalten.
- Nehmen Sie niemals Kundendaten, Anmeldedaten, private Infrastrukturdetails
  oder identifizierende Auditnachweise in einen Patch oder Testdatensatz auf.
- Bewahren Sie ausdrückliche Zustimmung, begrenzte Auswirkungen auf die
  Quelle, Zugriff mit minimalen Berechtigungen und eine ehrliche Darstellung
  fehlender oder eingeschränkter Beobachtungen.
- Dokumentieren Sie jede Änderung des Blueprint-Vertrags oder der
  Komprimierungskodierung. Bestehende Regeln für optionale Felder und
  Kompatibilität sind Teil der Schnittstelle.
- CLI-Optionen, Diagnosecodes und serialisierte Felder bleiben kanonisches
  Englisch. Änderungen an menschenlesbaren Laufzeittexten müssen alle
  ausgelieferten Laufzeitkataloge aktualisieren. Englisches Markdown ist
  maßgeblich; übersetztes Markdown ist ergänzend.

## Lokale Prüfungen

Im Quellrepository, mit installierter festgeschriebener Toolchain:

```bash
cargo fmt --all --check
cargo test --locked --all-targets
./tools/check_blueprint_core_sync.sh
python3 tools/check_public_tree.py
```

Der gemeinsame Kern besitzt eine eigene Unit-Testsuite:

```bash
cargo test --locked --manifest-path crates/dbwarp-blueprint-core/Cargo.toml --lib
```

Erfolgreiche lokale Tests sind keine Qualifikation von Datenbankversionen oder
Plattformen. Beschreiben Sie, was tatsächlich getestet wurde, und kennzeichnen
Sie andere Konfigurationen ausdrücklich als ungetestet. Veröffentlichen Sie
im Rahmen eines Patches keine Artefakte, aktualisieren Sie keine Release-Tags
und ändern Sie keine Repository-Sicherheitseinstellungen ohne Zustimmung der
Maintainer.

## Arbeitsablauf für Maintainer

Die kanonische Quelle besteht aus der englischen Rust-Hilfe und den Meldungs-/UI-Definitionen in [`src/i18n.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/i18n.rs). Wenn sich eine kundensichtbare Formulierung ändert:

1. aktualisieren Sie im selben Commit jeden Gebietsschemakatalog unter `locales/`;
2. bewahren Sie alle Platzhalter und kanonischen betrieblichen Token exakt auf;
3. führen Sie den fokussierten Test auf exakte Abdeckung aus;
4. fügen Sie den zugehörigen Fall an der Bedienergrenze in `tests/cli_errors.rs` hinzu oder aktualisieren Sie ihn, wenn sich ein Fehler oder eine Warnung ändert;
5. führen Sie die vollständige Testsuite aus und prüfen Sie repräsentative Hilfe-/Präsentationsausgaben;
6. holen Sie eine muttersprachliche technische Prüfung ein, bevor Sie neue Formulierungen als endgültig für einen Kundenvertrag, eine regulatorische Einreichung oder öffentliches Marketing behandeln.

Dieser Arbeitsablauf zur exakten Abdeckung gilt für die in die Binärdatei
eingebetteten Laufzeitkataloge. Übersetztes Markdown ist ergänzend; siehe
[`docs/TRANSLATIONS.md`](../TRANSLATIONS.md).

Fokussierte Validierung:

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

Die Integrationstests weisen außerdem nach, dass Optionstoken in allen Sprachen identisch sind, lokalisierte DBP-Codes stabil bleiben, ausgegebenes TOML sprachunabhängig ist und die erzeugte Präsentationsprosa das ausgewählte Gebietsschema trägt.
