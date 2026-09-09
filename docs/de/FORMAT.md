# DBWarp Blueprint-Dateiformat v6

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../FORMAT.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../FORMAT.md) | **Deutsch** | [Français](../fr/FORMAT.md) | [Español](../es/FORMAT.md) | [Polski](../pl/FORMAT.md) | [日本語](../ja/FORMAT.md) | [中文](../zh/FORMAT.md)

Menschenlesbar. Diff-fähig. Forensisch prüfbar.

> **Dieses Format reduziert das Risiko verdeckter Kanäle und direkter Offenlegung
> durch ein begrenztes Schema, mit geheimem Schlüssel erzeugte Bezeichner und dokumentierte
> numerische Genauigkeit. Anonyme Graphstrukturen und exakte Opt-in-Felder können
> weiterhin einen Workload identifizieren; prüfen Sie die Datei daher gemäß Ihrer
> eigenen Datenklassifizierungsrichtlinie.**

## Dateikopf

Wörtlich, Byte für Byte:

```
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

Die Leerzeile ist Teil des kanonischen Dateikopfs. Der Rust-Collector gibt exakt diesen Dateikopf und keine weiteren Kommentare aus. Der SQL-Fallback-Normalisierer behält ihn wörtlich bei und fügt anschließend genau einen festen Kommentar `Producer: blueprint_format.py SQL fallback` mit der Schlüsselquelle hinzu, damit Empfänger den Producer unterscheiden können. Dies ist keine Zusage, dass die verbleibenden strukturierten Felder kein unverwechselbares Schema oder keinen Abhängigkeitsgraphen identifizieren können.

## Felder auf oberster Ebene

| Feld | Typ | Beschreibung |
|---|---|---|
| `schema_version` | int | Formatversion. Derzeit `6`; Versionen 1 bis 5 bleiben lesbar. |
| `generated_at` | ISO-8601 string | UTC-Zeitstempel mit Sekundengenauigkeit ohne Nachkommastellen. Über das CLI-Flag `--generated-at "2026-04-26T00:00:00Z"` **festschreibbar**. Byteidentische Live-Erfassungen erfordern außerdem denselben geschützten `--anonymization-key-file`, Quellzustand, dieselben Optionen und denselben Producer. Das Auditprotokoll zeichnet bei gesetztem Flag stets `generated_at_pin: ...` auf, sodass die Festschreibung forensisch sichtbar ist. Keine Umgebungsvariable schreibt diesen Wert fest. |
| `engine` | string | `"postgresql"`, `"mysql"`, `"sqlserver"`, `"parquet"` oder `"avro"`. |
| `engine_version` | string | Schmale numerische Produktversion für versionsabhängige Erfassung und Generierung aus Live-Datenbanken; bei strukturierten Dateiquellen leer. Producer- und Distributionsbanner sind ausgeschlossen. |
| `source_kind` | string | Einer der Werte `"production"`, `"staging"`, `"scrubbed-replica"`, `"synthetic"`. Vom Kunden angegeben. |
| `length_metadata` | string | Legacy-Kompatibilitätskennzeichnung: `"hybrid-v2"`, `"exact"`, `"rounded"` oder `"not-captured"`. Neue Verbraucher müssen die drei nachfolgenden Felder verwenden. |
| `declared_length_fidelity` | string | `"exact"` für deklarierte PostgreSQL-Zeichenkapazitäten und die standardmäßigen ausgewogenen/exakten MySQL-Modi, `"coarse-rounded-v1"` für strikten MySQL-Datenschutz oder `"not-captured"`, wenn nicht verfügbar. |
| `index_length_fidelity` | string | `"exact"` für standardmäßig ausgewogene/exakte MySQL-Indexpräfixe, `"rounded-down-v1"` für strikten Datenschutz oder `"not-captured"`, wenn nicht verfügbar. |
| `observed_length_fidelity` | string | Standardmäßig `"relative-rounded-v2"`, wenn beprobt, `"exact"` im exakten Modus, `"coarse-rounded-v1"` im strikten Modus oder `"not-sampled"`. Die Stichprobenabdeckung bleibt eine separate Anforderung je Spalte. |
| `[totals]` | inline table | Aggregierte Anzahlen (siehe unten). |
| `[network]` | table | Optionaler Nachweis für Client-zu-Datenbank-Verbindung und Abfrage-RTT. |
| `[database_topology]` | table | Für Datenbankquellen mit Schema v6 erforderlich. Begrenzte, namenfreie Angaben zu Deployment, lokaler Rolle, Sichtbarkeit und Katalognachweisen. Bei strukturierten Dateien nicht vorhanden. |
| `[dataset_scope]` | table | Für jede Blueprint mit Schema v6 erforderlich. Gibt an, was die Summen abdecken und ob Tabellen-, Zeilen- und Byteabdeckung vollständig sind. |
| `[tables.X]` | tables | Eine je Tabelle, mit anonymisierter ID. |
| `[fk_edges]` | inline table | Fremdschlüsselgraph zwischen anonymisierten Tabellen. Optional. |
| `[artifact_inventory]` | table | Begrenzte, namenfreie Anzahlen von Nicht-Tabellenobjekten, optionaler anonymer Abhängigkeitsgraph, externe Voraussetzungen und optionale begrenzte Sprachanalyse. Nur für Datenbankquellen. |

## `[totals]`

| Feld | Typ | Genauigkeit |
|---|---|---|
| `table_count` | int | exakt |
| `row_count` | int | Summe der je Tabelle gerundeten Werte `rows` |
| `table_bytes` | int | Summe der je Tabelle gerundeten Werte `table_bytes` |
| `index_bytes` | int | Summe der je Tabelle gerundeten Werte `index_bytes` |

Diese Zahlen sind nicht automatisch Summen des gesamten Clusters. Sie müssen
immer zusammen mit `[dataset_scope]` interpretiert werden. Ein Shard-Gateway
oder Koordinator kann einen vollständig wirkenden Katalog anzeigen, ohne die
zugrunde liegenden Shards zu halten. Schema v6 stellt diese Unsicherheit
explizit dar, statt lokale Katalogstatistiken stillschweigend als globale
Wahrheit zu behandeln.

## `[database_topology]` (Datenbankquellen mit Schema v6)

Dieser Block speichert nur begrenzte Fakten, die über den verbundenen
Datenbankendpunkt sichtbar sind. Er speichert niemals Knotennamen, Hostnamen,
IP-Adressen, Clusternamen, Replikationskanalnamen, Serverkennungen oder
Endpunkte.

| Feld | Werte / Regel |
|---|---|
| `contract` | Immer `dbwarp-blueprint-topology/v1`. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed` oder `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member` oder `unknown`. |
| `visibility` | `full`, `partial` oder `unknown`; beschreibt Topologienachweise, nicht die Datenrichtigkeit. |
| `member_count` | Anzahl der durch erfolgreiche Nachweisabfragen sichtbaren Mitglieder. `0` bedeutet unbekannt, niemals null Mitglieder. |
| `identifiers_redacted` | Muss `true` sein. |
| `role_counts` | Optionale Anzahlen nach geschlossenem Rollentoken. Volle Sichtbarkeit verlangt, dass diese Anzahlen `member_count` entsprechen. |
| `features` | Sortierte geschlossene Token wie `citus`, `mysql-group-replication`, `mysql-galera`, `mysql-ndb`, `postgresql-streaming-replication`, `sqlserver-availability-group` oder `vitess`. |
| `catalogs_read` | Sortierte geschlossene Bezeichnungen für erfolgreich gelesene Topologiekataloge. |
| `catalogs_unreadable` | Sortierte geschlossene Bezeichnungen für nicht lesbare Topologiekataloge. Jeder Eintrag verhindert die Aussage voller Sichtbarkeit. |

