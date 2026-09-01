# Batch-Erfassung und Blueprint-Bundles

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../BATCH_AND_BUNDLES.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../BATCH_AND_BUNDLES.md) | **Deutsch** | [Français](../fr/BATCH_AND_BUNDLES.md) | [Español](../es/BATCH_AND_BUNDLES.md) | [Polski](../pl/BATCH_AND_BUNDLES.md) | [日本語](../ja/BATCH_AND_BUNDLES.md) | [中文](../zh/BATCH_AND_BUNDLES.md)

`dbwarp-blueprint` unterstützt sowohl Blueprint-Dateien für eine einzelne Quelle als auch Bundle-Verzeichnisse mit mehreren Quellen.

Verwenden Sie eine einzelne Datei `blueprint.toml`, wenn der Kunde eine Datenbank, eine Teilmenge von Tabellen, eine Parquet-Datei oder eine Avro-Datei weitergibt. Verwenden Sie ein Bundle, wenn der Kunde mehrere Datenbanken oder mehrere strukturierte Dateidatensätze besitzt oder ein einziges Prüfpaket für eine gesamte Datenbanklandschaft wünscht.

## Bundle-Aufbau

Ein Batch-Lauf schreibt ein Verzeichnis:

```text
customer-blueprint-bundle/
  bundle.toml
  blueprints/
    erp_pg.blueprint.toml
    billing_mysql.blueprint.toml
    orders_parquet.blueprint.toml
  audits/
    erp_pg.audit.txt
    billing_mysql.audit.txt
    orders_parquet.audit.txt
```

`bundle.toml` enthält Metadaten auf Quellenebene und relative Pfade zu untergeordneten Blueprint-Dateien. Dies ist die bevorzugte Arbeitsform, da jede Quelle unabhängig prüfbar, auditierbar und erneut ausführbar bleibt.

Packen Sie das Verzeichnis für eine separat geprüfte Übergabe in ein einziges eingebettetes TOML:

```bash
dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

Die gepackte Form bettet jeden untergeordneten Blueprint unter seinem Quelleneintrag ein. Sie behält die vom Bediener angegebenen Quell-IDs, Tags, Datensatzgruppen-IDs und Auditpfad-Metadaten bei; verwenden Sie daher anonyme Manifestwerte und prüfen Sie die gepackte Datei vor der Übertragung. Das Arbeitsverzeichnis lässt sich leichter prüfen, enthält aber auch detaillierte Auditprotokolle und gegebenenfalls `errors.txt`; übertragen Sie es standardmäßig nicht vollständig.

## Bundle-Vertrag

Aktuelle Bundles verwenden `schema_version = 3` und
`kind = "dbwarp-blueprint-bundle"`. Ein Verzeichnis-Bundle verweist mit
`blueprint_path` auf jeden untergeordneten Blueprint; ein gepacktes Bundle
bettet ihn unter `blueprint` ein. Writer geben ausschließlich diese kanonischen
Bezeichner aus.

Reader akzeptieren zusätzlich die Bundle-Schemata v1 und v2. Diese Verträge
dienen ausschließlich der Eingabekompatibilität: Ein akzeptiertes altes Bundle
wird auf v3 normalisiert und nie mit früheren Bezeichnern ausgegeben. Da alte
Bundles nicht angeben, ob Quellen unabhängig, Replikate oder Shards sind, wird
ihre Beziehung `unknown` und die quellenübergreifenden Summen werden
unterdrückt. Untergeordnete Pfade müssen relativ sein und nach der
Kanonisierung innerhalb des Bundle-Verzeichnisses bleiben.

Bundle v3 trennt physische Erfassungsquellen von logischen Datensätzen. Jede
Quelle besitzt `dataset_relationship`, `dataset_group` und
`dataset_scope_completeness`. Die oberste Tabelle `dataset_groups` erfasst
Beziehung, Mitgliedschaft und Vollständigkeit der deklarierten Mitglieder.

Die Aggregation arbeitet ausfallsicher:

- `independent`: genau eine Quelle je Gruppe; Summen werden einmal addiert.
- `replica`: übereinstimmende Kopien zählen einmal. Bei Abweichung bleibt ein
  deterministischer Vertreter erhalten; Werte werden nie gemittelt und das
  Ergebnis ist unvollständig.
- `shard`: Mitglieder werden nur addiert, wenn `members_complete = true` und
  alle deklarierten Mitglieder erfolgreich waren. Eine unvollständige Gruppe
  trägt keine Summen bei.
- `unknown`: alle quellenübergreifenden Tabellen-, Zeilen- und Byte-Summen
  werden unterdrückt.
- Jede Quelle mit unvollständigem oder unbekanntem `[dataset_scope]` markiert
  auch bei bekannter Beziehung die Aggregatnachweise als unvollständig.

Quellsummen bleiben immer erhalten. Die Unterdrückung betrifft nur das
quellenübergreifende Aggregat und verhindert damit eine Multiplikation von
Replikaten oder die Darstellung einer Teilmenge von Shards als Gesamtdatensatz.

## Batch-Manifest

Erstellen Sie ein kundeneigenes Manifest:

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
dataset_relationship = "independent"
tags = ["critical", "erp"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
dataset_relationship = "independent"
tags = ["billing"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
dataset_relationship = "independent"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
dataset_relationship = "independent"
tags = ["lake"]
```

