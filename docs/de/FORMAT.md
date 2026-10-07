# DBWarp Blueprint Dateiformat Version 7

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
# dbwarp-blueprint v7
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

Die Leerzeile ist Teil des kanonischen Dateikopfs. Der Rust-Collector gibt exakt diesen Dateikopf und keine weiteren Kommentare aus. Der SQL-Fallback-Normalisierer behält ihn wörtlich bei und fügt anschließend genau einen festen Kommentar `Producer: blueprint_format.py SQL fallback` mit der Schlüsselquelle hinzu, damit Empfänger den Producer unterscheiden können. Dies ist keine Zusage, dass die verbleibenden strukturierten Felder kein unverwechselbares Schema oder keinen Abhängigkeitsgraphen identifizieren können.

## Felder auf oberster Ebene

| Feld | Typ | Beschreibung |
|---|---|---|
| `schema_version` | int | Versionsnummer. Derzeit `7`. Versionen 1 bis 6 sind weiterhin lesbar. |
| `generated_at` | ISO-8601-Zeichenkette. | UTC-Zeitstempel, Sekunden-Genauigkeit, ohne Bruchteile. **Fixierbar** über das `--generated-at "2026-04-26T00:00:00Z"` CLI-Flag. Byte-identische Live-Aufnahmen erfordern ebenfalls das gleiche geschützte `--anonymization-key-file`, den Quellzustand, die Optionen und die Collector-Version. Das Audit-Protokoll protokolliert `generated_at_pin: ...`, wenn das Flag gesetzt wird, sodass die Fixierung forensisch sichtbar ist. Keine Umgebungsvariable fixiert diesen Wert. |
| `engine` | String. | `"postgresql"`, `"mysql"`, `"sqlserver"`, `"oracle"`, `"parquet"` oder `"avro"`. `oracle` erscheint nur in der Ausgabe der Oracle-Vorschau. |
| `engine_version` | String. | Numerische Produktversion der Quelldatenbank; leer für quellendateibasierte Quellen. Distributionshinweise werden ausgeschlossen. |
| `source_kind` | String. | Datenbankquellen verwenden vom Operator angegebene `"production"`, `"staging"`, `"scrubbed-replica"` oder `"synthetic"`. Strukturierte Quellen verwenden `"parquet"` oder `"avro"`. |
| `length_metadata` | String. | Zusammenfassung: Der Marker wurde für frühere Leser beibehalten: `"hybrid-v2"`, `"exact"`, `"rounded"` oder `"not-captured"`. Die drei Felder unten sind maßgeblich. |
| `declared_length_fidelity` | string | `"exact"` für deklarierte PostgreSQL-Zeichenkapazitäten und die standardmäßigen ausgewogenen/exakten MySQL-Modi, `"coarse-rounded-v1"` für strikten MySQL-Datenschutz oder `"not-captured"`, wenn nicht verfügbar. |
| `index_length_fidelity` | string | `"exact"` für standardmäßig ausgewogene/exakte MySQL-Indexpräfixe, `"rounded-down-v1"` für strikten Datenschutz oder `"not-captured"`, wenn nicht verfügbar. |
| `observed_length_fidelity` | string | Standardmäßig `"relative-rounded-v2"`, wenn beprobt, `"exact"` im exakten Modus, `"coarse-rounded-v1"` im strikten Modus oder `"not-sampled"`. Die Stichprobenabdeckung bleibt eine separate Anforderung je Spalte. |
| `[totals]` | inline table | Aggregierte Anzahlen (siehe unten). |
| `[network]` | table | Optionaler Nachweis für Client-zu-Datenbank-Verbindung und Abfrage-RTT. |
| `[database_topology]` | Tabelle. | Erforderlich für Datenbankquellen mit Schema-Version 6 und neuer. Schema-Version 7 verwendet den Topologie-Vertrag Version 2 und protokolliert den Umfang jeder Elementanzahl. Nicht vorhanden bei strukturierten Dateien. |
| `[dataset_scope]` | Tabelle. | Erforderlich für jedes Schema der Version 6 und neuer. Definiert, welche Gesamtzahlen erfasst werden und ob die Abdeckung von Tabellen, Zeilen und Bytes vollständig ist. |
| `[structure_scope]` | Tabelle. | Erforderlich in Version 7. Qualifiziert separat die Vollständigkeit des Inventars von Tabellen, Spalten, Indizes und Beziehungen. |
| `[source_environment]` | Tabelle. | Erforderlich in v7 Datenbank-Blueprints und verboten für strukturierte Dateien. Enthält nur grobe capacity/hosting-Hinweise, die über den Datenbank-Endpunkt oder einen expliziten Anbieter beobachtet wurden. |
| `[statistics_evidence]` | Tabelle. | Erforderlich in Version 7. Eine exakte Zusammenfassung der Klassifizierungen für die Anzahl der Zeilen pro Tabelle, Optimierungsstatistiken und Größenangaben. |
| `[activity_snapshot]` | Tabelle. | Nicht von DBWarp Blueprint 1.6 geschrieben. |
| `[tables.X]` | tables | Eine je Tabelle, mit anonymisierter ID. |
| `[fk_edges]` | inline table | Fremdschlüsselgraph zwischen anonymisierten Tabellen. Optional. |
| `[artifact_inventory]` | Tabelle. | Erforderlich in Version 7, sodass "nicht angefordert", "nicht anwendbar", "nicht lesbar" und ein verifiziertes Inventar von null Objekten weiterhin unterschieden werden können. Enthält begrenzte, namenlose Objektanzahlen, optionale, typisierte anonyme Beziehungen, Anforderungen und eine begrenzte Sprachstatistik. |

## `[totals]`

| Feld | Typ | Genauigkeit |
|---|---|---|
| `table_count` | int | exakt |
| `row_count` | int | Summe der serialisierten Daten pro Tabelle `rows`; Katalogschätzungen sind gerundet, während ein vollständig überprüfbarer, begrenzter Lesezugriff exakt ist. |
| `table_bytes` | int | Summe der je Tabelle gerundeten Werte `table_bytes` |
| `index_bytes` | int | Summe der je Tabelle gerundeten Werte `index_bytes` |

Diese Zahlen sind nicht automatisch Gesamtsummen für den gesamten Cluster. Interpretieren Sie sie immer zusammen mit `[dataset_scope]`. Ein shardetes Gateway oder ein Koordinator kann einen vollständigen Katalog anzeigen, ohne jedoch einen der zugrunde liegenden Shards zu enthalten; die Schemata v6 und v7 stellen diese Unsicherheit explizit dar, anstatt lokale Katalogstatistiken stillschweigend als globale Wahrheit zu behandeln.

`row_count` ist eine arithmetische Summe der serialisierten, tabellenspezifischen Werte und keine zweite, nicht gerundete Messung. Eine bekannte, positive Anzahl, die unter dem ersten Datenschutzkorb liegt, wird als `100` mit tabellenspezifischen `row_count_quality = "engine-estimate"` dargestellt; ein Datensatz, der viele solcher kleinen Tabellen enthält, kann daher eine konservativ hohe Gesamtzahl aufweisen. In diesem Fall enthält `dataset_scope.limitations` auch `row-counts-statistical`. Null bleibt für den Fall reserviert, dass die Quelltabelle leer ist.

## `[database_topology]` (Datenbankquellen)

Dieser Block speichert nur begrenzte Fakten, die über den verbundenen
Datenbankendpunkt sichtbar sind. Er speichert niemals Knotennamen, Hostnamen,
IP-Adressen, Clusternamen, Replikationskanalnamen, Serverkennungen oder
Endpunkte.

| Feld | Werte / Regel |
|---|---|
| `contract` | `dbwarp-blueprint-topology/v1` im Schema v6; `dbwarp-blueprint-topology/v2` in v7. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed` oder `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member`, `physical-standby`, `logical-standby`, `snapshot-standby` oder `unknown`. |
| `visibility` | `full`, `partial` oder `unknown`; beschreibt Topologienachweise, nicht die Datenrichtigkeit. |
| `member_count` | Anzahl der durch erfolgreiche Nachweisabfragen sichtbaren Mitglieder. `0` bedeutet unbekannt, niemals null Mitglieder. |
| `member_count_scope` | V7 nur: `deployment`, `visible-subset`, `connected-member` oder `unknown`. Für vollständige Topologie-Sichtbarkeit ist `deployment` erforderlich; `connected-member` erfordert einen Wert von eins. |
| `identifiers_redacted` | Muss `true` sein. |
| `role_counts` | Optionale Anzahlen nach geschlossenem Rollentoken. Volle Sichtbarkeit verlangt, dass diese Anzahlen `member_count` entsprechen. |
| `features` | Sortierte, geschlossene Token wie `citus`, MySQL-replication/cluster-Formulare, `postgresql-streaming-replication`, `sqlserver-availability-group`, `oracle-non-cdb`, `oracle-cdb`, `oracle-pdb`, `oracle-rac`, `oracle-data-guard` oder `vitess`. |
| `catalogs_read` | Sortierte geschlossene Bezeichnungen für erfolgreich gelesene Topologiekataloge. |
| `catalogs_unreadable` | Sortierte geschlossene Bezeichnungen für nicht lesbare Topologiekataloge. Jeder Eintrag verhindert die Aussage voller Sichtbarkeit. |
| `catalogs_not_applicable` | Nur für V7. Sortierte, geschlossene Labels haben sich für diese Quelle als nicht anwendbar erwiesen. Diese Menge ist disjunkt von den lesbaren und nicht lesbaren Mengen. |

Ein gewöhnlicher Endpunkt darf `deployment = "unknown"` melden und dennoch
vollständige lokale Statistiken einer Vollkopie liefern. Blueprint nimmt nicht
an, dass ein unauffälliger Server single-node ist, nur weil keine
Clusterfunktion sichtbar war.

## `[dataset_scope]` (Schemaversion 6 und höher)

Dieser Block qualifiziert jede Größenangabe unabhängig. Behandeln Sie die Gesamtwerte nicht als Werte für den gesamten Datensatz, wenn eine der erforderlichen Vollständigkeitsdimensionen `incomplete` oder `unknown` beträgt.

| Feld | Werte / Regel |
|---|---|
| `contract` | Immer `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset` oder `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete` oder `unknown`. |
| `row_count_completeness` | `complete`, `incomplete` oder `unknown`. |
| `size_completeness` | `complete`, `incomplete` oder `unknown`. |
| `row_count_method` | Ein geschlossener Provenance-Token wie `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter`, `oracle-table-statistics` oder `oracle-segment-statistics`. `bounded-complete-read` und `mixed-catalog-and-bounded-read` identifizieren Gesamtzahlen, die aus einem vollständig überprüften Tier-2-Lesevorgang stammen, entweder allein oder zusammen mit Katalogzahlen. Oracle verwendet `not-applicable` zusammen mit der gleichen Größenmethode, nur wenn ein nicht leerer Bestand keine Tabellen in der Kopie-Gesamtpopulation enthält. `distributed-aggregate` wird als Eingabe akzeptiert, wird aber von dieser Version nicht geschrieben. |
| `size_method` | Ein geschlossener Provenance-Token wie `postgres-local-relation-size`, `citus-distributed-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `oracle-segment-bytes`, `oracle-table-logical-estimate`, `mixed` oder `not-applicable`. Oracle verwendet `mixed`, wenn die enthaltenen Tabellen attributierte Segmentzähler mit beschrifteten logischen Schätzungen kombinieren. Es verwendet `not-applicable` nur, wenn ein nicht-leeres Inventar keine Tabellen in der Gesamtzahl enthält, sodass eine vollständige Nullsumme nicht fälschlicherweise eine Messmethode beansprucht. `distributed-aggregate` wird als Eingabe akzeptiert, wird aber von dieser Version nicht geschrieben. |
| `limitations` | Sortierte geschlossene Gründe für unvollständige oder unbekannte Abdeckung. Mindestens einer ist erforderlich, sofern nicht alle Dimensionen vollständig sind. |

`selection-limited` bedeutet, dass Summen und Vollständigkeitsangaben genau die über den wiederholbaren Live-Selektor `--schema` angeforderten Schemata abdecken; sie beanspruchen keine Abdeckung der gesamten verbundenen Datenbank. Ohne `--schema` bleibt das Erfassungsverhalten für alle sichtbaren Schemata erhalten.

Ein lesbares, ausgewähltes Schema kann durchaus nur Nicht-Tabellen-Objekte enthalten und wird in den entsprechenden Inventaren beibehalten. Wenn die vollständige Erfassung jedoch überhaupt keine Tabellen enthält, darf der Collector keinen definitiven, leeren Datensatz veröffentlichen: die Vollständigkeit von Tabellen, Zeilen und Größe bleibt unvollständig, und `table-inventory-visibility-unknown` protokolliert die konservative Grenze.

`row-count-evidence-incomplete` und `size-evidence-incomplete` bedeuten, dass mindestens eine der enthaltenen Tabellen den entsprechenden Katalogwert nicht enthielt. Die numerische Gesamtzahl ist dann die Summe der bekannten Beiträge und keine Aussage darüber, dass eine nicht verfügbare Tabelle null Zeilen oder Bytes enthielt. Tabellenspezifische Statistiken zeigen, welche Datensätze nicht verfügbar sind.