Ein gewöhnlicher Endpunkt darf `deployment = "unknown"` melden und dennoch
vollständige lokale Statistiken einer Vollkopie liefern. Blueprint nimmt nicht
an, dass ein unauffälliger Server single-node ist, nur weil keine
Clusterfunktion sichtbar war.

## `[dataset_scope]` (Schema v6)

Dieser Block qualifiziert jede Dimension der Größenangaben unabhängig.
Verbraucher müssen unqualifizierte Berechnungen über den gesamten Datensatz
ablehnen, wenn eine erforderliche Vollständigkeitsdimension `incomplete` oder
`unknown` ist.

| Feld | Werte / Regel |
|---|---|
| `contract` | Immer `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset` oder `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete` oder `unknown`. |
| `row_count_completeness` | `complete`, `incomplete` oder `unknown`. |
| `size_completeness` | `complete`, `incomplete` oder `unknown`. |
| `row_count_method` | Geschlossenes Herkunftstoken wie `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter` oder `distributed-aggregate`. |
| `size_method` | Geschlossenes Herkunftstoken wie `postgres-local-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `citus-distributed-relation-size` oder `distributed-aggregate`. |
| `limitations` | Sortierte geschlossene Gründe für unvollständige oder unbekannte Abdeckung. Mindestens einer ist erforderlich, sofern nicht alle Dimensionen vollständig sind. |

`selection-limited` bedeutet, dass Summen und Vollständigkeitsangaben genau die über den wiederholbaren Live-Selektor `--schema` angeforderten Schemata abdecken; sie beanspruchen keine Abdeckung der gesamten verbundenen Datenbank. Ohne `--schema` bleibt das Erfassungsverhalten für alle sichtbaren Schemata erhalten.

Die nativen PostgreSQL-, MySQL- und SQL-Server-Kollektoren prüfen unterstützte
Topologiekataloge, bevor sie entscheiden, ob lokale Statistiken den logischen
Datensatz darstellen können. Bekannte verteilte Gateways unterdrücken unsichere
Summen, wenn kein verlässliches Aggregat verfügbar ist. Der SQL-Fallback hat
keine Topologieprüfung und gibt deshalb seine nützlichen lokalen Schätzungen
mit allen Bereichsdimensionen als `unknown` sowie den Einschränkungen
`topology-unobserved` und `topology-visibility-unknown` aus.

Strukturierte Parquet- und Avro-Blueprints lassen `[database_topology]` weg und
verwenden `layout = "structured-dataset"` mit Footer-/Container-Herkunft.

Blueprint führt während einer gewöhnlichen Erfassung keinen
Speichergeschwindigkeitstest aus und leitet die Hardware des Datenbankservers
nicht von dem Rechner ab, auf dem der Client läuft. Datenbank-Byte-Summen
beschreiben das gespeicherte Datenvolumen nach der benannten Katalogmethode;
sie behaupten weder Datenträgertyp, IOPS, Durchsatz, CPU, RAM noch
Zielmigrationsleistung.

## `[network]` (optional)

Vom Kunden beobachtete Netzwerk-Roundtrip-Statistiken zwischen dem Blueprint-Werkzeug und der Quelldatenbank. Dies ist **NICHT** die RTT zwischen Migrationsquelle und -ziel, sondern lediglich ein Nachweis dafür, wie weit das Blueprint-Werkzeug zur Laufzeit von der Quelldatenbank des Kunden entfernt war. Der nachgelagerte Estimator verwendet diese Werte nur als Plausibilitätsprüfung für die vom Bediener angegebene Migrations-RTT. Beispielsweise ist eine angegebene Migrations-RTT von 200 ms unplausibel, wenn die lokale Kundenmessung 0,4 ms betrug; wahrscheinlich lief das Blueprint-Werkzeug direkt auf der Quelldatenbank.

Die Messung erfolgt nach dem Verbindungsaufbau und vor den Katalogabfragen, damit die Zeiten nicht durch das Aufwärmen des Abfragecaches verfälscht werden. Sie führt **5× `SELECT 1`** aus und gibt die Medianlatenz aus. Jede Abfrage `SELECT 1` liefert ausschließlich die konstante Ganzzahl 1; bei dieser Messung werden keine Zeilendaten gelesen.

Der Block fehlt, wenn der Kunde `--no-rtt-probe` angegeben hat oder die Messung während der Ausführung fehlgeschlagen ist. Der Fehler wird als nicht schwerwiegende Warnung auf stderr und im Auditprotokoll aufgezeichnet; die Blueprint-Datei wird weiterhin ohne diesen Block ausgegeben.

| Feld | Typ | Genauigkeit |
|---|---|---|
| `sample_count` | int | exakt (in v1 immer 5) |
| `connect_total_ms` | int | Gesamte verstrichene Zeit vom Beginn des TCP-Verbindungsaufbaus bis zur Bereitschaft der authentifizierten Sitzung in Millisekunden. Umfasst TCP-Handshake, TLS-Handshake, sofern zutreffend, und Authentifizierungs-Challenge/-Response. Auf die nächste Millisekunde gerundet. Typischerweise 3–6× `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | Medianlatenz eines einzelnen Roundtrips aus den 5 Stichproben `SELECT 1` in Millisekunden. Auf die nächste Millisekunde gerundet. Das natürliche Grundrauschen des Netzwerks (in der Praxis ≥ 1 ms) ist größer als die Rundungsgranularität. Dadurch wird jeder verdeckte Kanal über niederwertige Bits beseitigt, ohne nützliche Genauigkeit zu verlieren. LAN-Werte unter einer Millisekunde fallen auf 0 oder 1 zusammen. |
| `query_rtt_ms_p95` | int | 95. Perzentil der 5 Stichproben nach der Nearest-Rank-Methode (die langsamste Beobachtung) in Millisekunden. Auf die nächste Millisekunde gerundet. Zusammen mit p50 hilft der Wert, kurze Latenzspitzen zu erkennen; fünf Stichproben dienen nur zur Orientierung und sind kein Workload-Benchmark. |