Fehlt die Beziehung, ist der Standard `unknown`; der Lauf gelingt, gibt jedoch
`DBP1414W` und `DBP1417W` aus und unterdrückt Aggregatsummen. Das ist sicherer
als anzunehmen, zwei Endpunkte seien zwei unabhängige Datensätze.

Deklarieren Sie Replikatmitglieder mit einer gemeinsamen Gruppe:

```toml
[[source]]
id = "orders_primary"
kind = "postgresql"
connect_env = "ORDERS_PRIMARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true

[[source]]
id = "orders_secondary"
kind = "postgresql"
connect_env = "ORDERS_SECONDARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true
```

Bei Shard-Systemen listen Sie jeden bekannten Shard in einer gemeinsamen
Gruppe auf und setzen `dataset_group_complete = true` nur, wenn das Manifest
den vollständigen logischen Datensatz aufzählt. Ein fehlgeschlagenes Mitglied
macht diese Gruppe im Lauf unvollständig.

Führen Sie zuerst einen Probelauf aus:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Führen Sie den Batch aus:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Ein Batch, der kein Probelauf ist, erfordert `--yes`, weil er Verbindungen zu mehreren Datenbanken herstellen oder Stichproben aus strukturierten Dateien dekodieren kann. Jede untergeordnete Quelle erhält ihre eigene Auditdatei.

Mit `continue_on_error = true` werden die übrigen Quellen verarbeitet und das diagnostische Bundle einschließlich `errors.txt` atomar veröffentlicht. Der Befehl endet dennoch mit Fehler: `DBP1115E`, wenn alle Quellen fehlschlugen, und `DBP1116E` bei teilweisem Fehler. Ein partielles Bundle ist ein Nachweis zur Prüfung und Wiederholung, keine erfolgreiche vollständige Erfassung.

Sowohl der Probelauf als auch die Ausführung validieren das vollständige Manifest,
bevor eine Quelle berührt wird. Unbekannte Felder, doppelte IDs, nach sicherer
Dateinamen-Normalisierung kollidierende IDs, nicht unterstützte Felder für den
jeweiligen Quellentyp, mehrdeutige Datenbank-Verbindungsquellen, ungültige
Datensatzmodi und Kompressions-Sampling mit einem Budget von null werden
abgelehnt. Jede `source.id` muss eindeutig, ohne führende oder nachgestellte
Leerzeichen und nach der Normalisierung höchstens 120 ASCII-Bytes lang sein.