Für Oracle ist `oracle-segment-bytes` der bevorzugte, genaue Nachweis der zugewiesenen Größe. Wenn der Speicherplatz für eine Tabelle nicht aus `DBA_SEGMENTS` ermittelt werden kann – beispielsweise bei einer geclusterten Tabelle oder einer indexorganisierten Tabelle, deren Indexkatalog nicht verfügbar ist – kann der Collector `oracle-table-logical-estimate` unter Verwendung der bereits verfügbaren `DBA_TABLES.NUM_ROWS * AVG_ROW_LEN`-Werte ausgeben. Der Tabellennachweis enthält dann `size_quality = "engine-estimate"`, `size_scope = "table-only"` und `size_accounting = "logical-estimate"`, `size_visibility = "partial"`, und die Vollständigkeit der Datensatzgröße beträgt `incomplete`; er beansprucht jedoch keine LOB- oder Indexbytes. Ein fehlender Verfeinerungskatalog führt niemals dazu, dass bereits dieser logischen Tabelle zugewiesene Bytes verworfen werden: der gemessene Beitrag bleibt `oracle-segment-bytes`, mit teilweiser Sichtbarkeit und unvollständiger Aggregatsabdeckung. Nicht zugeordneter, gemeinsamer oder indexorganisierter Speicherplatz wird nicht als exakte Null veröffentlicht. Eine Oracle-Tabelle mit einem Domänenindex verwendet ebenfalls eine partielle Sichtbarkeit und einen unbekannten Umfang, da Text-, Spatial- und andere Domänenimplementierungen Bytes in sekundären Objekten außerhalb des ausgegebenen Benutzer-Tabelleninventars speichern können. Der logische Ausweichmechanismus ist ein Nachweis zur Größenbestimmung einer Kopie und nicht ein Zähler für zugewiesene Bytes. Er kann die aktuelle Zuweisung überschätzen, wenn die Zeilenzahlen des Optimierers veraltet sind, nachdem der Speicher freigegeben wurde (z. B. nach `TRUNCATE ... DROP STORAGE`), und seine Schätzung muss beibehalten werden. Für diesen Ausweichmechanismus ist keine `DBA_TABLESPACES`-Berechtigung erforderlich.

Für die Indexspeicherung in Oracle markiert das abgeschlossene Lesen des Index-Katalogs die logische Momentaufnahme. Eine spätere `DBA_SEGMENTS`-Zeile ohne passende Index-Identität liegt außerhalb dieser Datenmenge und wird keiner beliebigen Tabelle zugeordnet. Ein Index, der zum Zeitpunkt der Momentaufnahme vorhanden war, aber seinen erwarteten Segmentbeitrag nicht enthält, bietet keine vollständige Größeninformation für seine Tabelle.

`logical-partition-root-unmeasured` ist ein PostgreSQL-spezifisches Indiz dafür, dass ein eingeschlossener logischer Partitionsstamm absichtlich weder Zeilen noch Bytes liefert, da diese Werte in seinen physischen Blättern gespeichert sind. Im Gegensatz zu einer behebaren `row-count-evidence-incomplete`-Statistiklücke kann ein vollständiges Lesen einer anderen Tabelle die Vollständigkeit auf Datensatzebene nicht wiederherstellen, solange ein solcher Stamm im ausgewählten Inventar vorhanden ist.

`table-inventory-visibility-unknown` bedeutet, dass der Collector die vom System benötigte Klassifizierung nicht lesen konnte, um Benutzerobjekte von Supportobjekten zu trennen. Sichtbare Datensätze können weiterhin vorhanden sein, aber die Vollständigkeit von Tabellen, Zeilen und Größen wird nicht mehr gewährleistet, anstatt diesen Teil als vollständige Datenbasis zu betrachten.

`catalog-capture-truncated` bedeutet, dass eine Katalog-Sitzung, die von einer einzigen Quelle stammt, beendet wurde, bevor alle beabsichtigten Familien oder ausgewählten Schemata gelesen wurden. Bereits als vollständig erwiesene Datensätze können weiterhin ausgegeben werden, aber keine Aussage zur Vollständigkeit eines Datensatzes oder einer Struktur darf sich auf den nicht gelesenen Rest beziehen.

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

## `[structure_scope]` (Schemaversion v7)

Dieser Block unterscheidet einen verifizierten, leeren Katalog von einem Katalog, der gefiltert, nicht lesbar oder nicht überprüft wurde.

| Feld | Werte / Regel |
|---|---|
| `contract` | Immer `dbwarp-blueprint-structure-scope/v1`. |
| `visibility` | `full`, `privilege-filtered` oder `unknown`. Die Vollständigkeit bezieht sich auf die ausgewählten Schemata und den sichtbaren Berechtigungsbereich; sie stellt keine Aussage über uneingeschränkte Datenbankzugriffsmöglichkeiten dar. |
| `table_inventory_completeness`, `column_inventory_completeness`, `index_inventory_completeness`, `relationship_inventory_completeness` | Unabhängig von `complete`, `incomplete` oder `unknown`. Abhängige Familien können nicht als vollständig markiert werden, wenn ihre erforderliche Elternfamilie unvollständig ist. |
| `catalogs_read`, `catalogs_unreadable`, `catalogs_not_applicable` | Sortierte, disjunkte, geschlossene Katalogbezeichnungen. `catalogs_read` protokolliert positive Leseergebnisse; bei einer Multi-Owner-Erfassung kann ein Katalog beibehalten werden, wenn mindestens ein beabsichtigter Eigentümer erfolgreich gelesen wurde, auch wenn dies bei einem anderen Eigentümer nicht gelang. `catalogs_unreadable` bedeutet, dass keine positiven Leseergebnisse verbleiben. Eine vollständige Familie erfordert ihren enginespezifischen Katalog in `catalogs_read`, jede beabsichtigte Familienabfrage muss abgeschlossen sein und es darf keine betroffenen Lücken pro Objekt geben. |
| `limitations` | Sortierte, eindeutige Gründe wie `selection-limited`, `metadata-visibility-privilege-filtered` oder `table-kinds-not-inventoried`. Teilweise oder unbekannte Beweise erfordern einen Grund. |

Schema-Auswahlkriterien sind Teil des Umfangs: `complete` bedeutet vollständig für die ausgewählten, aufgelösten Schemata, nicht unbedingt für jedes Schema im System. Ein Auswahlschema, das zu keinem Schema aufgelöst wird, ist ein Fehler und darf nicht zu einem vollständigen, leeren Blueprint werden.

`catalog-capture-truncated` hat im Kontext der strukturellen Nachweise die gleiche Bedeutung: die veröffentlichten Tabellen- und Spalteneinträge stellen das verifizierte Präfix oder die Eigentümersubgruppe dar, nicht die Behauptung, dass die restlichen geplanten Katalogarbeiten abgeschlossen wurden. Ein Katalog, der von diesem Prozess nicht bearbeitet wurde, erscheint in keinem der drei Kataloge; er darf nicht als unlesbar oder nicht anwendbar umgelabelt werden.

Für einen Mehrfach-Eigentümer-Lesezugriff können `index-inventory-unavailable` oder `relationship-inventory-unavailable` daher einem Katalog in `catalogs_read` beigefügt sein: das Kataloglabel bewahrt den positiven Nachweis des erfolgreichen Eigentümers, während das Feld für die Vollständigkeit und der Begrenzungssatz festhalten, dass die gesamte ausgewählte Population nicht beobachtet wurde. Tabellspezifische Einschränkungen identifizieren ausgegebene Objekte mit einer Repräsentativitätslücke; sie ersetzen nicht den Nachweis des Abfragestatus für einen abgelehnten oder nicht versuchten Eigentümer, der keine Tabellen ausgegeben hat.

Oracle speichert die Datensätze `oracle-identity-columns` und `oracle-constraint-columns` getrennt von ihren übergeordneten Spalten- und Constraint-Katalogen. Ihr Vorhandensein oder Fehlen beschreibt optionale Identitätsgenerierung und Belege für Beziehungen; dies darf nicht zu der Aussage zusammengefasst werden, dass der übergeordnete Katalog nicht lesbar war.

## `[source_environment]` (Schema v7 Datenbankquellen)

Dieser Block beschreibt niemals die Workstation, auf der `dbwarp-blueprint` ausgeführt wird. `collector_machine_excluded` muss `true` sein. Nachweise für die Kapazität stammen ausschließlich vom verbundenen Datenbankendpunkt oder von einem explizit autorisierten Anbieter, Orchestrator oder Betreiber.

| Feld | Werte / Regel |
|---|---|
| `contract` | Immer `dbwarp-blueprint-source-environment/v1`. |
| `evidence_origin` | `database-endpoint`, `provider-api`, `orchestrator-api`, `operator-attested`, `mixed` oder `none`. |
| `hosting_model` | `managed-service`, `self-managed`, `orchestrated` oder `unknown`. |
| `infrastructure_location` | `cloud`, `on-premises`, `hybrid` oder `unknown`. |
| `capacity_scope` | `connected-instance`, `database-resource`, `cluster-aggregate`, `member-subset` oder `unknown`. |
| `capacity_visibility` | `capacity_visibility` kann `full`, `partial`, `unknown` oder `not-requested` sein. `not-requested` erfordert unbekannte Kapazitätsbereiche, -grundlagen und -umfang und kein klassifiziertes Kapazitätskatalog. Eine nicht-kapazitätsbezogene Klassifizierung, wie z.B. die SQL Server-Version, kann dennoch vorhanden sein. |
| `cpu_capacity_band` | `1`, `2`, `3-4`, `5-8`, `9-16`, `17-32`, `33-64`, `65-128`, `129-plus` oder `unknown`. |
| `cpu_capacity_basis` | `logical-cpu-limit`, `database-resource-limit`, `operating-system-visible`, `physical-host` oder `unknown`. `operating-system-visible` behauptet nicht, dass eine VM, ein Container oder eine verwaltete Dienstinstanz der zugrunde liegende physische Host ist. |
| `memory_capacity_band` | Grobe Bereiche von `under-2-gib` bis `512-gib-plus`, oder `unknown`. |
| `memory_capacity_basis` | `database-buffer-cache`, `database-resource-limit`, `operating-system-visible`, `physical-host` oder `unknown`. `operating-system-visible` ist die konservative Grundlage für eine Engine-DMV, deren Wert einen Gast oder Container und nicht unbedingt Bare-Metal beschreiben kann. Ein `database-buffer-cache`-Band ist die konfigurierte Cache-Zuweisung und daher nur eine untere Grenze für den gesamten Quell-Speicher; es darf niemals als Host-Kapazität dargestellt werden, ohne seine Grundlage zu berücksichtigen. |
| `member_capacity_uniform` | Optional observed/attested (boolescher Wert); das Auslassen bedeutet "unbekannt". |
| `features` | Sortierte, geschlossene Fakten wie `autoscaling`, `burstable`, `container-limits-visible`, `database-resource-governed`, `serverless` oder `shared-host`. |
| `limitations` | Sortierte, geschlossene Provenienzbeschränkungen. `oracle-client-version-mismatch` oder `oracle-client-version-unreadable` zeigen an, dass eine Oracle SQL*Plus-Clientversion nicht vollständig attestiert werden konnte. `oracle-client-version-below-tested-floor` protokolliert einen attestierten Client, der älter ist als 12.1, die in diesem Vertrag codierte Vergleichsuntergrenze. Die Katalogerfassung wird fortgesetzt, da die Provenienz des Client-Banners die Datenbankstruktur nicht bestimmt. |
| Katalog-Sets. | Sortierte, voneinander getrennte Beweise für die exakten, versuchten Quelldatenbank-Kataloge, einschließlich der Klassifizierung der SQL Server-Edition, auch wenn dessen optionale Leistungsüberwachungsdatenbank (DMV) nicht lesbar ist. |

Unbekannte Kapazität ist nicht gleich Null-Kapazität. Eine Remote-Verbindung autorisiert nicht das Auslesen der CPU- oder Speicherkapazität des Hosts, auf dem der Collector läuft, und deren Umbenennung in "Server-Kapazität".

Für Oracle ist ein Kapazitätskatalog, der nur für einen Teil des beabsichtigten Abfragesatzes erstellt wurde, weiterhin ein positiver `catalogs_read`-Nachweis, aber seine Werte werden zurückgehalten und `capacity_visibility` ist `unknown`. Teilweise Zeilen dürfen nicht als eine systemweite CPU- oder Speicherbeschränkung dargestellt werden.

Oracle-SQL*Plus-Funktionseinstellungen werden, sofern lesbar, aus der numerischen Version der laufenden Sitzung ausgewählt, andernfalls aus dem Banner der ausführbaren Datei und zuletzt aus einer konservativen Protokolluntergrenze, die weder `ROWLIMIT` noch CSV-Markup als Ausgabefunktion auswählt. Beide Einstellungsdurchläufe versuchen weiterhin, ein geerbtes `ROWLIMIT` und den CSV-Modus zu deaktivieren; die Diagnose einer unbekannten Option durch einen älteren Client wird nur innerhalb des abgegrenzten Reset-Fensters toleriert. Eine Versionsinkompatibilität, eine unvollständige Attestierung, ein Parse-Fehler oder ein attestierter Client, der älter als 12.1 ist, schwächen nur die Provenienz. Dies blockiert niemals die Katalogerfassung.