Die 5 Messabfragen erscheinen im Auditprotokoll als **ein einziger Zusammenfassungseintrag** und nicht als 5 getrennte Zeilen. Er trägt die Bezeichnung „5x SELECT 1 (RTT probe; constant integer 1, no row data)“ und entspricht damit der Vertrauenshaltung, dass keine Zeileninhalte gelesen werden.

## `[tables.<id>]`

Der Bezeichner lautet `table-NNN`, wobei `NNN` die bei 1 beginnende
Ordnungszahl in einer domänengetrennten HMAC-SHA256-Reihenfolge von Schema- und
Tabellenname ist. Standardmäßig wird der Schlüssel für jeden Prozess neu
erzeugt und niemals ausgegeben. Mit demselben kundenseitig verwahrten
`--anonymization-key-file` bleibt die Reihenfolge über genehmigte
Vergleichsläufe hinweg erhalten.

| Feld | Typ | Genauigkeit/Werte |
|---|---|---|
| `rows` | int | gerundet: auf die nächsten 100 (≤10k), 1000 (≤1M), 10000 (>1M) |
| `table_bytes` | int | nach Größenordnung gerundet: nächste 1KiB / 1MiB / 100MiB |
| `index_bytes` | int | wie `table_bytes` gerundet |
| `schema` | string | anonymisierte ID `schema-A`, `schema-B`, ..., `schema-AA` |
| `kind` | string | Optionales geschlossenes Token in Schema v6: `partitioned`, `materialized-view`, `temporal-current`, `temporal-history`, `memory-optimized`, `external`, `graph-node` oder `graph-edge`. Bei einer gewöhnlichen Tabelle oder unbekannter Evidenz nicht vorhanden. |
| `unlogged` | bool | Optionale PostgreSQL-Katalogbeobachtung in Schema v6. Nicht vorhanden, wenn nicht erfasst; explizites `false` bedeutet, dass der Katalog eine protokollierte Tabelle bestätigt hat. |
| `partition_strategy` | string | Optionales Token in Schema v6 für `partitioned`: `range`, `list`, `hash`, `key` oder `linear-hash`. |
| `partition_count` | int | Exakte positive Anzahl der Blattpartitionen in Schema v6; bei `kind = "partitioned"` erforderlich. |
| `partition_key_cols` | array of int | Spaltenordnungszahlen eines einfachen Partitionsschlüssels in Schema v6. Bei einem Ausdrucksschlüssel oder fehlender Katalogevidenz nicht vorhanden; der Schlüsselausdruck wird nie ausgegeben. |
| `partition_rows_max` | int | Optionale gerundete Schätzung der größten Blattpartition in Schema v6. |
| `temporal_history` | string | Tabellen-ID der zugehörigen `temporal-history`-Tabelle in Schema v6; bei `temporal-current` erforderlich. |
| `counted_in_totals` | bool | Schema v6. Nicht vorhanden bedeutet Einbeziehung in alle Gesamtsummen. `external` erfordert explizites `false`; die Tabelle wird dadurch aus `table_count`, `row_count`, `table_bytes` und `index_bytes` ausgeschlossen. Kein anderer expliziter Wert ist kanonisch. |
| `check_count` | int | Optionale exakte strukturelle Anzahl von CHECK-Constraints in Schema v6. Nicht vorhanden bedeutet unbekannt; `0` bedeutet, dass der relevante Katalog keine gefunden hat. |
| `has_clustered_index` | bool | für PostgreSQL immer `false` |
| `stats_freshness` | string | `"fresh"` / `"stale"` / `"never_analyzed"` für PostgreSQL, abgeleitet aus den Analysezeitstempeln in `pg_stat_all_tables`; leer für MySQL, SQL Server, strukturierte Dateien und SQL-Fallback-Pfade, die die Statistikaktualität ohne weitergehende Zugriffsrechte nicht feststellen können. MySQL `UPDATE_TIME` wird bewusst nicht verwendet, da es Änderungen an den Quelldaten beschreibt, nicht den Zeitpunkt von `ANALYZE TABLE`. |
| `[tables.<id>.cols.<cid>]` | sub-tables | eine je Spalte |
| `[tables.<id>.idxs.<iid>]` | sub-tables | eine je Index |
| `[tables.<id>.compression]` | sub-table | nur bei Tier 2 |

## `[tables.<id>.cols.<cid>]`

Der Bezeichner lautet `col-N`, wobei `N` die natürliche Attributreihenfolge der Spalte ist. Sie beginnt bei 1 und bewahrt die physische Ordnungszahl auf dem Datenträger. Der Wert ist über mehrere Läufe stabil.

