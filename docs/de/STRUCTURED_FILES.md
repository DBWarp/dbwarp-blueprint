# Blueprint-Quellen aus strukturierten Dateien

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../STRUCTURED_FILES.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../STRUCTURED_FILES.md) | **Deutsch** | [Français](../fr/STRUCTURED_FILES.md) | [Español](../es/STRUCTURED_FILES.md) | [Polski](../pl/STRUCTURED_FILES.md) | [日本語](../ja/STRUCTURED_FILES.md) | [中文](../zh/STRUCTURED_FILES.md)

`dbwarp-blueprint` kann ein begrenztes, anonymisiertes Blueprint-TOML aus
lokalen Parquet- und Avro-Eingaben erstellen, wenn die Quelle bereits als Datei
und nicht als Live-Datenbank vorliegt.

Dies ist ein Offline-Modus:

- keine Datenbankverbindung;
- keine Anmeldedaten;
- keine Telemetrie;
- keine Zeilenwerte in der Ausgabe;
- Tabellen- und Spaltenbezeichner ausschließlich als `table-NNN` und `col-N`;
- das Audit zeichnet lokale Ein-/Ausgabepfade, den Ausgabe-Hash und normale
  Betriebsnachweise wie Modus, Zeitablauf, Stichprobenarbeit und Warnungen auf;
  in diesem Modus wird kein Datenbankendpunkt aufgezeichnet.

## Parquet

```bash
dbwarp-blueprint \
  --from-parquet /data/orders.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

Der Parquet-Modus liest Footer- und Zeilengruppenmetadaten. Daraus werden abgeleitet:

- die Zeilenanzahl aus den Dateimetadaten;
- Spaltentypbezeichnungen aus den physischen/logischen Parquet-Typen;
- NULL-Fähigkeit aus den Definitionsebenen;
- beobachtete NULL-Anteile, wenn vollständige Spaltenstatistiken verfügbar sind;
- grobe kodierte mittlere Breite und Speicherverhältnis je Spalte aus den Metadaten der Spalten-Chunks;
- Quellobjektbytes, Row-Group-Anzahl, Partitionsanzahl und Codec-Herkunft.

Eine reine Metadatenerfassung von Parquet erfindet keine dekodierte p95-Breite.
Die optionale dekodierte Stichprobe ersetzt Hinweise zur kodierten Breite durch
beobachtete Werte für `len_avg`, `len_p95`, `null_fraction` und logische
`table_bytes`.

Parquet ohne dekodierte Stichprobe verwendet unkomprimierte Spalten-Chunk-Bytes
als logische Größenannahme `table_bytes`. `ratio_storage` auf Tabellenebene
vergleicht diesen Wert mit der tatsächlichen Objektgröße; `ratio_storage` auf
Spaltenebene vergleicht unkomprimierte und komprimierte Chunk-Bytes. Dies sind
Dateiplanungssignale, keine DBWarp-Transportkompression, und sie werden niemals
als `ratio_zstd_3` ausgegeben.

## Avro

```bash
dbwarp-blueprint \
  --from-avro /data/events.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Avro-Objektcontainer stellen keine Zeilenanzahl aus einem Parquet-artigen Footer bereit. Der Avro-Modus durchläuft daher den Container einmal, um Datensätze zu zählen, logische `table_bytes` abzuleiten und `len_avg`, `len_p95` sowie `null_fraction` je Spalte zu beobachten. Das Writer-Schema liefert die Metadaten der logischen Typen. `storage_bytes` und `ratio_storage` beschreiben den Avro-Container, nicht eine DBWarp-Übertragungsschätzung.

## Treue der logischen Typen

Die strukturierte Dateierfassung speichert begrenzte logische Metadaten, die für die Dimensionierung erforderlich sind: Dezimalzahlen precision/scale, Datums- und Zeitformate, Zeitstempelgenauigkeit und UTC/local-Semantik, UUIDs, binäre Daten fester Größe, UTF-8-Strings und rohe Bytes. Felder, die nur Nullwerte enthalten, bleiben `type = "null"` anstatt in synthetischen Text umgewandelt zu werden.

Verschachtelte Parquet-Elemente und Avro-Arrays, -Maps, -Records oder mehrtypige Unions können nicht als ein einzelner, exakter SQL-Skalar dargestellt werden. Das Blueprint speichert einen normalisierten `json`-Typ plus `source_semantics` wie z.B. `"repeated-leaf"`, `"nested-json"` oder `"multi-type-union"`. Diese Spalten werden als JSON formatiert; das verschachtelte Schema wird nicht exakt reproduziert.

Dateistämme, Parquet-Pfade, Avro-Feldnamen und `logical_table`-Bezeichnungen aus
einem Batch werden nicht als Blueprint-Bezeichner geschrieben. Ein
Mehrdatei-Datensatz gibt geheimschlüsselbasierte `table-NNN`-Bezeichner aus,
aggregiert Objektbytes, Partitionen, Row Groups, Codecs, Breiten, NULL-Anteile
und kompatible Kompressionsherkunft und weist Dateien zurück, deren logische
Spaltenverträge voneinander abweichen.

## Dekodierte Komprimierungsstichprobe

Der Modus für strukturierte Dateien unterstützt eine optionale dekodierte Komprimierungsstichprobe:

```bash
dbwarp-blueprint \
  --from-parquet /data/orders.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Dieselben Flags funktionieren mit `--from-avro`.

Bei Aktivierung führt `dbwarp-blueprint` Folgendes aus:

- dekodiert bis zu `--sample-rows` Datensätze aus der Datei;
- kodiert Stichprobenwerte mit derselben flüchtigen Darstellung
  `blueprint-compression-probe-v2`, die für die Blueprint-Erfassung aus
  Live-Datenbanken verwendet wird;
- gibt Komprimierungszusammenfassungen für zstd-3 auf Tabellen- und Spaltenebene aus;
- schreibt `sample_encoding = "blueprint-compression-probe-v2"` in das erzeugte TOML;
- hält beprobte Bytes ausschließlich im Arbeitsspeicher und schreibt niemals Zeilenwerte auf den Datenträger.

`--measure-compression` benötigt `--yes`, da es entschlüsselte Datenwerte liest. Es speichert aggregierte Kompressions-, Null-Dichte-, cardinality/frequency-Werte, Längen und Stile, niemals Stichprobenwerte.

Der aktuelle Sampler verwendet eine deterministische Stichprobe der ersten N Elemente. Dies ist reproduzierbar und kostengünstig, kann aber verzerrt sein, wenn eine Datei sortiert oder gruppiert ist. Für wichtige Schätzungen ist es besser, eine repräsentative Datei zu verwenden oder mehrere Blueprint-Dateien aus verschiedenen Partitionen zu erstellen.

## Umfang

Der Blueprint-Modus für strukturierte Dateien eignet sich für:

- die Dimensionierung eines Parquet-/Avro-Imports vor einem DBWarp-Lauf;
- die Planung eines Parquet/Avro-Datenbanktransfers.

Er ersetzt keine Blueprint-Erfassung aus einer Live-Datenbank, wenn die tatsächliche Quelle eine unterstützte Datenbank ist, also PostgreSQL, MySQL oder SQL Server. Ein Datenbankkatalog enthält Details zu Indizes, Schlüsseln, Fremdschlüsseln, Aktualität von Statistiken und Engine-Layout, die in generischen Dateimetadaten nicht vorhanden sind.