## `[statistics_evidence]` und `[tables.<id>.statistics]` (Schema Version 7)

Jede Tabelle der Version 7 hat einen Statistikblock. Der obersten Block enthält genaue Zählungen nach `statistics_state`, `row_count_quality` und `size_quality`; jede Zuordnung muss jede Tabelle abdecken und genau den Tabellenebene-Klassifizierungen entsprechen. Die aggregierte Sichtbarkeit ist nur `full` für eine nicht leere Gesamtzahl, wenn jede gezählte Tabelle eine vollständige Größenansicht, bekannte Zeilendaten und einen klassifizierten Statistikstatus hat und kein Statistik-Katalog unlesbar ist. Explizit ausgeschlossene externe, temporäre und abgeleitete Objekte bleiben inventarisiert; ihre richtlinienbasierte Zeilen- und Größenverfügbarkeit mindert die Sichtbarkeit dieser Gesamtzahl nicht, aber ein unklassifizierter Statistikstatus tut dies dennoch. Wenn jede Tabelle ausgeschlossen ist, ist die aggregierte Sichtbarkeit `unknown` mit `statistics-visibility-unknown`; eine leere Population darf nicht unberechtigt `full` erhalten. `catalog-capture-truncated` protokolliert, dass die beabsichtigte Arbeit am Statistik-Katalog gestoppt wurde, bevor alle Eigentümer erreicht wurden, wobei jedoch alle positiven Katalog-Lesebelege beibehalten werden, die bereits erhalten wurden. Die gleiche Regel für positive Belege gilt, wenn ein Eigentümer erfolgreich liest und ein anderer abgelehnt wird: der Katalog bleibt in `catalogs_read`, während `statistics-partial` und die aggregierte Sichtbarkeit protokollieren, dass die ausgewählte Population nicht vollständig beobachtet wurde.

Tabellenbezogene Felder sind:

| Feld | Werte / Regel |
|---|---|
| `row_count_method` | Engine/version-aware Katalogmethode, `bounded-complete-read` wenn eine Anweisung der Stufe 2 die sichtbaren Tabellen sicher aufzählte, einen strukturierten Dateizähler oder `unknown`; eine normale Erfassung greift nicht stillschweigend auf `COUNT(*)` zurück. |
| `row_count_quality` | `exact-counter`, `exact-read`, `engine-counter`, `engine-estimate`, `cached-engine-estimate`, `sample-extrapolation`, `unavailable` oder `unknown`. Ein bekannter, positiver SQL Server-Zähler, der unter dem ersten Nicht-Null-Datenschutzbereich liegt, verwendet `engine-estimate`, nachdem `rows = 100` serialisiert wurde; dies unterscheidet den Datenschutzbereich sowohl von einem exakten Zähler als auch von einem gemessenen Nullwert. |
| `statistics_state` | `current`, `possibly-stale`, `known-stale`, `never-analyzed`, `locked`, `user-supplied`, `not-applicable` oder `unknown`. |
| `refresh_age_band` | `under-1h`, `1h-1d`, `1-7d`, `1-4w`, `1-3m`, `3m-plus`, `unknown` oder `not-applicable`. |
| `modification_ratio_band` | `none`, `under-1pct`, `1-5pct`, `5-10pct`, `10-20pct`, `20-50pct`, `over-50pct`, `unknown` oder `not-applicable`. |
| `sample_fraction_band` | `full`, `75-99pct`, `50-74pct`, `25-49pct`, `under-25pct`, `unknown` oder `not-applicable`. |
| `statistics_scope` | `global`, `partition`, `subpartition`, `session`, `local-member`, `logical-dataset`, `database-resource`, `structured-dataset`, `selected-object` oder `unknown`. |
| `size_method`, `size_quality`, `size_scope`, `size_accounting`, `size_visibility` | Beschreiben Sie gesondert, woher die Größe stammt, ob es sich um eine Zählung oder eine Schätzung handelt, ob sie LOB/index-Speicher umfasst, ob sie zugewiesen oder logisch ist und ob die Sichtbarkeit vollständig, teilweise, nicht verfügbar oder unbekannt ist. |

Der oberste Statistikblock verwendet `visibility = "full"`, `"partial"` oder `"unknown"` für die oben beschriebene, nicht-leere Gesamtzahl der Kopien. Er verwendet niemals nur ausgeschlossene Objekte, um `full` Sichtbarkeit zu erreichen.

Oracle `oracle-segment-bytes`-Beweise sind nur mit `exact-counter`, `allocated-segment`, vollständiger oder teilweiser Sichtbarkeit und einem `segment_state` gültig, das beweist, dass die Segmentzählung zugeordnet wurde (`created`, `deferred`, `mixed` oder `mixed-table-and-index`). Ein logischer Fallback verwendet stattdessen `oracle-table-logical-estimate`, `engine-estimate`, `logical-estimate`, teilweise Sichtbarkeit und einen nicht verfügbaren Segmentstatus. Dies verhindert, dass eine nicht zugeordneten Speicherklasse zu einem gemessenen Nullwert wird. Der Status wird aus den zugeordneten Katalogdaten abgeleitet, bevor eine Anonymisierung durchgeführt wird. `created` kann daher zusammen mit serialisierten Null-Tabellendaten vorliegen, wenn ein bekannter, positiver Roh-Tabellenzähler unter dem ersten Byte-Bucket liegt. Teilweise gemessene Oracle-Beweise verwenden `size_scope = "unknown"`: die zugeordneten Bytes bleiben exakt, aber ein fehlender LOB, eine verschachtelte Speicherung oder eine Indexzuordnung bedeutet, dass der Collector nicht ehrlich den vollständigen table/LOB/index-Umfang beanspruchen kann. Für Oracle bedeutet `mixed`, dass eine positive, zugeordnete Indexzuweisung unterdrückt wurde, um sie als serialisierte `index_bytes = 0` zu kennzeichnen; dies unterscheidet die Null von einer Tabelle, für die die Segmentzählung keine Indexzuordnung gefunden hat. `mixed-table-and-index` bedeutet, dass sowohl die Tabellen- als auch die Indexzuordnungen positiv waren, bevor sie gerundet wurden, und beide serialisierten Zähler Null sind, wobei beide Fakten erhalten bleiben, ohne dass Unter-Byte-Werte offengelegt werden. `deferred` bedeutet, dass die zugeordneten Roh-Zähler Null waren und beide serialisierten Byte-Werte ebenfalls Null sein müssen. Verwenden Sie den Status, um eine gerundete Unter-Byte-Zuordnung von einer Speicherung zu unterscheiden, für die nachweislich kein Material vorhanden ist.

Der oberste Block zeichnet auch sortierte, disjunkte Kataloge und geschlossene Einschränkungen auf. Ein Wert von `rows` oder `table_bytes` kann nur als beobachtete Null verwendet werden, wenn entsprechende quality/visibility-Nachweise vorliegen; ignorieren Sie nicht den Provenienz-Block. Eine gezählte Tabelle, deren Zeilen- oder Größenqualität `unavailable` oder `unknown` ist, führt dazu, dass die Vollständigkeit des entsprechenden Datensatzes von `complete` abweicht; der Validator verwirft einen numerischen Platzhalter, der als vollständige Abdeckung dargestellt wird. Insbesondere hat eine PostgreSQL-Tabelle ohne Optimierungsstatistiken oder einen nachgewiesenen vollständigen, begrenzten Lesezugriff ein unbekanntes Zeilenvolumen, nicht eine gemessene Null.

Oracle Basic lässt `check_count` weg, wenn das Datenwörterbuch eine deklarierte `NOT NULL`-Einschränkung nicht von einer expliziten, textlich identischen `CHECK` unterscheiden kann. Es schließt nicht aus dem generierten Namen einer Einschränkung oder der aktuellen Nullbarkeit der Spalte. Andere Tabellen, deren Einschränkungszeilen eindeutig sind, können dennoch eine genaue Anzahl enthalten.

## `[activity_snapshot]`

DBWarp Blueprint 1.6 schreibt diesen Block nicht.

## `[network]` (optional)

Die Round-Trip-Zeit erstreckt sich von der Maschine, auf der der Collector ausgeführt wird, zu Ihrer Datenbank. Dies ist nicht die Round-Trip-Zeit zwischen der Quelle und dem Ziel der Migration.

Die Messung erfolgt nach dem Verbindungsaufbau und vor den Katalogabfragen, damit die Zeiten nicht durch das Aufwärmen des Abfragecaches verfälscht werden. Sie führt **5× `SELECT 1`** aus und gibt die Medianlatenz aus. Jede Abfrage `SELECT 1` liefert ausschließlich die konstante Ganzzahl 1; bei dieser Messung werden keine Zeilendaten gelesen.

Fehlt, wenn `--no-rtt-probe` verwendet wird oder wenn die Prüfung selbst während der Ausführung fehlschlägt (wird als nicht-fataler Fehler in stderr und im Audit-Protokoll protokolliert; die Blueprint-Datei wird weiterhin ohne diesen Block ausgegeben).