| Feld | Typ | Hinweise |
|---|---|---|
| `ordinal` | int | dasselbe N wie in der ID |
| `type` | string | normalisierte Typfamilie wie `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"` oder `"user-defined"`. Echte Namen von Domains, Enums, Aliasen, zusammengesetzten und benutzerdefinierten Typen werden nicht ausgegeben. |
| `nullable` | bool | |
| `value_source` | string | Optionales geschlossenes Token in Schema v6: `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time` oder `rowversion`. Bei gewöhnlich gelieferten Werten oder unbekannter Evidenz nicht vorhanden. |
| `has_default` | bool | Optionale Katalogbeobachtung in Schema v6. Nicht vorhanden bedeutet unbekannt; explizites `false` bedeutet, dass der Katalog keinen Standardwert gefunden hat. |
| `default_kind` | string | Optionale Klassifizierung `constant`, `function` oder `expression` in Schema v6; nur mit `has_default = true` gültig. Text und Literale des Standardwerts werden nie ausgegeben. |
| `type_kind` | string | Optionales geschlossenes Token in Schema v6: `enum`, `set`, `domain`, `composite`, `array`, `range` oder `alias`. Bei einem Basistyp oder unbekannter Evidenz nicht vorhanden. |
| `member_count` | int | Exakte positive strukturelle Elementanzahl in Schema v6; nur für `enum` und `set` erforderlich. Elementnamen werden nie ausgegeben. |
| `domain_has_check` | bool | Optionale Domain-CHECK-Beobachtung in Schema v6; nur mit `type_kind = "domain"` gültig. |
| `hidden`, `masked`, `encrypted`, `sparse` | bool | Optionale Katalogbeobachtungen in Schema v6. Nicht vorhanden bedeutet unbekannt; explizites `false` bedeutet, dass der Katalog die Eigenschaft als nicht vorhanden bestätigt hat. |
| `has_check` | bool | Optionale Beobachtung eines einspaltigen CHECK in Schema v6. Jedes explizite `true` wird durch `check_count` der Tabelle abgedeckt. |
| `null_fraction` | float | Optionaler beobachteter NULL-Anteil von `0.0` bis `1.0`. Nur das gerundete Aggregat wird beibehalten; es wird keine NULL-Bitmap gespeichert. |
| `native_type` | string | Optionaler bereinigter Engine-Basistyp wie `varchar` oder `longtext`; keine Bezeichner, Enum-Mitglieder, Standardwerte oder Ausdrücke. Wird von den nativen MySQL- und SQL-Server-Collectors ausgegeben. |
| `declared_max_chars` | int | Optional deklarierte Zeichenkapazität. Exakt für Katalogwerte von PostgreSQL `character`/`character varying` und in den standardmäßigen ausgewogenen/exakten MySQL-Modi; nur bei MySQL mit `--length-fidelity strict` grob gerundet. |
| `declared_max_bytes` | int | Optional deklarierte Bytekapazität. Exakt in den standardmäßigen ausgewogenen/exakten MySQL-Modi; nur bei `--length-fidelity strict` grob gerundet. |
| `numeric_precision`, `numeric_scale`, `datetime_precision` | int | Optional von der Engine deklarierte skalare Genauigkeit. |
| `charset`, `collation` | string | Optionale bereinigte Zeichenmetadaten. MySQL gibt die Katalognamen für Zeichensatz und Sortierung aus. SQL Server gibt `utf-16le` für `nchar`/`nvarchar`/`ntext`, `utf-8` für Codepage 65001, `windows-N` für Windows-Codepages 1250–1258 oder `code-page-N` für eine andere positive Katalog-Codepage sowie den Katalognamen der Sortierung aus. Dies sind Kodierungsfakten und Katalognamen, niemals Kundenbezeichner oder -werte. |
| `len_avg` | int | Beprobte durchschnittliche Byteanzahl variabler Werte. Die standardmäßigen relativen Buckets haben einen maximalen Fehler von ungefähr 3,2 % und bewahren Werte bis 32 Byte exakt; exakt mit `--length-fidelity exact --yes`; grobe Rundung auf die nächsten 10 nur im strikten Modus. 0 = feste Länge oder nicht gemessen. |
| `len_p95` | int | Beprobtes 95. Perzentil mit denselben standardmäßigen relativen Buckets; exakt mit `--length-fidelity exact --yes`; grobe Rundung auf die nächsten 100 nur im strikten Modus. 0 = nicht gemessen. |
| `style` | string | Nur Tier 2. Einer der Werte `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"` oder `"precompressed"`; leer, wenn nicht klassifiziert. `"precompressed"` wird nur für eine materiell byte-dominierende Stichprobe binärer Werte mit erkannten Standardsignaturen von Containern ausgegeben. Die erkannte Containerfamilie wird absichtlich nicht offengelegt. |
| `magnitude_min`, `magnitude_max` | int | Optionale vorzeichenbehaftete Dezimalexponenten in Schema v6, welche die Größenordnung beprobter numerischer Nicht-NULL-Werte begrenzen. Sie werden zusammen mit `has_negative` ausgegeben; exakte Werte werden nie ausgegeben. |
| `has_negative` | bool | Optionale Beobachtung des Vorzeichens in Schema v6; nur zusammen mit beiden Größenordnungsgrenzen ausgegeben. |
| `time_span` | string | Optionaler beprobter Datums-/Zeitbereich in Schema v6: `intraday`, `days`, `weeks`, `months`, `years` oder `decades`. |
| `time_recent_decade` | int | Jahrzehnt des neuesten beprobten Datums-/Zeitwerts in Schema v6; nur mit `time_span` ausgegeben und immer durch 10 teilbar. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Nur Tier 2. Für beprobte Text-/Binärkandidatenspalten vorhanden. Derselbe Feldaufbau wie bei der Komprimierung auf Tabellenebene, aber auf eine anonymisierte Spalte begrenzt. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Zusammenfassung der Verteilung beprobter Werte in Schema v3. Enthält ausschließlich begrenzte oder gerundete Anzahlen und Häufigkeiten. |

### `[tables.<id>.cols.<cid>.cardinality]` (Schema v3)

Wenn die Zeilenstichprobe aktiviert ist, hält der Collector höchstens 8.192
temporäre 64-Bit-Fingerprints je Spalte im Speicher, leitet daraus aggregierte
NDV- und Schiefestatistiken ab und verwirft die Fingerprints. Weder Werte noch
Fingerprints werden serialisiert. Der Block enthält `measured`, `sample_rows`,
`non_null_rows`, `observed_distinct_count`, `estimated_distinct_count`,
`top_value_fraction`, `frequency_p50`, `frequency_p95`, `frequency_p99`,
`frequency_max`, `sample_method`, `sample_layout`, `sampled_with_bias` und
`bias_reason`. `sample_layout` ist eine optionale maschinenlesbare Enumeration.
Der derzeit ausgegebene Wert ist `primary-key-range-windows`; fehlt das Feld,
ist kein Ordnungsvertrag verfügbar. Verbraucher dürfen keine
Generierungssemantik aus dem menschenlesbaren Feld `sample_method` ableiten.

Anzahlen und Anteile werden, soweit erforderlich, datenschutzgerecht gerundet.
Die Statistiken sollen Duplikatdichte, die Schiefe häufig auftretender Werte und
endliche Wertebereiche in synthetischen Testdatensätzen nachbilden. Sie enthalten
keine beprobten Werte, aber auffällige Verteilungen können einen Workload
identifizieren; behandeln Sie sie nicht als unumkehrbar oder als Nachweis dafür,
dass fachliche Bedeutung nicht aus externem Wissen abgeleitet werden kann.

### `[tables.<id>.cols.<cid>.compression]` (nur Tier 2)

Die Komprimierung je Spalte wird ausschließlich für begrenzte Text-/Binärkandidaten ausgegeben, wenn `--measure-compression --yes` verwendet wird. Damit können nachgelagerte Werkzeuge synthetische Text-/Binärdaten mit einer realistischeren Entropie erzeugen, als dies allein mit Verhältnissen auf Tabellenebene möglich wäre.