## Datensatzmodi für strukturierte Dateien

Für Parquet- und Avro-Quellen:

- `single_file` erfordert genau eine aufgelöste Datei und behandelt sie als eine logische Tabelle.
- `one_table_per_file` ordnet jede Datei einer eigenen bereinigten Tabelle in einer untergeordneten Blueprint-Datei zu.
- `merge_same_schema` führt viele Dateien zu einer logischen Tabelle zusammen, wenn ihre Spaltenanzahlen übereinstimmen.
- `partitioned_dataset` verwendet derzeit dasselbe Zusammenführungsverhalten wie `merge_same_schema`; der Wert reserviert die semantische Unterscheidung für die Erkennung von Partitionen im Hive-Stil.

Die Zusammenführungsprüfung ist absichtlich konservativ. Sie erfordert eine
übereinstimmende anonymisierte Spaltenanordnung, kanonische und native Typen,
NULL-Zulässigkeit, deklarierte Breiten, Präzision und Skalierung, vorzeichenlose
und `BIT(n)`-Semantik, Zeitstempelpräzision, Zeichensatz und Sortierung sowie
strukturierte Quellsemantik. Gruppieren Sie Datensätze für risikoreiche
Data-Lake-Planungen auch dann nach bekanntem Schema, wenn diese strukturelle
Prüfung erfolgreich ist.

## Bundle-Operationen

Quellen auflisten:

```bash
dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

Die ersten Zeilen melden `aggregation`, physische `sources`,
`logical_datasets`, Aggregatsummen und `limitations`. Gruppenzeilen zeigen
`relationship`, `members_complete` und Quellen-IDs. Quellzeilen zeigen
`dataset_relationship`, `dataset_group` und `dataset_scope`. Behandeln Sie
`aggregation=suppressed` als Aufforderung, das Manifest zu prüfen oder zu
korrigieren, nicht als Datenbestand der Größe null.

Eine mit einem Tag gekennzeichnete Teilmenge der Quellen auflisten:

```bash
dbwarp-blueprint \
  --bundle-list customer-blueprint-bundle/bundle.toml \
  --select tag=erp
```

Eine Quelle extrahieren:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Eine Tabelle aus einer Quelle extrahieren:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Unterstützte Selektorschlüssel sind:

- `source=ID`
- `table=ID`
- `engine=postgresql|mysql|sqlserver|parquet|avro`
- `tag=NAME`

Selektoren können als eine durch Kommas getrennte Zeichenfolge oder als wiederholte Flags `--select` übergeben werden. Widersprüchliche Werte für denselben Schlüssel werden abgelehnt.

## Nachgelagerte Übergabe

Ein Bundle ist eine portable, prüfbare Blueprint-Eingabe. Bevor ein nachgelagerter Verbraucher es akzeptiert, muss er den Bundle-Vertrag und die Schemaversionen validieren, die aufgezeichneten Selektoren anwenden und beim Kombinieren mehrerer Kinder die Quellen-IDs bewahren, damit Tabellen-IDs nicht kollidieren können. Befehle und Kompatibilitätsregeln für andere DBWarp-Produkte gehören in deren separat geprüfte Dokumentation und werden hier bewusst nicht dupliziert.

## Datenschutz- und Prüfgrenze

Ein Bundle lockert das Datenschutzmodell nicht:

- Live-Datenbankquellen geben weiterhin bereinigte Tabellen-, Spalten- und Index-IDs aus;
- Werte aus strukturierten Dateien werden nur dekodiert, wenn `--measure-compression --yes` aktiviert ist;
- dekodierte Stichproben verbleiben im Arbeitsspeicher;
- Bundle-Metadaten verwenden vom Kunden gewählte Quellen-IDs und Tags;
- kein Bundle-Befehl sendet Telemetrie oder lädt Dateien hoch.

Der Kunde kann vor der Weitergabe des Bundles jeden untergeordneten Blueprint oder jeden Quelleneintrag entfernen.