| Feld | Typ | Genauigkeit |
|---|---|---|
| `sample_count` | int | exakt (in v1 immer 5) |
| `connect_total_ms` | int | Gesamte verstrichene Zeit vom Beginn des TCP-Verbindungsaufbaus bis zur Bereitschaft der authentifizierten Sitzung in Millisekunden. Umfasst TCP-Handshake, TLS-Handshake, sofern zutreffend, und Authentifizierungs-Challenge/-Response. Auf die nächste Millisekunde gerundet. Typischerweise 3–6× `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | Medianlatenz eines einzelnen Roundtrips aus den 5 Stichproben `SELECT 1` in Millisekunden. Auf die nächste Millisekunde gerundet. Das natürliche Grundrauschen des Netzwerks (in der Praxis ≥ 1 ms) ist größer als die Rundungsgranularität. Dadurch wird jeder verdeckte Kanal über niederwertige Bits beseitigt, ohne nützliche Genauigkeit zu verlieren. LAN-Werte unter einer Millisekunde fallen auf 0 oder 1 zusammen. |
| `query_rtt_ms_p95` | int | 95. Perzentil der 5 Stichproben nach der Nearest-Rank-Methode (die langsamste Beobachtung) in Millisekunden. Auf die nächste Millisekunde gerundet. Zusammen mit p50 hilft der Wert, kurze Latenzspitzen zu erkennen; fünf Stichproben dienen nur zur Orientierung und sind kein Workload-Leistungstest. |

Die fünf Messabfragen erscheinen im Auditprotokoll als **ein einziger zusammenfassender Eintrag** (nicht fünf getrennte Zeilen) mit der Bezeichnung `5x SELECT 1 (RTT probe; constant integer 1, no row data)`. Dies entspricht der Vertrauensannahme, dass keine Zeileninhalte gelesen werden.

## `[tables.<id>]`

Der Bezeichner ist `table-NNN`, wobei `NNN` die 1-basierte Ordnungszahl in einer durch Domänen getrennten HMAC-SHA256-Reihenfolge von Schema- und Tabellennamen ist. Der Standard-Schlüssel wird für den Prozess neu generiert und niemals ausgegeben. Das Übergeben des gleichen geschützten `--anonymization-key-file` bewahrt die Reihenfolge über genehmigte Vergleichsläufe hinweg. Schema v7 erfordert den vollständigen, dichten Satz von Ordnungszahlen von `table-001` bis zur ausgegebenen Tabellenzahl (die Breite wächst natürlich bei `table-1000`); übersprungene, Null-, Nicht-Dezimal- oder aus der Quelle abgeleitete Suffixe sind ungültig.

| Feld | Typ | Genauigkeit/Werte |
|---|---|---|
| `rows` | int | Katalogschätzungen werden gerundet: auf die nächste 100 (≤10.000), 1.000 (≤1.000.000), 10.000 (>1.000.000). Eine bekannte positive Schätzung, die andernfalls auf Null gerundet würde, verwendet den ersten nicht-null Wert (`100`); Null ist reserviert für einen Katalog mit dem Wert Null oder für nicht verfügbare Daten, die durch die angrenzenden Qualitätsmetriken identifiziert werden. Wenn ein begrenzter Tier-2-Lesezugriff beweist, dass er die vollständige, sichtbare Tabelle aufgezählt hat, ist `rows` die genaue Zahl, die bereits durch die genaue `sample_rows` dieser Stichprobe angegeben wurde; dies vermeidet widersprüchliche Tabellen-, Kardinalitäts- und Aggregatzählungen, ohne einen neuen Kanal hinzuzufügen. |
| `table_bytes` | int | nach Größenordnung gerundet: nächste 1KiB / 1MiB / 100MiB |
| `index_bytes` | int | wie `table_bytes` gerundet |
| `schema` | String. | Anonymisierte IDs: `schema-A`, `schema-B`, ..., `schema-AA`. Schema v7 erfordert eine dichte, alphabetisch geordnete Mengenbildung für jedes Schema, auf das eine ausgegebene Tabelle oder ein graph/analyzed-Artefakt verweist; ein ausgewähltes Schema, das nur Nicht-Tabellen-Objekte enthält, wird daher beibehalten. |
| `object_kind` | String. | V7 benötigte ein geschlossenes Token: `ordinary-table`, `materialized-view`, `external-table`, `temporary-table`, `nested-table` oder `object-table`. Die Objektidentität ist unabhängig vom physischen Speicher und der Partitionierung. |
| `storage_organization` | String. | V7 benötigt ein geschlossenes Token: `heap`, `index-organized`, `clustered`, `external` oder `unknown`. `external` ist nur für `object_kind = "external-table"` gültig. |
| `partitioning` | String. | V7 benötigte ein geschlossenes Token: `none`, `range`, `list`, `hash`, `interval`, `reference`, `composite`, `system`, `key`, `linear-hash`, `linear-key` oder `unknown`. |
| `segment_state` | String. | V7 benötigt ein geschlossenes Token: `created`, `deferred`, `mixed`, `mixed-table-and-index`, `unavailable` oder `unknown`. Dies unterscheidet Objekte, die nur Metadaten enthalten, von materialisierten Speicher. Es ist ein kategorischer Beweis, der vor der Rundung der Bytes erbracht wird, sodass `created` möglicherweise zusammen mit null serialisierten Tabellenbytes eine positive Sub-Bucket-Zuweisung begleitet. Mit Oracle-Segment-Zähler-Beweisen zeichnet `mixed` eine positive, zugeschriebene Index-Zuweisung auf, deren serialisierte Größe `index_bytes` auf null gerundet wurde; `mixed-table-and-index` zeichnet auf, dass beide Roh-Zuweisungen positiv waren, während beide serialisierten Zähler auf null gerundet wurden. |
| `parent_table`, `child_tables` | String / Array | Optionale, wechselseitige, anonymisierte Tabellenverknüpfungen für verschachtelte, partitionierte oder anderweitig eingeschlossene Objekte. Kind-IDs sind sortiert und eindeutig; der übergeordnete Graph muss azyklisch sein. |
| `table_features` | Array. | Sortierte, geschlossene Token: `graph-edge`, `graph-node`, `memory-optimized`, `temporal-current` oder `temporal-history`. |
| `unlogged` | bool | Optionale Beobachtung des protokollierten Zustands von PostgreSQL. Wird weggelassen, wenn nicht erfasst; `false` bedeutet explizit, dass der Katalog bewiesen hat, dass die Tabelle protokolliert wird. |
| `partition_count` | int | Die genaue Anzahl der physischen Blattpartitionen, die im Geltungsbereich liegen, ist erforderlich, wenn `partitioning` eine bekannte Partitionierungsstrategie angibt. PostgreSQL meldet rekursive Blattpartitionen und schließt Blätter außerhalb der aufgelösten Schemata unter `selection-limited` aus. MySQL zählt Unterpartitionen bei zusammengesetzten Tabellen, da diese ihre physischen Blätter darstellen; beispielsweise melden vier oberste Partitionen mit jeweils acht Unterpartitionen `32`. Null ist nur für eine logische Partitionswurzel mit `segment_state = "unavailable"` und ohne Blattpartition im Geltungsbereich gültig. |
| `partition_key_cols` | Array von Ganzzahlen. | Vollständige, einfache Ordnungszahlen für Partitionsschlüsselspalten. Werden für Schlüssel ausgelassen, die vollständig oder teilweise auf Ausdrücken basieren, oder wenn keine Kataloginformationen verfügbar sind; eine teilweise Ordnungszahlenliste und Schlüssel-Ausdrücke werden niemals serialisiert. |
| `partition_rows_max` | int | Optionale, gerundete Schätzung der größten Blattzeile. Für Schätzungen von Tabellensummen wird ein bekannter, positiver Wert verwendet, der den ersten nicht-null-Zeilenbereich verwendet, begrenzt durch `rows`. Bei einer exakten Tabellenauswertung wird eine Schätzung der größten Blattzeile, deren Datenschutzkategorie null oder die exakte Auswertung überschreiten würde, als nicht darstellbar verworfen und nicht auf einen falschen Wert gesetzt. Wenn vorhanden, darf sie nicht null sein, solange `rows` positiv ist oder `rows` überschreitet. |
| `temporal_history` | String. | Die anonyme Tabellen-ID der zugehörigen temporären Historie-Tabelle ist erforderlich, sofern die `temporal-current`-Funktion verwendet wird, es sei denn, diese Tabelle enthält ein entsprechendes, objektspezifisches `table_limitations`-Token. Eine globale Auswahl allein entbindet niemals von der Verknüpfung. |
| `table_limitations` | Array. | Sortierte, vollständige Objektnachweise. `table-classification-unavailable` identifiziert eine Tabelle, deren Objekttypen unvollständige Eingaben aufwiesen. `column-inventory-unavailable` identifiziert eine Tabelle mit einem oder mehreren fehlenden oder nicht lesbaren Spalteneinträgen; `dependent-structure-suppressed` besagt, dass weder der Index noch die Beziehungsstruktur für diese Tabelle als vollständig angesehen werden können, da eine emittierte Spalte fehlt. `index-inventory-unavailable` und `relationship-inventory-unavailable` beschränken eine reine Verfeinerungslücke auf die betroffene abhängige Familie, ohne dabei das erforderliche Spalteninventar zurückzuziehen. Jeder Index, Partitionsschlüssel oder jede Beziehung, die auf eine fehlende, emittierte Spalte verweist, wird weggelassen, anstatt die gesamte Erfassung ungültig zu machen. `relationship-target-outside-selected-scope` protokolliert, dass mindestens ein deklarierter Fremdschlüssel für diese Tabelle ein Objekt außerhalb des aufgelösten, ausgewählten Schemas anspricht; er ist nur in einer `selection-limited`-Erfassung gültig. `relationship-target-visibility-unknown` protokolliert, dass der Katalog ein Fremdschlüsselziel offengelegt hat, das im sichtbaren Inventar nicht aufgelöst werden konnte; die Vollständigkeit der Beziehung muss dann unvollständig sein. `row-security-filter-active` protokolliert ein sichtbares, aktiviertes SQL Server-Filterprädikat. `row-security-visibility-unknown` protokolliert, dass die vollständige Sichtbarkeit des SQL Server-Sicherheitspolicy-Katalogs nicht bewiesen werden konnte, sodass eine Stichprobenentnahme der Stufe 2 unterdrückt wird, anstatt einen potenziell gefilterten Teil als die Tabellendefinition zu behandeln. `temporal-history-outside-selected-scope` ist nur für eine nicht verknüpfte, aktuelle Tabelle in einer `selection-limited`-Erfassung gültig, nachdem der Collector das History-Schema außerhalb der Auswahl aufgelöst hat. `temporal-history-visibility-unknown` protokolliert, dass der Katalog eine History-Objekt-ID, aber nicht genügend Metadaten offengelegt hat, um sie aufzulösen. |
| `counted_in_totals` | bool | Ausgelassen bedeutet enthalten. Ein `external-table`, `materialized-view`, `temporary-table`, oder Tabelle, die `memory-optimized` enthält, erfordert eine explizite Angabe von `false` und schließt externe, abgeleitete, sitzungsspezifische oder derzeit nicht gemessene Daten von `table_count`, `row_count`, `table_bytes` und `index_bytes` aus. Für jedes Objekt bleiben Nachweise verfügbar, um die Wiederherstellung zu planen, ohne nicht verfügbare Werte als gemessene Gesamtzahlen darzustellen. Kein anderer expliziter Wert ist der Standard. |
| `check_count` | int | Optional, genaue Anzahl der strukturellen CHECK-Constraints. Wenn dieser Wert fehlt, bedeutet dies "unbekannt"; `0` bedeutet, dass der entsprechende Katalog keine gefunden hat. |
| `has_clustered_index` | bool | für PostgreSQL immer `false` |
| `[tables.<id>.statistics]` | Unter-Tabelle | Erforderliche v7-Provenienz für die Zeilenzahl, den Zustand der Optimierungsstatistiken und die Größenangaben. Das Feld `stats_freshness` der Version 6 wird nur beim Lesen älterer Dateien akzeptiert und wird in Version 7 niemals ausgegeben. |
| `[tables.<id>.cols.<cid>]` | sub-tables | eine je Spalte |
| `[tables.<id>.idxs.<iid>]` | sub-tables | eine je Index |
| `[tables.<id>.compression]` | sub-table | nur bei Tier 2 |

## `[tables.<id>.cols.<cid>]`

Der Bezeichner ist `col-N`, wobei `N` die natürliche Attributreihenfolge der Spalte ist (1-basiert, wobei die ursprüngliche Reihenfolge auf der Festplatte beibehalten wird). Bleibt über mehrere Durchläufe hinweg konstant. In Schema v7 muss das Dezimalsuffix genau `ordinal` entsprechen; Nullwerte, führende Nullen und aus der Quelle stammende Bezeichnungen sind ungültig. Quellsysteme können Lücken in den physischen Spaltennummern aufweisen, nachdem eine Spalte gelöscht wurde.

| Feld | Typ | Hinweise |
|---|---|---|
| `ordinal` | int | dasselbe N wie in der ID |
| `type` | string | normalisierte Typfamilie wie `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"` oder `"user-defined"`. Echte Namen von Domains, Enums, Aliasen, zusammengesetzten und benutzerdefinierten Typen werden nicht ausgegeben. |
| `nullable` | bool |  |
| `value_source` | string | Optionales geschlossenes Token in Schema v6: `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time` oder `rowversion`. Bei gewöhnlich gelieferten Werten oder unbekannter Evidenz nicht vorhanden. |
| `has_default` | bool | Optionale Katalogbeobachtung in Schema v6. Nicht vorhanden bedeutet unbekannt; explizites `false` bedeutet, dass der Katalog keinen Standardwert gefunden hat. |
| `default_kind` | string | Optionale Klassifizierung `constant`, `function` oder `expression` in Schema v6; nur mit `has_default = true` gültig. Text und Literale des Standardwerts werden nie ausgegeben. |
| `default_on_null` | bool | V7 optionale Quell-Katalog-Beobachtung für Oracle `DEFAULT ON NULL`; gültig nur, wenn ein Standardwert vorhanden ist. "Omitted" bedeutet, dass keine Beobachtung erfolgt ist. |
| `type_kind` | string | Optionales geschlossenes Token in Schema v6: `enum`, `set`, `domain`, `composite`, `array`, `range` oder `alias`. Bei einem Basistyp oder unbekannter Evidenz nicht vorhanden. |
| `member_count` | int | Exakte positive strukturelle Elementanzahl in Schema v6; nur für `enum` und `set` erforderlich. Elementnamen werden nie ausgegeben. |
| `domain_has_check` | bool | Optionale Domain-CHECK-Beobachtung in Schema v6; nur mit `type_kind = "domain"` gültig. |
| `hidden`, `invisible`, `masked`, `encrypted`, `sparse` | bool | Optionale Katalogbeobachtungen. `invisible` unterscheidet sich von einer vom System erstellten versteckten Spalte. "Ausgelassen" bedeutet unbekannt; explizit `false` bedeutet, dass der Katalog die Abwesenheit dieser Eigenschaft bewiesen hat. |
| `has_check` | bool | Optionale Beobachtung eines einspaltigen CHECK in Schema v6. Jedes explizite `true` wird durch `check_count` der Tabelle abgedeckt. |
| `null_fraction` | Gleitkommazahl. | Optional, beobachteter Anteil von Nullwerten von `0.0` bis `1.0`. Wenn die Kardinalität vorhanden ist, wird dieser Wert aus den öffentlich zugänglichen, auf Datenschutz optimierten Zählungen dieses Blocks abgeleitet; andernfalls wird er unabhängig gerundet. Es wird kein Null-Bitmap gespeichert. |
| `native_type` | string | Optionaler bereinigter Engine-Basistyp wie `varchar` oder `longtext`; keine Bezeichner, Enum-Mitglieder, Standardwerte oder Ausdrücke. Wird von den nativen MySQL- und SQL-Server-Collectors ausgegeben. |
| `declared_max_chars` | int | Optional deklarierte Zeichenkapazität. Exakt für Katalogwerte von PostgreSQL `character`/`character varying` und in den standardmäßigen ausgewogenen/exakten MySQL-Modi; nur bei MySQL mit `--length-fidelity strict` grob gerundet. |
| `declared_max_bytes` | int | Optional deklarierte Bytekapazität. Exakt in den standardmäßigen ausgewogenen/exakten MySQL-Modi; nur bei `--length-fidelity strict` grob gerundet. |
| `length_semantics` | String. | V7 optionale deklarierte Längeneinheit: `characters`, `bytes`, `not-applicable` oder `unknown`. Dies bewahrt die Oracle-Semantik von CHAR im Vergleich zu BYTE, ohne Deklarationen zu serialisieren. |
| `numeric_model` | String. | V7 benötigt eine geschlossene Familie: `integer`, `fixed-decimal`, `unconstrained-decimal`, `decimal-float`, `binary-float`, `not-applicable` oder `unknown`. `not-applicable` kennzeichnet einen bekannten, nicht-numerischen Datentyp; `unknown` ist für einen numerischen oder benutzerdefinierten Datentyp reserviert, dessen Semantik nicht klassifiziert wurde. `decimal-float` enthält exakte Oracle-`FLOAT(p)`-Werte und ist keine IEEE-Gleitkommazahl. |
| `numeric_precision` | int | Optionale, positiv deklarierte Genauigkeit, begrenzt durch die Engine und das Modell der Quelle: Oracle `NUMBER` und SQL Server mit Dezimalgenauigkeit bis zu 38, Oracle `FLOAT(p)` bis zu 126 Binärstellen, MySQL mit Dezimalgenauigkeit bis zu 65 und PostgreSQL mit numerischer Genauigkeit bis zu 1.000. |
| `numeric_scale` | int | Optional, signierte, deklarierte Skalierung, validiert anhand der Quell-Engine. Oracle `NUMBER` verwendet `-84..127`; PostgreSQL unterstützt dessen breiteren, versionsabhängigen Deklarationsbereich, während MySQL, SQL Server, Parquet und Avro eine nicht-negative Skalierung erfordern, die nicht größer als die Genauigkeit ist. Negative Oracle/PostgreSQL-Skalierungen und Skalierungen, die größer als die Genauigkeit sind, werden beibehalten, sofern die Engine dies zulässt. |
| `numeric_precision_radix` | String. | `decimal` oder `binary`, falls dies durch das numerische Modell erforderlich ist. Oracle `FLOAT(p)` verwendet binäre Genauigkeit mit dem exakten `decimal-float`-Wertmodell; `BINARY_FLOAT` und `BINARY_DOUBLE` verwenden `binary-float`. |
| `numeric_unsigned`, `bit_width` | bool / int | Optionale Integer-Semantik, sofern sie von der Quell-Engine bereitgestellt werden. |
| `datetime_precision` | int | Optionale, von der Engine deklarierte Genauigkeit der Sekundenbruchteile für Datum/Uhrzeit. |
| `charset`, `collation` | string | Optionale bereinigte Zeichenmetadaten. MySQL gibt die Katalognamen für Zeichensatz und Sortierung aus. SQL Server gibt `utf-16le` für `nchar`/`nvarchar`/`ntext`, `utf-8` für Codepage 65001, `windows-N` für Windows-Codepages 1250–1258 oder `code-page-N` für eine andere positive Katalog-Codepage sowie den Katalognamen der Sortierung aus. Dies sind Kodierungsfakten und Katalognamen, niemals Ihre Bezeichner oder Werte. |
| `len_avg` | int | Beprobte durchschnittliche Byteanzahl variabler Werte. Die standardmäßigen relativen Buckets haben einen maximalen Fehler von ungefähr 3,2 % und bewahren Werte bis 32 Byte exakt; exakt mit `--length-fidelity exact --yes`; grobe Rundung auf die nächsten 10 nur im strikten Modus. 0 = feste Länge oder nicht gemessen. |
| `len_p95` | int | Beprobtes 95. Perzentil mit denselben standardmäßigen relativen Buckets; exakt mit `--length-fidelity exact --yes`; grobe Rundung auf die nächsten 100 nur im strikten Modus. 0 = nicht gemessen. |
| `style` | string | Nur Tier 2. Einer der Werte `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"` oder `"precompressed"`; leer, wenn nicht klassifiziert. `"precompressed"` wird nur für eine materiell byte-dominierende Stichprobe binärer Werte mit erkannten Standardsignaturen von Containern ausgegeben. Die erkannte Containerfamilie wird absichtlich nicht offengelegt. |
| `[tables.<id>.cols.<cid>.lob_storage]` | Unter-Tabelle | V7 optionale Datenbank-LOB-Speicher-Nachweise: `storage_class` (`basicfile`, `securefile`, `external`, `unknown`), Kompression (`none`, `low`, `medium`, `high`, `not-applicable`, `unknown`), Deduplizierung (`enabled`, `disabled`, `not-applicable`, `unknown`), optionale in-row/encrypted-Optionen und Sichtbarkeit (`full`, `partial`, `unknown`). Externe Inhalte erfordern, dass die beiden Speichersteuerungen auf `not-applicable` gesetzt sind, und Datenbank-interne Optionen werden weggelassen. Kein Pfad- oder Segmentname wird beibehalten. |
| `magnitude_min`, `magnitude_max` | int | Optionale vorzeichenbehaftete Dezimalexponenten in Schema v6, welche die Größenordnung beprobter numerischer Nicht-NULL-Werte begrenzen. Sie werden zusammen mit `has_negative` ausgegeben; exakte Werte werden nie ausgegeben. |
| `has_negative` | bool | Optionale Beobachtung des Vorzeichens in Schema v6; nur zusammen mit beiden Größenordnungsgrenzen ausgegeben. |
| `time_span` | string | Optionaler beprobter Datums-/Zeitbereich in Schema v6: `intraday`, `days`, `weeks`, `months`, `years` oder `decades`. |
| `time_recent_decade` | int | Jahrzehnt des neuesten beprobten Datums-/Zeitwerts in Schema v6; nur mit `time_span` ausgegeben und immer durch 10 teilbar. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Nur Tier 2. Für beprobte Text-/Binärkandidatenspalten vorhanden. Derselbe Feldaufbau wie bei der Komprimierung auf Tabellenebene, aber auf eine anonymisierte Spalte begrenzt. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Zusammenfassung der Verteilung beprobter Werte in Schema v3. Enthält ausschließlich begrenzte oder gerundete Anzahlen und Häufigkeiten. |