Der Block enthält dieselben Felder wie `[tables.<id>.compression]`: `measured`, `sample_rows`, `sample_bytes`, `sample_method`, `sampled_with_bias`, `bias_reason`, `ratio_zstd_3`, `ratio_zstd_19`, `ratio_stddev` und `sample_encoding`.

Beispiel:

```toml
[tables.table-001.cols.col-2]
ordinal = 2
type = "json"
nullable = false
len_avg = 430
len_p95 = 0
style = "json"

[tables.table-001.cols.col-2.compression]
measured = true
sample_rows = 1000
sample_bytes = 65536
sample_method = "column TABLESAMPLE SYSTEM(0.1) LIMIT N (text format)"
sampled_with_bias = false
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "blueprint-compression-probe-v2"
```

Es werden keine beprobten Spaltenwerte in die Blueprint-Datei geschrieben.

Für Binärspalten kann dieselbe begrenzte Tier-2-Stichprobe das grobe Profil
`style = "precompressed"` ausgeben. Die Erkennung erfolgt nur an den Grenzen
der Stichprobenwerte und verlangt eine materielle, byte-dominierende
Beobachtung. Blueprint analysiert oder dekomprimiert den Wert nicht, bewahrt
seine Signatur nicht auf und unterscheidet Bild-, Archiv-, komprimierte Medien-,
verschlüsselte und zufällige Nutzdaten über diese eine verlässliche
Kennzeichnung hinaus nicht. Text- und Base64-Kodierungen werden weiterhin nach
ihrem Textstil klassifiziert und nicht als vorkomprimierte Binärcontainer
behandelt.

Der deterministische Generator bildet dieses grobe Profil auf einen neutralen,
gültigen ZIP-Container mit einem gespeicherten hochentropischen Mitglied ab.
Dadurch bleibt das betriebliche Komprimierungsverhalten erhalten, ohne zu
behaupten, dass die Quelle ZIP-Dateien enthielt, oder offenzulegen, ob der
erkannte Quellwert ein Bild-, Archiv- oder Mediencontainer war. Die gewöhnliche
Binärdatengenerierung bleibt unverändert.

## `[tables.<id>.idxs.<iid>]`

Der Bezeichner lautet `idx-N`, wobei `N` die bei 1 beginnende Ordnungszahl des Index innerhalb der Tabelle ist, sortiert nach einem domänengetrennten HMAC-SHA256 des Indexnamens.

| Feld | Typ | Werte |
|---|---|---|
| `type` | string | Normalisierte Familie von Indexmethoden wie `"btree"`, `"hash"`, `"gin"`, `"gist"`, `"brin"`, `"spgist"`, `"fulltext"`, `"spatial"`, `"clustered"`, `"nonclustered"`, `"clustered columnstore"`, `"nonclustered columnstore"` oder `"other"`. Namen von Erweiterungs-/benutzerdefinierten Methoden werden nicht ausgegeben. |
| `primary` | bool | Optional; wird für Primärschlüsselindizes als `true` ausgegeben. Andernfalls nicht vorhanden beziehungsweise false. |
| `unique` | bool | |
| `cols` | array of int | beteiligte Spaltenordnungszahlen in der Reihenfolge der Indexspalten |
| `prefix_lengths` | array of int | Optionale MySQL-Indexpräfixlängen, an `cols` ausgerichtet; null bedeutet die vollständige Spalte. Standardmäßig exakt; nur bei `--length-fidelity strict` abgerundet. |
| `include_cols` | array of int | Optional; Ordnungszahlen von Nichtschlüssel-INCLUDE-Spalten, sofern die Quell-Engine sie bereitstellt. |
| `expression` | bool | Optional; true, wenn Ausdrucks-/Funktionsschlüsselmaterial vorhanden ist und nicht als einfache Spaltenordnungszahlen dargestellt werden kann. |
| `filtered` | bool | Optional; true für gefilterte/partielle Indizes. |
| `descending` | bool | Optional; true, wenn eine Schlüsselspalte ausdrücklich absteigend sortiert ist. |
| `prefix_distinct_counts` | array of int | In Schema v3 geschätzte Anzahl unterschiedlicher Tupel für jedes Schlüsselpräfix von einer bis N Spalten. Null bedeutet, dass für dieses Präfix kein Wert verfügbar ist. |
| `cardinality_sample_method` | string | Begrenzte Herkunftsinformation für `prefix_distinct_counts`; abgeleitete Produkte sind ausdrücklich gekennzeichnet und werden nicht als direkte Tupelstichproben dargestellt. |

## `[tables.<id>.compression]` und `[tables.<id>.cols.<cid>.compression]` (nur Tier 2)

Nur vorhanden, wenn die Datei mit `--measure-compression --yes` erzeugt wurde. Der Block auf Tabellenebene misst eine neutrale spaltenorientierte Projektion der vollständigen Stichprobe und bleibt das maßgebliche Verhältnis für Schätzungen der vollständigen Tabellenübertragung. Blöcke auf Spaltenebene werden aus denselben beprobten Zeilen spaltenweise projiziert und sollen nachgelagerten Generatoren synthetischer Testdatensätze dabei helfen, die Entropie je Spalte abzustimmen, ohne Kundenwerte zu sehen. Sie lösen keine zusätzlichen Datenbanklesevorgänge aus.

