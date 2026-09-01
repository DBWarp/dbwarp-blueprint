# Internationalisierung

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../INTERNATIONALISATION.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../INTERNATIONALISATION.md) | **Deutsch** | [Français](../fr/INTERNATIONALISATION.md) | [Español](../es/INTERNATIONALISATION.md) | [Polski](../pl/INTERNATIONALISATION.md) | [日本語](../ja/INTERNATIONALISATION.md) | [中文](../zh/INTERNATIONALISATION.md)

`dbwarp-blueprint` trennt die menschenlesbare Darstellung von der betrieblichen Syntax. Dies ist eine Sicherheits- und Automatisierungsgrenze, nicht nur eine Anzeigepräferenz.

## Unterstützte Sprachen

Der englische Quelltext ist maßgeblich. Die nicht englischen Darstellungskataloge sind maschinell unterstützt und können trotz validierter Schlüssel- und Tokenabdeckung Fehler enthalten. Gleichen Sie sicherheitsbezogene, vertragliche, regulatorische und Least-Privilege-Entscheidungen mit dem englischen Text ab. Siehe [`TRANSLATIONS.md`](../TRANSLATIONS.md) für die separate Freigabe übersetzter Dokumente.

| Wert | Sprache | In erzeugten Präsentationen verwendetes Gebietsschema-Tag |
|---|---|---|
| `en` | Englisch | `en-US` |
| `de` | Deutsch | `de-DE` |
| `fr` | Französisch | `fr-FR` |
| `es` | Spanisch | `es-ES` |
| `pl` | Polnisch | `pl-PL` |
| `ja` | Japanisch | `ja-JP` |
| `zh` | Vereinfachtes Chinesisch | `zh-CN` |

Wählen Sie ausdrücklich eine Sprache:

```bash
dbwarp-blueprint --lang de --help
dbwarp-blueprint --lang ja --connect postgresql://db.internal/app --dry-run
```

Wenn `--lang` fehlt, gilt folgende Auflösungsreihenfolge:

1. `DBWARP_BLUEPRINT_LANG`;
2. `LC_ALL`;
3. `LC_MESSAGES`;
4. `LANG`;
5. Englisch.

Bei Gebietsschema-Tags aus der Umgebung werden Regions- und Codierungssuffixe akzeptiert; `de_CH.UTF-8`, `pl_PL.UTF-8` und `ja-JP` werden daher auf ihre Basissprache aufgelöst. Explizite Werte für `--lang` sind bewusst auf die sieben kanonischen Token in der Tabelle begrenzt.

Unter Windows sind `LC_ALL`, `LC_MESSAGES` und `LANG` normalerweise nicht gesetzt. Das Werkzeug verwendet daher Englisch, sofern nicht `--lang` oder `DBWARP_BLUEPRINT_LANG` gesetzt wird, zum Beispiel `$env:DBWARP_BLUEPRINT_LANG = "de"` in PowerShell oder `set DBWARP_BLUEPRINT_LANG=de` in cmd. Umgebungsvariablennamen sind unter Windows nicht, unter Linux und macOS jedoch sehr wohl groß-/kleinschreibungssensitiv; verwenden Sie immer die kanonischen Großbuchstaben.

## Was übersetzt wird

- Beschreibungen der obersten Hilfeebene und der Optionen;
- Hilfsrahmen wie Beschriftungen für Verwendung und mögliche Werte;
- Vorabpläne und Zustimmungsaufforderungen;
- Zusammenfassung, Ursache und Korrekturmaßnahme von DBP-Meldungen;
- Fortschritts- und Warntexte;
- Überschriften, Beschriftungen, Erläuterungen und Gebietsschemametadaten der PowerPoint-Präsentation.

Schwerwiegende technische Details können für die Diagnose unverändert unter der lokalisierten DBP-Meldung stehen. Nicht schwerwiegende Datenbankwarnungen schwärzen rohe Treiberdetails, wenn sie Quellkennungen enthalten könnten; der stabile DBP-Code und das anonyme Blueprint-Ziel bleiben erhalten.

## Was sich niemals ändert

Folgendes bleibt in jeder Anzeigesprache ein kanonisches englisches Token:

- der Befehl `dbwarp-blueprint` und Optionsnamen wie `--measure-compression`;
- zulässige Werte wie `verify-full`, `balanced` und `exact`;
- URI-Schemata wie `postgresql://`, `mysql://` und `sqlserver://`;
- Namen von Umgebungsvariablen und Dateipfade;
- Selektoren wie `source=ID` und `table=ID`;
- DBP-Bezeichner wie `DBP1001E`;
- anonymisierte Bezeichner wie `table-001`, `col-1` und `schema-A`;
- Audit-Schlüssel, TOML-Schlüssel, Bundle-Schlüssel, Datenbanktypnamen und Indexmethoden.

Folglich benötigen Skripte keine sprachspezifische Behandlung von Optionen oder Werten, und ein unter `--lang ja` erzeugter Blueprint ist byteidentisch mit einem unter `--lang en` erzeugten, wenn alle anderen deterministischen Eingaben gleich sind.

## Strenges Katalogverhalten

Alle Kataloge sind in die Binärdatei einkompiliert. Beim Start prüft das Programm, ob jedes angegebene nicht englische Gebietsschema Folgendes exakt abdeckt:

- den aktuellen Live-Clap-Hilfebaum;
- jeden stabilen DBP-Code und alle drei Diagnosefelder;
- jeden stabilen Schlüssel für Eingabeaufforderungen, Fortschritt, Warnungen und Präsentationen;
- jeden erforderlichen Platzhalter und jedes geschützte betriebliche Token.

Bei fehlenden oder zusätzlichen Einträgen, geänderten Platzhaltern, veränderten betrieblichen Token, ungültigem JSON oder unsichtbaren/bidirektionalen Formatsteuerzeichen bricht das Programm mit `DBP1010E` sicher ab. Es ersetzt eine fehlende Übersetzung nicht stillschweigend durch Englisch.

## Arbeitsablauf für Maintainer

Die kanonische Quelle besteht aus der englischen Rust-Hilfe und den Meldungs-/UI-Definitionen in [`src/i18n.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/i18n.rs). Wenn sich eine kundensichtbare Formulierung ändert:

1. aktualisieren Sie im selben Commit jeden Gebietsschemakatalog unter `locales/`;
2. bewahren Sie alle Platzhalter und kanonischen betrieblichen Token exakt auf;
3. führen Sie den fokussierten Test auf exakte Abdeckung aus;
4. fügen Sie den zugehörigen Fall an der Bedienergrenze in `tests/cli_errors.rs` hinzu oder aktualisieren Sie ihn, wenn sich ein Fehler oder eine Warnung ändert;
5. führen Sie die vollständige Testsuite aus und prüfen Sie repräsentative Hilfe-/Präsentationsausgaben;
6. holen Sie eine muttersprachliche technische Prüfung ein, bevor Sie neue Formulierungen als endgültig für einen Kundenvertrag, eine regulatorische Einreichung oder öffentliches Marketing behandeln.

Fokussierte Validierung:

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

Die Integrationstests weisen außerdem nach, dass Optionstoken in allen Sprachen identisch sind, lokalisierte DBP-Codes stabil bleiben, ausgegebenes TOML sprachunabhängig ist und die erzeugte Präsentationsprosa das ausgewählte Gebietsschema trägt.