`numeric_model` ist maßgeblich für die numerische Semantik. `type` behält die Schreibweise der Engine-Familie bei: Oracles `NUMBER`-Familie wird von `type = "number"` begleitet, und `FLOAT(p)` wird von `"float"` begleitet, während `native_type` die bereinigte ursprüngliche Deklaration beibehält.

### `[tables.<id>.cols.<cid>.cardinality]` (Schema v3)

Wenn die Zeilenabtastung aktiviert ist, speichert der Collector maximal 8.192 temporäre 64-Bit-Fingerabdrücke pro Spalte im Speicher, berechnet aggregierte NDV/skew-Statistiken und verwirft die Fingerabdrücke. Weder Werte noch Fingerabdrücke werden serialisiert. Der Block enthält `measured`, `sample_rows`, `non_null_rows`, `observed_distinct_count`, `estimated_distinct_count`, `top_value_fraction`, `frequency_p50`, `frequency_p95`, `frequency_p99`, `frequency_max`, `sample_method`, `complete_source_read`, `sample_layout`, `sampled_with_bias` und `bias_reason`. `complete_source_read = true` ist ein maschinenlesbarer Nachweis dafür, dass eine begrenzte Aussage die vollständige, sichtbare Quelle erfasst hat und diese Spalte ohne Wertreduzierung beibehalten hat. Ein vollständiger Lesezugriff auf eine Zeile stellt sicher, dass `sample_rows` innerhalb des exakten Zeilenbereichs der Tabelle erhalten bleibt, selbst wenn eine Zellbegrenzung oder ein begrenzter Fingerabdruckspeicher `complete_source_read` falsch macht; Die genaue Tabellenausfüllung ist bereits in `tables.<id>.rows` enthalten, daher wird hier keine zusätzliche Information preisgegeben. `non_null_rows` wird zuerst mit Blick auf den Datenschutz verarbeitet, und `null_fraction` wird dann aus `(sample_rows - non_null_rows) / sample_rows` abgeleitet. Der Bruchteil bleibt daher exakt konsistent mit den öffentlich verfügbaren Werten und kann aus dem separaten Raster mit dem Wert 0,005 entfernt werden, ohne dabei weitere Informationen preiszugeben. Exakte Nullwerte und alle Nicht-NULL-Endpunkte werden beibehalten. Eine gemischte Zählung behält eine positive, nicht-NULL-Population positiv und bleibt unterhalb von `sample_rows`. Mixed `non_null_rows` verwendet das gleiche, auf die Magnitude bezogene Zählungsraster wie die anderen Kardinalitätswerte. Am unteren Ende des Wertebereichs kann dies dazu führen, dass die Anzahl um bis zu einen vollständigen Zählbereich unterhalb der tatsächlichen gespeicherten Datenmenge liegt (zum Beispiel `9,728` für `9,999`). es ist keine nahezu exakte Zählung. Der genaue, vollständig nicht-NULL-Wert zeigt explizit, dass in den gespeicherten Zeilen keine NULL-Werte beobachtet wurden, während jedes beobachtete NULL-Wert die Anzahl unter `sample_rows` hält. Die eindeutigen Werte und Häufigkeitsangaben bleiben innerhalb ihrer dokumentierten Datenschutzrichtlinien, auch wenn sie durch diese Population begrenzt sind. Für Datenbankquellen begrenzt eine unvollständige Leseoperation `sample_rows` auf die gerundete Schätzung der Anzahl der Katalogeinträge. Für Parquet und Avro ist die genaue Anzahl der Footer-Zeilen die Obergrenze. In beiden Pfaden ist `sample_rows` die exakte, bereits offengelegte Anzahl der beibehaltenen Zeilen, die bereits durch den Komprimierungsblock auf Tabellenebene angezeigt wird `sample_rows`, es sei denn, diese Obergrenze ist niedriger. Es darf niemals so interpretiert werden, dass vollständige Abdeckung impliziert wird. Die Beschneidung von Werten erhält die Klarheit und die Aussagekraft der Häufigkeitsdaten und darf diese niemals über die tatsächliche Datenmenge in der Tabelle hinaus erhöhen. Schließen Sie keine Vollständigkeit aus, indem Sie `sample_method` analysieren. `sample_layout` ist eine optionale, maschinenlesbare Enumeration. Der aktuell ausgegebene Wert ist `primary-key-range-windows`; "Abwesenheit" bedeutet, dass kein Bestellvertrag verfügbar ist. Leiten Sie keine Semantik aus der Analyse des menschenlesbaren `sample_method`-Felds ab.

Anzahlen und Brüche werden, wo angebracht, datenschutztechnisch angepasst. Die Statistiken beschreiben die Duplizierungsdichte, die Verteilung von häufigen Werten und endliche Bereiche. Sie enthalten keine Stichprobenwerte, aber charakteristische Verteilungen können eine Arbeitslast identifizieren; behandeln Sie sie nicht als irreversibel oder als Beweis dafür, dass keine geschäftlichen Erkenntnisse aus externem Wissen abgeleitet werden können. Eine begrenzte Aussage kann die sichtbare Anzahl von Zeilen beweisen, ohne zu beweisen, dass jede Stichprobenzelle vollständig beibehalten wurde. Wenn eine serverseitige Zellengrenze eine Spalte abschneidet, bleibt ihre Kardinalität eine begrenzte, verzerrte Schätzung, auch wenn die Anzahl der Zeilen in der Tabelle aus einem vollständigen, begrenzten Lesezugriff erfasst wird; unbeeinflusste Spalten können weiterhin die Kardinalität eines vollständigen Lesezugriffs aufweisen.

### `[tables.<id>.cols.<cid>.compression]` (nur Tier 2)

Spaltenweise Komprimierung wird nur für begrenzte text/binary-Kandidaten ausgegeben, wenn `--measure-compression --yes` verwendet wird. Sie liefert eine Schätzung der spaltenweisen Komprimierung.

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
sample_method = "TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "server_side_cell_cap"
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

## `[tables.<id>.idxs.<iid>]`

Der Bezeichner ist `idx-N`, wobei `N` die 1-basierte Ordnungszahl des Index innerhalb der Tabelle ist, sortiert nach einem domänentrennten HMAC-SHA256 des Indexnamens. Schema v7 benötigt für jede Tabelle das dichte Set `idx-1` bis `idx-N`; Nullen, führende Nullen, Lücken und nicht-dezimale Suffixe sind ungültig.

| Feld | Typ | Werte |
|---|---|---|
| `type` | string | Normalisierte Familie von Indexmethoden wie `"btree"`, `"hash"`, `"gin"`, `"gist"`, `"brin"`, `"spgist"`, `"fulltext"`, `"spatial"`, `"clustered"`, `"nonclustered"`, `"clustered columnstore"`, `"nonclustered columnstore"` oder `"other"`. Namen von Erweiterungs-/benutzerdefinierten Methoden werden nicht ausgegeben. |
| `primary` | bool | Optional; wird für Primärschlüsselindizes als `true` ausgegeben. Andernfalls nicht vorhanden beziehungsweise false. |
| `unique` | bool |  |
| `cols` | array of int | beteiligte Spaltenordnungszahlen in der Reihenfolge der Indexspalten |
| `prefix_lengths` | array of int | Optionale MySQL-Indexpräfixlängen, an `cols` ausgerichtet; null bedeutet die vollständige Spalte. Standardmäßig exakt; nur bei `--length-fidelity strict` abgerundet. |
| `include_cols` | array of int | Optional; Ordnungszahlen von Nichtschlüssel-INCLUDE-Spalten, sofern die Quell-Engine sie bereitstellt. |
| `expression` | bool | Optional; true, wenn Ausdrucks-/Funktionsschlüsselmaterial vorhanden ist und nicht als einfache Spaltenordnungszahlen dargestellt werden kann. |
| `filtered` | bool | Optional; true für gefilterte/partielle Indizes. |
| `descending` | bool | Optional; true, wenn eine Schlüsselspalte ausdrücklich absteigend sortiert ist. |
| `partitioning` | String. | V7 optionale physische Partitionierung: `none`, `local`, `global` oder `unknown`. |
| `visibility` | String. | V7 optionale Sichtbarkeit der Quelle: `visible`, `invisible` oder `unknown`. |
| `state` | String. | V7 optionale Betriebszustände: `usable`, `unusable`, `in-progress`, `failed` oder `unknown`. |
| `prefix_distinct_counts` | array of int | In Schema v3 geschätzte Anzahl unterschiedlicher Tupel für jedes Schlüsselpräfix von einer bis N Spalten. Null bedeutet, dass für dieses Präfix kein Wert verfügbar ist. |
| `cardinality_sample_method` | string | Begrenzte Herkunftsinformation für `prefix_distinct_counts`; abgeleitete Produkte sind ausdrücklich gekennzeichnet und werden nicht als direkte Tupelstichproben dargestellt. |