| Feld | Typ | Genauigkeit |
|---|---|---|
| `measured` | bool | immer `true`, wenn der Block vorhanden ist |
| `sample_rows` | int | exakt |
| `sample_bytes` | int | Größe des Stichprobenpuffers im Arbeitsspeicher, in **Buckets** eingeordnet: nächste **64 KiB** unter 1 MiB, nächste **1 MiB** unter 1 GiB, nächste **100 MiB** darüber. Bytes werden niemals auf den Datenträger geschrieben. Die Bucket-Einteilung beseitigt den verdeckten Kanal über niederwertige Bits je Tabelle, den ein exaktes `buf.len()` andernfalls eröffnen würde. |
| `sample_method` | string | Engine-spezifische Beschreibung der begrenzten Stichprobe, beispielsweise `"TABLESAMPLE SYSTEM(0.1) LIMIT N"`, `"LIMIT N (fallback after empty TABLESAMPLE)"` oder `"SELECT TOP N"` |
| `sampled_with_bias` | bool | true, wenn die Stichprobe nicht gleichmäßig ist, beispielsweise bei einem reinen LIMIT-Fallback |
| `bias_reason` | string | leer, wenn `sampled_with_bias = false`; andernfalls eine Kennzeichnung wie `"unordered_limit_after_empty_TABLESAMPLE"` |
| `ratio_zstd_3` | float | auf die nächsten **0.05** gerundet, gemäß der zstd-Messrichtlinie des Vertrags für Stufe 3. Gemessen an Bytes, die mit `sample_encoding` kodiert wurden. |
| `ratio_zstd_19` | float | veraltetes zstd-Stufe-19-Verhältnis aus älteren Erfassungen; das Werkzeug misst und emittiert es nicht mehr |
| `ratio_stddev` | float | auf die nächsten **0.05** gerundet, Standardabweichung der Stufe-3-Verhältnisse über begrenzte Tabellen-Probe-Frames. Projektionsblöcke auf Spaltenebene geben derzeit `0.0` aus, weil sie beratende Entropiehinweise und kein Varianzmodell darstellen. |
| `sample_encoding` | string | Bezeichner der Byte-Kodierung und Richtlinie für die Komprimierungssitzung, die für die Messung verwendet wurden. Live-Tabellenblöcke von PostgreSQL verwenden derzeit `"blueprint-columnar-transfer-probe-v2"`. MySQL und SQL Server verwenden `"blueprint-columnar-transfer-probe-v3"`, das zusätzlich an 256-KiB-Probe-Blockgrenzen einen Flush ausführt. SQL Server behält für `nvarchar`/`nchar`/`ntext` die native UTF-16LE-Byteverteilung bei; `varchar`/`char`/`text` behalten ihre beprobte Bytebreite bei, und `charset` gibt die Katalog-Codepage an, die ein Verbraucher benötigt. V1 bleibt ein reiner Eingabekompatibilitätsvertrag. Spaltenblöcke verwenden `"blueprint-compression-probe-v2"`. Der dbwarp-Estimator MUSS diese Zeichenfolge validieren, bevor er das Verhältnis verwendet; unterschiedliche Kodierungen oder Sitzungsrichtlinien sind NICHT austauschbar. Ältere Blueprint-Dateien enthalten dieses Feld möglicherweise nicht; Estimatoren sollten gemessene Verhältnisse nur verwenden, wenn die Kodierungskennzeichnung vorhanden und bekannt ist. |

Der dbwarp-Estimator sollte beim Erstellen synthetischer Testdatensätze erkannte Komprimierungsblöcke je Spalte bevorzugen, danach auf die Komprimierung auf Tabellenebene und schließlich auf Typ-/Stilvorgaben zurückfallen.

### Byte-Kodierung `blueprint-compression-probe-v2`

Der Tier-2-Sampler hängt Zeilen oder beprobte Spaltenwerte in diesem Format an einen Puffer im Arbeitsspeicher an und führt darauf zstd mit Stufe 3 aus. Der Puffer wird verworfen. Der Blueprint behält ausschließlich die dokumentierten aggregierten Felder für Komprimierung, NULL-Dichte, Kardinalität/Häufigkeit, Länge und Stil.

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

Typkennzeichnungen sind Teil des Probe-Vertrags und werden nicht neu nummeriert,
ohne einen neuen versionierten Probe-Bezeichner zu verwenden.

| Kennzeichnung | Name | Verwendet für |
|---|---|---|
| 0x00 | Null | SQL NULL (keine Länge, keine Nutzdaten) |
| 0x01 | TextUtf8 | UTF-8-Text |
| 0x02 | TextUtf16Le | UTF-16LE-Bytes, hauptsächlich SQL Server `nvarchar`/`nchar`/`ntext` |
| 0x03 | TextOther | Bytes in einem anderen Zeichensatz |
| 0x04 | NumberText | dezimaltextuelle Darstellung numerischer Werte |
| 0x05 | BoolText | Boolescher Wert als Text |
| 0x06 | TimestampText | ISO-8601-Zeitstempeltext |
| 0x07 | DateText | ISO-8601-Datumstext |
| 0x08 | TimeText | `HH:MM:SS[.fff]`-Text |
| 0x09 | UuidText | kanonischer UUID-Text mit 36 Zeichen |
| 0x0F | JsonText | JSON UTF-8 |
| 0x10 | BinaryRaw | `bytea`-, `varbinary`-, `image`- oder Blob-Bytes |
| 0xFE | UnknownText | von der Datenbank bereitgestellte textuelle Fallback-Darstellung |

### Byte-Kodierung `blueprint-columnar-transfer-probe-v1`, `v2` und `v3`

Live-Datenbankverhältnisse auf Tabellenebene transformieren dieselben begrenzten
V2-Spaltenstichproben in neutrale Frames mit jeweils 1.000 Zeilen. Jeder Frame
besitzt einen versionierten Probe-Header und für jede Spalte eine Ordnungszahl,
eine Typkennzeichnung, eine Vier-Byte-Länge je Zeile und anschließend die
spaltenweise zusammenhängenden Nutzdatenbytes. Eine Länge von `0xffffffff`
stellt NULL dar. Die Bytedarstellung ist in allen drei Versionen gleich. V1
komprimierte die verbundene Framefolge als eine zstd-Operation der Stufe 3 mit
angekündigter Eingabelänge. V2 führt die Frames durch einen persistenten
zstd-Kontext der Stufe 3 und führt nach jedem Frame einen Flush aus. V3 behält
diesen Kontext und die neutrale Zeilengruppendarstellung bei, führt aber
zusätzlich an jeder 256-KiB-Probe-Kompressionsblockgrenze innerhalb einer
Zeilengruppe einen Flush aus. MySQL und SQL Server verwenden V3; V2 bleibt die
aktuelle PostgreSQL-Messung. SQL-Server-Unicode-Text wird als UTF-16LE gemessen.
Schmaler SQL-Server-Text
behält die Quellbytebreite bei und zeichnet einen geschlossenen, bereinigten
Zeichensatz aus der Sortierungs-Codepage auf. Die äußeren Zeilengruppenausgaben
liefern die Beobachtungen für `ratio_stddev`. Versionierte Kennzeichnungen
verhindern, dass eine Framing- oder Flush-Richtlinie stillschweigend als eine
andere interpretiert wird.

Diese Darstellung modelliert generische komprimierungsrelevante Eigenschaften
einer spaltenorientierten Massenübertragung. Sie ist weder eine Aufzeichnung
eines Datenbankprotokolls noch ein Migrations-Wire-Format oder ein kodierter
Datenexport. Stichprobenbytes verbleiben nur im Speicher und werden verworfen,
sobald die Aggregatmessungen abgeleitet sind.

### Genauigkeitsgrenzen

`ratio_zstd_3` beschreibt das angegebene `sample_encoding`; es ist keine Erfassung von Datenbankprotokoll- oder Migrations-Wire-Bytes. Die öffentliche automatisierte Testsuite validiert deterministische Codierung, begrenzte Stichproben und Serialisierung, beansprucht aber keinen universellen prozentualen Fehler über alle Engines und Extraktionspfade hinweg.

Bevor Sie das Verhältnis für eine wichtige Kapazitätsentscheidung verwenden, qualifizieren Sie die aktuelle Binärdatei und Engine-Version mit repräsentativen Quelldaten und dem beabsichtigten Extraktionsmechanismus. Erfassen Sie Vergleichsmethode, Stichprobengröße, Binärdatei-Hash, Engine-Version und beobachteten Fehler zusammen mit dem resultierenden Plan. Die grundlegende Beziehung lautet `compressed_bytes ≈ sample_bytes / ratio_zstd_3` unter der Byteverteilung, die das aufgezeichnete sample_encoding erzeugt.

## `[fk_edges]`

Optional. Inline-Tabelle, in der jeder Schlüssel eine ID `table-NNN` ist, die
auf eine Liste von Kanten abbildet. Schema v3 bewahrt Elternordnungszahlen,
referenzielle Aktionen, den Übereinstimmungsmodus, Aufschiebbarkeit,
Validierungs-/Vertrauensstatus sowie eine optionale begrenzte, namenfreie
Beziehungszusammenfassung. Kanten werden zuerst nach Ziel und danach nach
Spaltenliste sortiert.

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

Der optionale Block `statistics` enthält beprobte oder abgeleitete Werte für
`non_null_rows`, `distinct_parent_values`, `parent_coverage_fraction`, Fanout
p50/p95/p99/max und `orphan_rows` sowie Herkunfts- und Verzerrungsfelder. Aus
validierten Quell-Constraints folgt, dass keine verwaisten Zeilen vorliegen.
Aus spaltenweisen Stichproben abgeleitete zusammengesetzte Schätzungen sind
ausdrücklich als abgeleitet gekennzeichnet. Generatoren verwenden diese
Aggregate, um NULL-Abdeckung und Fanout nachzubilden, und ordnen dabei jeden
zusammengesetzten Kindschlüssel einem konsistenten synthetischen Elterntupel zu.

## `[artifact_inventory]` (seit Schema v4, Datenbankquellen)

Der unabhängig versionierte Vertrag `dbwarp-blueprint-artifacts/v1` beschreibt
Nicht-Tabellenobjekte, ohne Quellnamen oder Definitionen zu serialisieren. Bei
strukturierten Dateiquellen und bei Auswahl von `--artifact-detail none` fehlt er.

Die Voreinstellung `--artifact-detail summary` gibt `object_count`,
`external_prerequisite_count`, `counts_by_kind` und
`counts_by_external_class` aus. `graph` ergänzt einen anonymen Objektdatensatz
je Artefakt und Abhängigkeitskanten. `analyzed` ergänzt begrenzte Datensätze des
Vertrags `dbwarp-language-feature-census/v1`, die vorübergehend aus verfügbaren
Definitionen abgeleitet werden. `graph` und `analyzed` erfordern ausdrücklich
`--yes`, weil die Graphtopologie eine Anwendung identifizieren kann.

Der Nachweis auf Inventarebene umfasst:

| Feld | Werte / Regel |
|---|---|
| `detail` | `none`, `summary`, `graph` oder `analyzed` |
| `visibility` | `full`, `privilege_filtered` oder `unknown` |
| `inventory_complete` | Darf nur bei vollständiger Sichtbarkeit, ohne unlesbare Kataloge und ohne deklarierte unmodellierte Familien wahr sein |
| `dependencies_complete` | Darf nur wahr sein, wenn die modellierten Abhängigkeitskataloge lesbar waren |
| `analysis_complete` | Darf nur bei Detailgrad analyzed und nur dann wahr sein, wenn jede ausgegebene Analyse vollständig ist |
| `catalogs_read` | Geschlossene Standardbezeichnungen der erfolgreich geprüften Engine-Kataloge |
| `catalogs_unreadable` | Fehlgeschlagene Katalogbezeichnungen; jeder Eintrag verhindert eine Vollständigkeitsbehauptung |
| `families_not_inventoried` | Bekannte Objektfamilien außerhalb des aktuellen Collector-Vertrags |

Objekt-IDs besitzen die Form `<kind>-NNN`, beispielsweise `view-001` oder
`function-002`. Ein Datensatz enthält nur geschlossene Kind-/Subkind-/Tier-Token,
anonyme Schema-/Eltern-IDs, anonyme Abhängigkeiten, die Anzahl ungelöster
Abhängigkeiten, begrenzte Definitionssichtbarkeit/Sicherheitsmodi, eine optionale
externe Voraussetzung und eine optionale Sprachanalyse. Quellobjektnamen,
SQL-Text, Principals, Endpunkte, Anmeldedaten, Schlüssel, Zertifikate und
Binärdateien sind keine Felder des Vertrags.

Externe Voraussetzungen zeichnen eine geschlossene `class`, den
Bereitstellungsumfang, den Bedarf an nicht erfasstem Binär-/Geheimnis-/Endpunktmaterial
und eine begrenzte Kompatibilitätskategorie auf. Ihre Anzahl ist
Migrationsplanungsnachweis und keine Behauptung, DBWarp könne sie automatisch
bereitstellen oder übersetzen.

Datensätze der Sprachanalyse verwenden `analyzer_version = "lexical-v1"` und
`status = "partial"`. Anzahlen, Größe, Verschachtelung, Komplexität und
Opaque-Regionen sind Bänder, keine exakten Quellfingerprints. Features stammen
aus einem geschlossenen Vokabular. Der Analyzer entfernt Kommentare, Literale
und quotierte Bezeichner; er ist kein Parser, semantischer Binder oder Nachweis
für Übersetzungserfolg.

Siehe [Inventar der Nicht-Tabellenartefakte](ARTIFACT_INVENTORY.md) für
Betriebsanleitung und Engine-Abdeckung.

## Steganografie-Abwehr nach Vektor