## `[tables.<id>.compression]` und `[tables.<id>.cols.<cid>.compression]` (nur Tier 2)

Der Abschnitt wird nur angezeigt, wenn die Datei mit `--measure-compression --yes` generiert wurde. Der tabellenbezogene Abschnitt misst eine neutrale, spaltenweise Projektion der vollständigen Stichprobe und stellt das maßgebliche Verhältnis für Schätzungen des Datenvolumens einer ganzen Tabelle dar. Die spaltenbezogenen Abschnitte werden aus den gleichen Stichprobenzeilen projiziert, wobei jeweils eine Spalte betrachtet wird, und zeigen, welche Spalten sich gut komprimieren lassen, ohne dabei die Stichprobenwerte preiszugeben. Sie lösen keine zusätzlichen Datenbanklesevorgänge aus.

PostgreSQL-Tabellen, die von aktiver zeilenbasierter Sicherheit gesteuert werden, einschließlich geerbter oder partitionierter Kindtabellen, deren Ursprungspolicy durch eine direkte Kindabfrage umgangen würde, sowie SQL Server-Tabellen, die von einem aktivierten Sicherheitsfilter-Prädikat gesteuert werden, werden nicht gesampelt. Ihre Kataloginformationen werden beibehalten, und die Ausführungsaufzeichnungen `DBP1407W` werden gespeichert, anstatt einen policy-gefilterten Teil als die gesamte Tabelle zu extrapolieren.

| Feld | Typ | Genauigkeit |
|---|---|---|
| `measured` | bool | immer `true`, wenn der Block vorhanden ist |
| `sample_rows` | int | exakt |
| `sample_bytes` | int | Größe des Stichprobenpuffers im Arbeitsspeicher, in **Buckets** eingeordnet: nächste **64 KiB** unter 1 MiB, nächste **1 MiB** unter 1 GiB, nächste **100 MiB** darüber. Bytes werden niemals auf den Datenträger geschrieben. Die Bucket-Einteilung beseitigt den verdeckten Kanal über niederwertige Bits je Tabelle, den ein exaktes `buf.len()` andernfalls eröffnen würde. |
| `sample_method` | string | Engine-spezifische Beschreibung der begrenzten Stichprobe, beispielsweise `"TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"`, `"LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"` oder `"SELECT TOP N bounded projection FROM <table> (compression sample; server-side cell cap)"` |
| `sampled_with_bias` | bool | true, wenn die Stichprobe nicht gleichmäßig ist, beispielsweise bei einem reinen LIMIT-Fallback |
| `bias_reason` | string | Wenn `sampled_with_bias = false`, ist dieses Feld leer. Andernfalls enthält es eine Kennzeichnung wie `"unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"`. |
| `ratio_zstd_3` | float | auf die nächsten **0.05** gerundet, gemäß der zstd-Messrichtlinie des Vertrags für Stufe 3. Gemessen an Bytes, die mit `sample_encoding` kodiert wurden. |
| `ratio_zstd_19` | Gleitkommazahl. | Nicht von dieser Version geschrieben; kann in Dateien aus früheren Versionen vorkommen. |
| `ratio_stddev` | float | auf die nächsten **0.05** gerundet, Standardabweichung der Stufe-3-Verhältnisse über begrenzte Tabellen-Probe-Frames. Projektionsblöcke auf Spaltenebene geben derzeit `0.0` aus, weil sie beratende Entropiehinweise und kein Varianzmodell darstellen. |
| `sample_encoding` | String. | Identifier für die auf Byte-Ebene verwendete Kodierung und die Komprimierungssitzungspolicy, die für die Messung verwendet wird. PostgreSQL-Live-Tabellenblöcke verwenden `"blueprint-columnar-transfer-probe-v2"`. MySQL und SQL Server verwenden `"blueprint-columnar-transfer-probe-v3"`, wobei zusätzlich bei 256-KiB-Proben-Chunk-Grenzen geleert wird. SQL Server `nvarchar`/`nchar`/`ntext`-Payloads behalten die nativen UTF-16LE-Byteweitenverteilungen bei; `varchar`/`char`/`text` behalten ihre abgetastete Bytenweite, und das Feld `charset` identifiziert die Katalog-Codepage. V1 wird als Eingabe akzeptiert. Für einzelne Spalten werden Blöcke mit `"blueprint-compression-probe-v2"` verwendet. Verhältnisse, die mit unterschiedlichen `sample_encoding`-Werten gemessen werden, sind nicht vergleichbar. |

PostgreSQL verwendet Version 2, während MySQL und SQL Server Version 3 verwenden; vergleichen Sie die Verhältnisse nur innerhalb einer einzigen Kodierung.

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

`ratio_zstd_3` beschreibt das benannte `sample_encoding`; es handelt sich nicht um eine Erfassung von Datenbankprotokoll- oder Migrationsdaten. Die Testsuite in diesem Repository validiert deterministische Kodierung, begrenzte Stichproben und Serialisierung, behauptet aber nicht, einen universellen, engineübergreifenden prozentualen Fehler für jeden Extraktionspfad zu erreichen.

Bevor Sie den Wert für eine wichtige Entscheidung bezüglich der Kapazität verwenden, überprüfen Sie diesen Wert anhand von repräsentativen Quelldaten und dem vorgesehenen Extraktionsmechanismus. Notieren Sie die Vergleichsmethode, die Stichprobengröße, den binären Hash, die Engine-Version und den beobachteten Fehler zusammen mit dem resultierenden Plan. Die grundlegende Beziehung ist `compressed_bytes ≈ sample_bytes / ratio_zstd_3` unter der Byteverteilung, die durch die aufgezeichnete Kodierung erzeugt wird.

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
ausdrücklich als abgeleitet gekennzeichnet.

## `[artifact_inventory]` (seit Schemaversion 4; erforderlich ab Version 7)

Schema v7 verwendet den unabhängig versionierten `dbwarp-blueprint-artifacts/v2`-Vertrag, um Nicht-Tabellen-Objekte zu beschreiben, ohne Quellnamen oder -definitionen zu serialisieren. Ältere Schemaversionen verwenden den v1-Vertrag. V7 gibt immer diesen Block aus: `--artifact-detail none` protokolliert explizit ein nicht angefordertes Datenbankinventar, während strukturierte Dateiquellen ein explizites "nicht anwendbar"-Inventar ausgeben. Ein fehlender Block wird daher niemals fälschlicherweise als ein verifizierter, leerer Katalog interpretiert.

Die Voreinstellung `--artifact-detail summary` gibt `object_count`,
`external_prerequisite_count`, `counts_by_kind` und
`counts_by_external_class` aus. `graph` ergänzt einen anonymen Objektdatensatz
je Artefakt und Abhängigkeitskanten. `analyzed` ergänzt begrenzte Datensätze des
Vertrags `dbwarp-language-feature-census/v1`, die vorübergehend aus verfügbaren
Definitionen abgeleitet werden. `graph` und `analyzed` erfordern ausdrücklich
`--yes`, weil die Graphtopologie eine Anwendung identifizieren kann.

`object_count` ist die Anzahl der Artefakt-Einträge, die vom Collector erzeugt werden, und nicht die Anzahl der Zeilen, die von einem einzelnen nativen Katalog zurückgegeben werden. Ein Paket oder Typ kann daher separate Spezifikations-, Body- und Member-Einträge beitragen. Ein natives Objekt, das in mehr als einem Katalog vorkommt, ist immer noch ein Eintrag: Beispielsweise werden Oracle-Trigger-Zeilen aus den Trigger- und Source-Katalogen durch ihre native Objekt-ID verknüpft, und Source-Zeilen erweitern den Trigger-Eintrag, anstatt ihn zu duplizieren.

Oracle-Pakete und Objekttypen verwenden die gleiche Datenstruktur: ein `specification`-Datensatz, ein `body`-Datensatz, der als Implementierung verknüpft ist, und ein `package_member` procedure/function-Datensatz pro Katalogeintrag, wobei die Spezifikation als übergeordnetes Element dient. Nur der Hauptteil enthält den gesamten Quelltext und die Sprachstatistik; die einzelnen Einträge behalten ihre Kataloginformationen, verwenden aber keine anwendbare Definitionanalyse. Dies verhindert, dass ein lexikalischer Analysator vorgibt, er könne den Quelltext eines Pakets in die einzelnen Einträge aufteilen.

Der Nachweis auf Inventarebene umfasst:

| Feld | Werte / Regel |
|---|---|
| `detail` | `none`, `summary`, `graph` oder `analyzed` |
| `scope` | V7: `all-visible-schemas`, `selected-schemas`, `structured-source` oder `unknown`; es muss mit den Informationen zur Schema-Auswahl übereinstimmen, die sich an anderer Stelle in der Datei befinden. |
| `visibility` | `full`, `privilege_filtered` oder `unknown` |
| `inventory_complete` | Darf nur bei vollständiger Sichtbarkeit, ohne unlesbare Kataloge und ohne deklarierte unmodellierte Familien wahr sein |
| `dependencies_complete` | Darf nur wahr sein, wenn die modellierten Abhängigkeitskataloge lesbar waren |
| `requirements_complete` | V7-Aggregat: nur bei vollständiger Abdeckung der Bewertungspopulation im ausgewählten Umfang und `requirement_status = complete | not_applicable` für jedes ausgegebene Artefakt wahr; Weglassen bedeutet falsch, und eine leere Anforderungsliste ist kein Vollständigkeitsnachweis |
| `analysis_complete` | Darf nur bei Detailgrad analyzed und nur dann wahr sein, wenn jede ausgegebene Analyse vollständig ist |
| `catalogs_read` | Geschlossene Standardbezeichnungen der erfolgreich geprüften Engine-Kataloge |
| `catalogs_unreadable` | Katalogbezeichnungen, bei denen Fehler aufgetreten sind; jeder Eintrag verhindert die Vollständigkeitsaussagen, die von diesem Katalog bereitgestellt werden, während sich die für einzelne Objekte relevanten Nachweise der Anforderungen weiterhin als vollständig erweisen können. |
| `catalogs_not_applicable` | Die V7-Katalogbezeichnungen erwiesen sich als unanwendbar; sie waren von lesbaren und nicht lesbaren Katalogen getrennt. |
| `families_not_inventoried` | Bekannte Objektfamilien, die in dieser Version nicht inventarisiert werden |

### `[artifact_inventory.complexity]` (Schemaversion v7)

Der `dbwarp-blueprint-artifact-complexity/v1`-Block ist eine aggregierte Bewertung, die sich ausschließlich auf die anonymisierten Artefakt-Daten bezieht. Er fehlt bei den Details `none` und `summary`, ist aber bei den Details `graph` und `analyzed` erforderlich, wobei seine Anwesenheit bedeutet, dass der Versuch einer Bewertung unternommen wurde. Ein Berechnungsfehler führt zu einem "Fail-Closed"-Ergebnis mit unbekanntem Status, anstatt den Blueprint zu beenden.

Die obersten Felder sind fest vorgegeben:

| Feld | Werte / Regel |
|---|---|
| `contract` | `dbwarp-blueprint-artifact-complexity/v1` |
| `assessor_version` | `1` |
| `scope` | Muss exakt `artifact_inventory.scope` entsprechen. |
| `population_policy` | `exclude-known-engine-generated-and-secondary`; fehlende Optionen bleiben gültig und temporäre Objekte bleiben gültig. |
| `assessment_population_complete` | Das ist nur dann wahr, wenn jedes Objekt, das gemäß der Populationsrichtlinie zulässig ist, bekannt ist; eine Auslassung bedeutet falsch, und diese Aussage ist unabhängig vom umfassenderen Feld `inventory_complete`. |
| `eligible_object_count` | Objekte, die von der Richtlinie bewertet werden. |
| `fully_assessed_object_count` | Jede Dimension ist entweder bekannt oder bewiesen `not-applicable`. |
| `partially_assessed_object_count` | Mindestens eine anwendbare Dimension ist bekannt und mindestens eine ist unbekannt. |
| `unassessed_object_count` | Keine anwendbare Dimension bekannt. |
| `excluded_object_count` | Objekte, die durch die festgelegte Richtlinie ausgeschlossen sind. |
| `analyzer_version` | Der einzelne Analyzer, der von der v7-Erfassung verwendet wird: `lexical-v2` oder `not-applicable` im Graphenmodus. |
| `analysis_spans` | Sortierte, eindeutige, geschlossene Intervalle, die in den zulässigen Volkszählungsdaten vorhanden sind: `executable-body`, `not-applicable` oder `unknown`; leer im Graphenmodus. |
| `dialects` | Sortierte, eindeutige, geschlossene Dialekt-Token, die in den zulässigen Volkszählungsdaten enthalten sind. |
| `grammar_profiles` | Sortierte, eindeutige Grammatikprofile, die in den zulässigen Volkszählungsdaten enthalten sind. |
| `overall_band` | `trivial`, `low`, `moderate`, `high`, `very-high`, `not-applicable` oder `unknown`. |
| `overall_score` | Nicht von dieser Version erstellt. |
| `limitations` | Sortierte, abgeschlossene Gründe, wie unten beschrieben. |

Beide Populationsgleichungen verwenden geprüfte Arithmetik:

```text
artifact_inventory.object_count = eligible_object_count + excluded_object_count
eligible_object_count = fully_assessed_object_count
                      + partially_assessed_object_count
                      + unassessed_object_count
```

`dimensions` enthält genau `volume`, `control_flow`, `feature_breadth`, `entanglement`, `environment_coupling`, `opacity` und `dialect_coupling`. Jede Dimension hat eine geschlossene `band`, einen `coverage`-Wert (`complete`, `partial`, `not-applicable` oder `unknown`) und ein festes Histogramm. Die `volume`-Dimension `band`, wie auch die anderen Dimensionsergebnisse, verwendet `trivial`, `low`, `moderate`, `high`, `very-high`, `not-applicable` oder `unknown`. Ihr Histogramm verwendet die Größen-Schlüssel `0`, `1-255`, `256-1k`, `1k-4k`, `4k-16k`, `16k-64k` und `64k+`. Die anderen sechs Histogramme verwenden die Zähl-Schlüssel `0`, `1`, `2-4`, `5-8`, `9-16`, `17-32` und `33+`. Jedes Histogramm hat außerdem `not_applicable`- und `unknown`-Buckets. Für jede Dimension erfordert die überprüfte Arithmetik:

```text
eligible_object_count = assessed evidence-band counts
                      + not_applicable
                      + unknown
```

Die Abdeckung erfolgt daher pro Dimension, nicht auf einer einzigen Ebene. Ein `not_applicable`-Zensus-Ergebnis ist ein bestimmter Nachweis und trägt zum `not_applicable`-Bereich der Dimension bei; es bedeutet nicht, dass das Objekt nicht bewertet wurde. Die Anzahl der fully/partially/unassessed-Objekte auf oberster Ebene ist eine abgeleitete Zusammenfassung: alle nicht anwendbaren Objekte sind vollständig bewertet, "teilweise" bedeutet, dass mindestens eine anwendbare Dimension bekannt ist und eine andere unbekannt ist, und "nicht bewertet" bedeutet, dass keine anwendbare Dimension bekannt ist.

Eine partielle Dimension wird als untere und obere Grenze ausgewertet. Ihr `band` ist `unknown`, es sei denn, die bekannte untere Grenze ist bereits `very-high`, da jede unbekannte Beobachtung den höchsten Bereich einnehmen kann. Dies verhindert, dass ein partielles Histogramm seine beobachtete untere Grenze als endgültiges Ergebnis darstellt.

`external_binary` ist ein Zustandsmerkmal der Sichtbarkeit von Definitionen, kein Ausschluss-Flag. Am Standort installierte Plugins, CLR-Assemblies, Java-Objekte und externe Bibliotheken bleiben weiterhin migrationsfähige Arbeit und tragen normalerweise unbekannte, definitionsabhängige Informationen bei. Nur das explizite `generated_by_engine = true`-Flag schließt ein vom System bereitgestelltes Objekt unter dem Assessor v1 aus.

Histogramme sind absichtlich eindimensional. Kreuztabellen nach Art, Merkmal, Schema oder einem anderen Attribut sind nicht Teil des Vertrags. Exakte Zählungen liefern keine zusätzlichen Informationen über die serialisierte, objektspezifische Erfassung im Analysemodus, während die feste Form verhindert, dass ein offizielles Werkzeug zur Identifizierung von Datenbeständen veröffentlicht wird.

Die Gründe für eingeschränkte Funktionalität sind `definition-analysis-not-requested`, `definitions-withheld`, `unsupported-dialect`, `wrapped-source`, `graph-incomplete`, `requirements-incomplete`, `outside-selected-scope`, `computation-limit` und `computation-failed`. `requirements-incomplete` bedeutet, dass die Erfassung der Anforderungen nicht vollständig ist. Objekte mit dem Anforderungsstatus `partial` oder `unavailable` liefern unbekannte, anstatt von Null, Informationen über die Umgebungs- und Dialektabhängigkeit; unabhängig vollständige Objekte bleiben bewertet. `unsupported-dialect` bedeutet, dass die Definition abgerufen wurde, aber ihre Sprache oder ihr Dialekt verfügt über keinen unterstützten Analysator; sie wird weder zurückgehalten noch absichtlich verschleiert. Die Detailgenauigkeit der Graphen verwendet `definition-analysis-not-requested`; sie darf keine Einschränkung beim Lesen der Definition beanspruchen, da kein solcher Leseversuch unternommen wurde.

Unbekannte Beweise werden unabhängig für jede betroffene Dimension begrenzt. Der Gesamtbereich wird nur dann ausgegeben, wenn die untere und obere Bewertung übereinstimmen. Eine vollständige, leere, zulässige Population ist `not-applicable`, niemals `trivial`. Der Graphenmodus verwendet immer einen `unknown`-Gesamtbereich für eine nicht-leere Population, da er nicht die Definitionen liest, die für die Gesamtbewertung erforderlich sind. Fehlende Graphenkanten machen den betroffenen Zusammenhangsbeweis unbekannt. `computation-limit` wird in dieser Version nicht geschrieben. Ein unerwarteter Berechnungsfehler protokolliert `computation-failed`, behält das vollständige Artefaktinventar und markiert die Gesamtbewertung als fehlgeschlagen, anstatt das Blueprint zu unterdrücken.

`assessment_population_complete` bestimmt, anstatt einer allgemeinen `artifact_inventory.inventory_complete`, ob das begrenzte Ergebnis aussagekräftig sein kann. Wenn die Bewertungsgruppe unvollständig ist, ist der Gesamtbereich `unknown`, es sei denn, die bekannte untere Grenze ist bereits `very-high`; für Objekte, die möglicherweise nicht sichtbar sind, wird keine endliche obere Grenze angenommen.

Komplett erfasste Objekte tragen `unknown` zur Deckkraft bei; sie werden nicht vom Deckkraft-Histogramm ausgeschlossen, nur weil keine vollständige Erfassung möglich war. Zeigen Sie die Deckkraft zusammen mit ihrer Abdeckung an, damit ein kleiner, beobachteter Bereich mit hoher Deckkraft keine große, unbekannte Population verbergen kann.

`unsupported-dialect` bleibt eine deutliche Einschränkung, da die Zählung zwar einen Dialekt benennen und `unavailable` angeben kann, aber keinen `unsupported`-Status hat. Sie wird nur dann abgeleitet, wenn eine Definition verfügbar war und der aufgezeichnete Dialekt nicht vom angegebenen Parser unterstützt wird. Andere Definitionseinschränkungen leiten sich ebenfalls aus der Sichtbarkeit der Definition, dem Zählstatus und den Artefaktbeweisen ab und werden nicht als unabhängige Aussage aufrechterhalten.

Die Eignung für einen Vergleich wird zwischen Dateien anhand des Komplexitätsvertrags, der Assessor-Version, der Analyzer-Version, des exakten Analysebereichs, der Dialekt- und Grammatikprofilsätze, des Gültigkeitsbereichs und der Populationsrichtlinie berechnet. Ein grobes homogeneous/mixed-Flag wird nicht serialisiert, da unterschiedliche, gemischte Sätze nicht unbedingt vergleichbar sind.

Komplexität ist immer quellenspezifisch. Ein Bundle bewahrt die Bewertung jedes einzelnen Blueprint und erstellt niemals eine komplexitätsbezogene Gruppierung oder ein Histogramm über verschiedene Engines, Analyzer-Versionen, Dialekte oder Grammatikprofile hinweg.

Objekt-IDs haben die Form `<kind>-NNN`, wie z.B. `view-001`, `package-002` oder `procedure-003`. V7 erkennt die üblichen Objektfamilien sowie Oracle-Pakete, Scheduler-Objekte, Datenbankverbindungen, Verzeichnisse, Bibliotheken, Java-Objekte, Operatoren, Index-Typen, Domänen, Annotationen und Eigenschaftsgraphen, sowie datenbankunabhängige `queue`- und `edition`-Typen. Das Dreistellige Minimum wird mit Nullen aufgefüllt, und jeder Typ hat seinen eigenen dichten Ordnungsbereich, der bei `001` beginnt; die Breite wächst über 999 hinaus. Der Datensatz enthält nur geschlossene kind/subkind/tier-Token, anonyme schema/parent-IDs, den Definitionsmodus visibility/security, optionale Gültigkeits- und Katalog-Flags, eine geschlossene Anforderungsabdeckung, eine optionale externe Voraussetzung und eine optionale Sprachstatistik. Ein übergeordnetes Element kann eine anonyme Tabelle oder ein anderes Artefakt sein, sodass eine Hierarchie von Paketen zu Prozeduren ohne Namen erhalten bleiben kann; übergeordnete Graphen müssen azyklisch sein.

V7 verwendet ein einheitliches `subkind`-Vokabular für alle Engines:

```text
ordinary, other, materialized, integer_sequence, stored_procedure,
stored_function, scalar_function, inline_table_function, table_function,
user_defined_aggregate, table_trigger, ddl_event_trigger, before_insert,
before_update, before_delete, after_insert, after_update, after_delete,
generated_column, column_default, default_constraint, check_constraint,
row_security, rewrite_rule, legacy_rule, enum, domain, composite, range,
alias_type, table_type, clr_type, clr_procedure, clr_scalar_function,
clr_table_function, clr_aggregate, clr_trigger, clr_assembly,
server_extension, loadable_udf, foreign_data_wrapper_server, foreign_table,
federated_table, external_table, external_data_source, external_file_format,
logical_replication_publication, logical_replication_subscription,
full_text_catalog, partition_scheme, partition_function, tablespace, filegroup,
database_certificate, symmetric_key, asymmetric_key, column_master_key,
column_encryption_key, database_scoped_credential, linked_server,
enabled_event, disabled_event, enabled_agent_job, disabled_agent_job,
database_synonym, specification, body, package_member, public, private,
java_source, java_class, java_resource, external_library,
user_defined_operator, domain_indextype, scheduler_job, scheduler_program,
scheduler_schedule, scheduler_chain, advanced_queuing, service_broker
```

V7 ersetzt die mehrdeutige Abhängigkeitsliste v1 durch eine sortierte, typisierte Liste `relationships`. Beziehungstypen unterscheiden zwischen Aufrufen, Leseoperationen, Schreiboperationen, table/object-Referenzen, Trigger-Besitz, Implementierung, physischer Platzierung, Sicherheit, Erweiterungsnutzung, externen binaries/services-Verbindungen und Fernzugriffen database/server. Jede Beziehung speichert einen geschlossenen Beweis-Token (`catalog-confirmed`, `dependency-confirmed`, `syntax-confirmed`, `lexical-hint` oder `unresolved`). `dependency_edge_count` muss exakt mit dem erzeugten Graphen übereinstimmen.

`requirements` Verwenden Sie geschlossene, engine-spezifische Token und einen begrenzten Zählbereich. Sie identifizieren Kompatibilitätsanforderungen wie eine Oracle-umschlossene Quelle, einen zusammengesetzten Trigger, einen Paketstatus, dynamisches SQL, eine autonome Transaktion, pipelined/parallel/aggregate Routine, eine externe Bibliothek, eine Datenbankverbindung, einen Domänenindex, einen Scheduler, object/collection/spatial/vector Typ, ein Java-Objekt oder einen Property-Graphen. Sie dienen lediglich als Planungsnachweise.

Anforderungen stammen entweder aus einem begrenzten Katalogeintrag oder einer dedizierten, engine-sensiblen Syntaxprüfung. Eine generische lexikalische Analyse erzeugt niemals eine engine-spezifische Anforderung. Jeder Schema-v7-Graph oder jedes analysierte Artefakt enthält `requirement_status = complete | partial | unavailable | not_applicable`. `complete` stellt fest, dass die Liste für dieses Artefakt vollständig ist; `not_applicable` stellt fest, dass das Anforderungsmodell nicht anwendbar ist und daher Anforderungs- und externe-Voraussetzungs-Einträge verbietet. `partial` und `unavailable` machen die Umgebungs- und Dialekt-Kopplungsbeobachtungen für dieses Objekt unbekannt. `partial` bedeutet, dass mindestens eine Fakt-Quelle erfolgreich war, ohne vollständige Abdeckung zu erzielen; `unavailable` bedeutet, dass keine Anforderungs-Quelle eine brauchbare Abdeckung hergestellt hat und daher keine bekannten Anforderungs- oder externen Voraussetzungsnachweise enthalten kann. Solche Nachweise erfordern `partial`. Dies ermöglicht es einem nicht zugänglichen Objekt, lokal zu versagen, anstatt eine nützliche Abdeckung für den Rest des Systems zu verlieren.