| Vektor | Abwehr |
|---|---|
| Reihenfolge der Bezeichner | Domänengetrenntes HMAC-SHA256 mit einem geheimen prozesslokalen Schlüssel verhindert Offline-Prüfungen möglicher Namen. Verwenden Sie einen kundenseitig verwahrten Schlüssel nur, wenn stabile Kennzeichnungen über mehrere Läufe erforderlich sind. |
| Niederwertige numerische Bits | Statistiken werden standardmäßig auf die dokumentierte Genauigkeit gerundet. Der Modus für exakte Längen ist ausdrücklich, zustimmungspflichtig, wird im Auditprotokoll aufgezeichnet und muss als sensiblere Metadaten behandelt werden. |
| Zeitstempel unter einer Sekunde | Ein UTC-Zeitstempel am Anfang, nur mit Sekundengenauigkeit |
| TOML-Formatierung | Kanonisch: alphabetische Schlüssel, feste Einrückung und nur die festen Header-/Producer-Kommentare; keine aus Eingaben abgeleiteten Kommentare |
| Stichprobenzufall | Die Stichprobe verwendet feste Seeds (deterministisches `TABLESAMPLE SYSTEM` von PG). Unabhängig davon bezieht die Anonymisierung der Bezeichner absichtlich einen geheimen Schlüssel aus dem CSPRNG des Betriebssystems, sofern der Kunde keinen Schlüssel bereitstellt. |
| Nicht verwendete Felder | Jedes Feld ist oben dokumentiert; es gibt keine Felder „metadata“, „comment“ oder „reserved“, die unbegrenzte Daten enthalten könnten. |
| Artefakt-Quelltext und externes Material | Definitionen sind vorübergehend und werden nach begrenzter Analyse genullt; Namen, SQL-Text, Endpunkte, Providerzeichenfolgen, Anmeldedaten, Schlüssel, Zertifikate, Paketnamen und Binärdateien besitzen kein serialisiertes Feld |

## Kompatibilität der Schemaversion

Aktuelle Producer geben Schemaversion 6 aus. Versionen 1 bis 5 bleiben
abwärtskompatibel lesbar. Eine v1/v2-Datei enthält keine Verteilungsblöcke;
Generatoren verwenden daher deterministische Typ-/Breiten-
und gleichmäßige Beziehungs-Fallbacks und melden den Genauigkeitsverlust. Eine
v3-Datei enthält Verteilungsmetadaten, aber kein Artefaktinventar. Eine
v4-Datei kann ein Artefaktinventar enthalten, ist aber älter als die aktuellen
Blueprint-Vertragsbezeichner. Reader normalisieren frühere v4-Bezeichner bei
der Eingabe und geben das Dokument mit kanonischen Blueprint-Bezeichnern aus.
Eine v5-Datei ist älter als die in v6 hinzugefügte Qualifizierung von Topologie
und Datensatzumfang. Verbraucher müssen unbekannte künftige Schemaversionen mit einer klaren
Upgrade-Meldung ablehnen, statt Felder stillschweigend zu verwerfen.

## Warum TOML und nicht JSON

- TOML trennt strukturelle Abschnitte lesbarer von Blattdaten (`[tables.table-001.cols.col-2]` gegenüber verschachteltem JSON).
- Unterschiede sind leichter zu erkennen: ein Schlüssel je Zeile, zusammenhängende Untertabellen anhand von Bezeichnern.
- Der Kunde kann die Datei von Hand bearbeiten, wenn er vor der Weitergabe ein bestimmtes Feld entfernen möchte.

JSON wird im SQL-Fallback-Pfad als **Zwischenformat** verwendet. Jedes Skript `sql/blueprint.*.sql` erzeugt JSON, das `blueprint_format.py` zu TOML normalisiert. Das Zwischen-JSON enthält echte Quellbezeichner; MySQL kann über `COLUMN_TYPE` außerdem enum/set-Deklarationen enthalten. Es muss daher geschützt innerhalb der Quellumgebung bleiben. Der Normalisierer verwendet standardmäßig einen neuen geheimen Schlüssel und akzeptiert für genehmigte Vergleichsläufe denselben geschützten `--anonymization-key-file`-Vertrag. Die für eine Weitergabe an DBWarp geprüfte Enddatei ist immer TOML.

## Herkunftserweiterungen für strukturierte Dateien

Wenn `engine` oder `source_kind` `"parquet"` oder `"avro"` ist, kann
Schemaversion 3 und neuer zusätzlich die folgenden begrenzten Felder ausgeben.
Ältere Leser müssen Felder ignorieren, die sie nicht verstehen; neuere Leser
müssen die Unterscheidung zwischen der Speicherung in der Quelldatei und
begrenzten Messungen an dekodierten Stichproben bewahren.

Blueprints strukturierter Dateien verwenden dieselben anonymisierten Bezeichner
wie Datenbank-Blueprints: `table-NNN` in mit geheimem Schlüssel erzeugter Reihenfolge und `col-N`
in Schemaordinalreihenfolge. Dateistämme, Parquet-Pfade, Avro-Feldnamen und das
Manifestfeld `logical_table` werden nicht als Tabellen- oder Spaltenbezeichner
ausgegeben.

Auf Tabellenebene bezeichnet
`table_bytes` die logische Größenannahme für den Transfer, `storage_bytes`
dagegen die tatsächliche Größe des Quellobjekts. Parquet ohne dekodierte
Stichprobe verwendet unkomprimierte Spalten-Chunk-Bytes für `table_bytes`; eine
optionale dekodierte Stichprobe ersetzt sie durch hochgerechnete
`blueprint-compression-probe-v2`-Bytes. Avro leitet den Wert aus seinem vollständig
dekodierten Durchlauf ab. `source_partitions`, `row_group_count` und
`source_codec` dokumentieren Dateiaufbau und Planungsherkunft. Mehrdatei-Datensätze
aggregieren diese Werte. `row_group_count` gilt für Parquet; bei einem einzelnen
Eingabeobjekt ist `source_partitions` gleich `1`.

Auf Spaltenebene ist `null_fraction` ein beobachteter Wert von `0.0` bis `1.0`.
`length_sample_rows` und `length_sample_method` beschreiben die Herkunft von
`len_avg` und `len_p95`. `source_semantics` hält begrenzte
Kompatibilitätsfakten wie `"repeated-leaf"`, `"nested-json"` oder
`"multi-type-union"` fest; es enthält niemals einen Kundenfeldnamen oder
-wert. Dezimalpräzision und -skala,
Zeitstempelpräzision und UTC/lokale Semantik, UUID sowie Metadaten für binäre
Werte fester Größe werden in den vorhandenen bereinigten skalaren Feldern und
`native_type` geführt.

Auf Tabellenebene vergleicht `ratio_storage` `table_bytes` mit den tatsächlichen
Quellobjekt-Bytes. Auf Parquet-Spaltenebene vergleicht der Wert unkomprimierte
und komprimierte Spalten-Chunk-Bytes aus dem Footer. Beides sind Signale für die
Dateiplanung, keine Schätzungen aus dekodierten Stichproben. `ratio_zstd_3` und
`ratio_zstd_19` sind nur dann gültige Eingaben zur Übertragungskalibrierung, wenn `sample_encoding` den
erkannten Wert `"blueprint-compression-probe-v2"` besitzt. Parquet-Footer- und
Avro-Containerquoten dürfen niemals in diese zstd-Felder kopiert werden.