Der Inventar-Level `requirements_complete` ist die aggregierte Aussage. Er kann nur wahr sein, wenn jedes erzeugte Artefakt `complete` oder `not_applicable` ist und die Bewertungsmenge für den ausgewählten Umfang vollständig ist; er kann auch dann falsch bleiben, wenn jedes Artefakt `complete` ist. Interpretieren Sie ein leeres `requirements`-Array niemals als Null-Kopplung, es sei denn, der Status dieses Artefakts ist `complete`.

`unresolved_relationships` ist eine abgegrenzte Zuordnung von Gründen zu Zählungen. Sie unterscheidet zwischen Remote- und Cross-Database-Referenzen, ausgewählten Schema-Grenzen, Berechtigungs-versteckten Zielen, verschlüsselten oder zurückgehaltenen Definitionen, dynamischem SQL, mehrdeutiger Bindung, fehlenden oder unvollständigen nativen Identitäten, nicht modellierten Zielgruppen und unbekannten Fällen. Ein vollständiger Abhängigkeitsnachweis erfordert, dass diese Zuordnung leer ist. Quellobjektnamen, SQL-Text, Prinzipale, Endpunkte, Zugangsdaten, Schlüssel, Zertifikate und Binärdateien sind keine Felder im Vertrag.

Externe Voraussetzungen zeichnen eine geschlossene `class`, den
Bereitstellungsumfang, den Bedarf an nicht erfasstem Binär-/Geheimnis-/Endpunktmaterial
und eine begrenzte Kompatibilitätskategorie auf. Ihre Anzahl ist
Migrationsplanungsnachweis und keine Behauptung, DBWarp könne sie automatisch
bereitstellen oder übersetzen.

V7-Sprachzählungen verwenden `analyzer_version = "lexical-v2"` und protokollieren `analysis_span`. Der Analysator empfängt nur den ausführbaren oder deklarativen Teil, ohne den äußeren Erstellungs-Wrapper, die Identität, die Signatur, die Rückgabedeklaration und die Moduloptionen. Header-Informationen bleiben Kataloganforderungen oder Optionen. Ein Sammler, der den Teil nicht sicher isolieren kann, protokolliert `analysis_span = "unknown"` und unvollständige Informationen anstelle der Analyse des Wrappers. Eine bewiesenermaßen nicht anwendbare Definition verwendet `analysis_span = "not-applicable"`. Fehlende Bereichsinformationen werden konservativ als `unknown` interpretiert; sie werden niemals aus dem Engine- oder Objekttyp abgeleitet. Eine unterstützte Definition, die von dieser lexikalischen Implementierung analysiert wird, verwendet `status = "partial"`; fehlende oder nicht unterstützte Definitionen verwenden `unavailable`, und ein bewiesenermaßen nicht anwendbares Objekt kann `not_applicable` verwenden. Anzahl, Größe, Verschachtelung, Komplexität und Werte für undurchsichtige Bereiche sind Bereiche, nicht exakte Quellfingerabdrücke. Funktionen werden aus einem geschlossenen Vokabular ausgewählt. Der Analysator entfernt Kommentare, Literale und Anführungszeichen; er ist kein Parser, semantischer Binder oder eine Garantie für eine erfolgreiche Übersetzung.

"Wrapped PL/SQL" ist niemals ein Beweis für den ausführbaren Code. Der Collector markiert es als verschlüsselt und hält seine Bytes von der Analyse fern; der gemeinsame Analyzer lehnt auch eine PL/SQL-Einheit ab, deren Header den "wrapped"-Marker enthält, um zu verhindern, dass eine Fehlklassifizierung plausible, aber falsche Zählungsbereiche erzeugt.

Siehe [Inventar der Nicht-Tabellenartefakte](ARTIFACT_INVENTORY.md) für
Betriebsanleitung und Engine-Abdeckung.

## Steganografie-Abwehr nach Vektor

| Vektor | Abwehr |
|---|---|
| Identifizierungsreihenfolge. | Domänengetrennte HMAC-SHA256-Verschlüsselung mit einem geheimen, prozesslokalen Schlüssel verhindert Offline-Überprüfungen von Kandidatennamen. Verwenden Sie einen Schlüssel nur dann wieder, wenn stabile, läuferübergreifende Kennzeichnungen erforderlich sind. |
| Niederwertige numerische Bits | Statistiken werden standardmäßig auf die dokumentierte Genauigkeit gerundet. Der Modus für exakte Längen ist ausdrücklich, zustimmungspflichtig, wird im Auditprotokoll aufgezeichnet und muss als sensiblere Metadaten behandelt werden. |
| Zeitstempel unter einer Sekunde | Ein UTC-Zeitstempel am Anfang, nur mit Sekundengenauigkeit |
| TOML-Formatierung | Die kanonische Ausgabe verwendet eine unveränderliche Schlüsselreihenfolge und Einrückung. Sie enthält nur den Standard-Header und Producer-Kommentare, ohne aus der Eingabe abgeleitete Kommentare. |
| Zufällige Stichprobenziehung. | Sampling verwendet feste Seeds (PG's deterministische `TABLESAMPLE SYSTEM`). Unabhängig davon erfolgt die Identifikator-Anonymisierung absichtlich durch die Beschaffung eines geheimen Schlüssels vom Betriebssystem-CSPRNG, es sei denn, Sie stellen einen solchen bereit. |
| Nicht verwendete Felder | Jedes Feld ist oben dokumentiert; es gibt keine Felder „metadata“, „comment“ oder „reserved“, die unbegrenzte Daten enthalten könnten. |
| Artefakt-Quelltext und externes Material | Definitionen sind vorübergehend und werden nach begrenzter Analyse genullt; Namen, SQL-Text, Endpunkte, Providerzeichenfolgen, Anmeldedaten, Schlüssel, Zertifikate, Paketnamen und Binärdateien besitzen kein serialisiertes Feld |

## Kompatibilität der Schemaversion

Aktuelle Versionen geben das Schema der Version 7 aus. Versionen 1 bis 6 werden weiterhin akzeptiert, um die Abwärtskompatibilität zu gewährleisten. Eine v1/v2-Datei hat keine Verteilungsblöcke. Eine Version-3-Datei enthält Metadaten zur Verteilung, aber kein Inventar der Artefakte. Eine Version-4-Datei kann ein Inventar der Artefakte enthalten, stammt aber von vor den aktuellen Blueprint-Vertragsidentifikatoren. Leser normalisieren frühere Version-4-Identifikatoren bei der Eingabe und geben das Dokument mit kanonischen Blueprint-Identifikatoren erneut aus. Eine Version-5-Datei stammt von vor den Topologie- und Datensatzbereichs-Nachweisen, die in Version 6 hinzugefügt wurden. Version 6 verwendet den Topologie-Vertrag der Version 1, den Artefakt-Vertrag der Version 1, kombinierte Tabellenfelder kind/partition, eine vorzeichenlose Dezimalskala und optionale Statistiken zur Aktualität. Version 7 verwendet den Topologie-Vertrag der Version 2 und den Artefakt-Vertrag der Version 2, erfordert explizite structure/environment/statistics-Nachweise, trennt orthogonale Tabellensemantiken, unterstützt vorzeichenbehaftete Dezimalskalen und Oracle-Numerikmodelle und erfordert einen expliziten Artefakt-Inventar-Status. Außerdem wird der feste `dbwarp-blueprint-artifact-complexity/v1`-Aggregat-Vertrag reserviert, ohne ein Objekt-spezifisches Score- oder Kreuztabellierungsfeld hinzuzufügen. Leser verwerfen unbekannte zukünftige Schemaversionen mit einer klaren Upgrade-Meldung, anstatt Felder stillschweigend zu verwerfen. Leser wenden die gleiche strengen Version-7-Validierung auf eigenständige und eingebettete Blueprints an, anstatt ungültige Nachweise während des Parsens neu zu schreiben.

## Warum TOML und nicht JSON

- TOML trennt strukturelle Abschnitte lesbarer von Blattdaten (`[tables.table-001.cols.col-2]` gegenüber verschachteltem JSON).
- Unterschiede sind leichter zu erkennen: ein Schlüssel je Zeile, zusammenhängende Untertabellen anhand von Bezeichnern.
- Überprüfen Sie den Inhalt gemäß der Datenklassifizierungspolitik Ihres Unternehmens, bevor Sie ihn weitergeben.

JSON wird im SQL-Fallback-Pfad als **Zwischenformat** verwendet. Jedes Skript `sql/blueprint.*.sql` erzeugt JSON, das `blueprint_format.py` zu TOML normalisiert. Das Zwischen-JSON enthält echte Quellbezeichner; MySQL kann über `COLUMN_TYPE` außerdem enum/set-Deklarationen enthalten. Es muss daher geschützt innerhalb der Quellumgebung bleiben. Der Normalisierer verwendet standardmäßig einen neuen geheimen Schlüssel und akzeptiert für genehmigte Vergleichsläufe denselben geschützten `--anonymization-key-file`-Vertrag. Die für eine Weitergabe an DBWarp geprüfte Enddatei ist immer TOML.

## Herkunftserweiterungen für strukturierte Dateien

Wenn `engine` oder `source_kind` `"parquet"` oder `"avro"` ist, kann die Schemaversion 3 oder höher auch die folgenden, auf bestimmte Werte beschränkten Felder ausgeben. Die Leser müssen die Unterscheidung zwischen der Speicherung in der Quelldatei und den auf bestimmte Werte beschränkten, dekodierten Messwerten beibehalten; Leser, die die Schemaversion des Dokuments nicht unterstützen, müssen es mit einer Meldung zur Aktualisierung ablehnen, anstatt unbekannte Felder zu verwerfen.

V7 strukturierte Dateibluesprints erfordern vollständige `[structure_scope]`- und `[statistics_evidence]`-Blöcke, lassen `[database_topology]`, `[source_environment]` und `[activity_snapshot]` aus und geben ein explizites "nicht zutreffend" für `[artifact_inventory]` aus. Sie leiten niemals die Datenbanktopologie oder die Server-Maschinenkapazität vom Host des Collectors ab.

Blueprints strukturierter Dateien verwenden dieselben anonymisierten Bezeichner
wie Datenbank-Blueprints: `table-NNN` in mit geheimem Schlüssel erzeugter Reihenfolge und `col-N`
in Schemaordinalreihenfolge. Dateistämme, Parquet-Pfade, Avro-Feldnamen und das
Manifestfeld `logical_table` werden nicht als Tabellen- oder Spaltenbezeichner
ausgegeben.

Auf Tabellenebene ist `table_bytes` die logische Schätzung der Datenmenge, die übertragen werden muss, während `storage_bytes` die tatsächliche Größe des Quellobjekts auf der Festplatte ist. Metadata-only Parquet verwendet unkomprimierte Byte-Chunks für Spalten für `table_bytes`; optionales, dekodiertes Sampling ersetzt diese Schätzung durch die prognostizierten `blueprint-compression-probe-v2` Bytes. Avro leitet diesen Wert aus seinem vollständig dekodierten Scan ab. Die optionalen Felder `source_partitions`, `row_group_count` und `source_codec` beschreiben das Dateiformat. Mehrdateimengen aggregieren diese Werte. `row_group_count` ist spezifisch für Parquet; `source_partitions` ist `1` für ein einzelnes Eingabeobjekt.

Auf Spaltenebene ist `null_fraction` ein beobachteter Wert von `0.0` bis `1.0`.
`length_sample_rows` und `length_sample_method` beschreiben die Herkunft von
`len_avg` und `len_p95`. `source_semantics` hält begrenzte
Kompatibilitätsfakten wie `"repeated-leaf"`, `"nested-json"` oder
`"multi-type-union"` fest; es enthält niemals Ihre Feldnamen oder Werte.
Dezimalpräzision und -skala,
Zeitstempelpräzision und UTC/lokale Semantik, UUID sowie Metadaten für binäre
Werte fester Größe werden in den vorhandenen bereinigten skalaren Feldern und
`native_type` geführt.

Im Bereich der Komprimierung vergleicht `ratio_storage` auf Tabellenebene `table_bytes` mit den tatsächlichen Bytes des Quellobjekts. Ein Parquet-Wert auf Spaltenebene vergleicht die unkomprimierten und komprimierten Bytes des Spalten-Chunks im Footer. Beide sind Planungssignale für die Dateispeicherung und keine Schätzungen basierend auf dekodierten Samples. `ratio_zstd_3` und `ratio_zstd_19` sind nur dann vergleichbar, wenn `sample_encoding` der erkannte `"blueprint-compression-probe-v2"`-Wert ist. Ein Parquet-Footer-Verhältnis oder ein Avro-Container-Verhältnis darf niemals in diese zstd-Felder kopiert werden.
