# Was dbwarp-blueprint liest und schreibt

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../AUDIT.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../AUDIT.md) | **Deutsch** | [Français](../fr/AUDIT.md) | [Español](../es/AUDIT.md) | [Polski](../pl/AUDIT.md) | [日本語](../ja/AUDIT.md) | [中文](../zh/AUDIT.md)

Dieses Dokument beschreibt das Netzwerk-, Dateisystem-, Umgebungs-, Datenbank- und Auditverhalten der Anwendung zur Laufzeit. Gleichen Sie jeden aktiven Modus und jede Option mit Ihrer Sicherheitsrichtlinie ab. Datenbanktreiber, TLS- und Identitätsbibliotheken, der dynamische Loader und das Betriebssystem können zusätzlich Plattformkonfiguration, Trust-Stores, DNS, Anmeldedatencaches und netzwerkgebundene Speicher konsultieren; diese Aktionen der Unterstützungsschicht sind im Anwendungsaudit nicht vollständig sichtbar.

## Ausgehender Netzwerkverkehr

Der Live-Modus `--connect` für PostgreSQL, MySQL und SQL Server öffnet eine Datenbanktreibersitzung zum angegebenen Endpunkt. Der Batch-Modus verarbeitet seine Quellen sequenziell und öffnet eine Sitzung für jede Datenbankquelle. Die DNS-Auflösung kann den konfigurierten Resolver verwenden, und die integrierte Kerberos-/SSPI-Authentifizierung kann einen KDC oder Domänencontroller kontaktieren. Offline-Operationen mit TOML, Parquet, Avro und Bundles öffnen keine von der Anwendung initiierte Netzwerkverbindung; ein Pfad auf einem Netzwerk-Dateisystem unterliegt jedoch weiterhin dem Speicher-Stack des Hosts.

Die bestätigungspflichtige Oracle-Vorschau startet stattdessen nur die mit `--oracle-sqlplus` ausdrücklich angegebene SQL*Plus-Programmdatei (einschließlich einer begrenzten `-V`-Abfrage, sofern verfügbar) und verwendet sie für die Katalogsitzung. Der Kindprozess erhält ein leeres privates Verzeichnis als `TNS_ADMIN`; seine Anmeldeinformationen werden in die Standardeingabe geschrieben und niemals in Prozessargumenten abgelegt. Die Umgebung wird vor dem Start geleert. Nur `PATH`, `SystemRoot`, `WINDIR`, `ORACLE_HOME`, `LD_LIBRARY_PATH`, `DYLD_LIBRARY_PATH`, `LIBPATH` und `SHLIB_PATH` werden weitergereicht, wenn sie vorhanden sind; der Collector setzt seine feste Locale, Zeitzone und privaten `TNS_ADMIN`-Werte separat.

Die Binärdatei besitzt keinen Telemetrie-, Lizenzprüfungs-, Versionsaktualisierungs-, Cloud-API- oder Uploadpfad.

Sie können dies je nach Plattform mit `strace -f -e trace=connect,sendto,recvfrom`, `tcpdump` oder eBPF überprüfen.

## Dateisystem-Lesezugriffe

Das Werkzeug liest die vom aktiven Modus ausgewählten Eingaben:

| Datei | Wann | Inhalt |
|---|---|---|
| `--user-file PATH` | Falls angegeben | Nur Benutzername. Nachfolgender Leerraum wird entfernt; eine leere Datei ist ein Fehler. |
| `--password-file PATH` | Falls angegeben | Wird einmal gelesen. Der DBWarp-eigene `Secret`-Puffer wird beim Verwerfen genullt; Datenbanktreiber können eigene Kopien wie in SECURITY.md dokumentiert behalten. Wird unter Unix bei Lesezugriff für Gruppe/Andere abgelehnt. |
| `--anonymization-key-file PATH` | Falls angegeben | Ein HMAC-Schlüssel mit 32 Bytes oder 64 Hexadezimalzeichen, den Sie verwahren. Wird unter Unix bei Lesezugriff für Gruppe/Andere abgelehnt. Der Schlüssel wird nie ausgegeben. |
| `--azure-token-file PATH` | Falls angegeben | SQL-Server-Entra-ID-Token. Wird einmal gelesen; der DBWarp-eigene `Secret`-Puffer wird beim Verwerfen genullt. Wird unter Unix bei Lesezugriff für Gruppe/Andere abgelehnt. |
| `--tls-ca PATH` | Falls angegeben | Vertrauenswürdiges CA-PEM, das beim Verbindungsaufbau gelesen wird. PostgreSQL/MySQL akzeptieren ein Bundle; SQL Server akzeptiert genau ein Zertifikat. Die bereitgestellte Datei ersetzt die Standard-Stammzertifikate der Engine. |
| `--tls-cert PATH` | Falls angegeben | PostgreSQL-/MySQL-TLS-Clientzertifikat (PEM), das beim Verbindungsaufbau gelesen wird. Wird bei SQL Server mit `DBP1015E` abgelehnt. |
| `--tls-key PATH` | Falls angegeben | PostgreSQL-/MySQL-TLS-Clientschlüssel (PEM). Wird unter Unix bei Lesezugriff für Gruppe/Andere abgelehnt. Wird beim Verbindungsaufbau gelesen und bei SQL Server mit `DBP1015E` abgelehnt. |
| `--from-toml PATH` | Falls angegeben | Vorhandene dbwarp-blueprint-TOML-Datei, die lokal gelesen wird, um ohne Datenbankverbindung eine Präsentation zu erstellen. |
| `--from-parquet PATH` | Falls angegeben | Parquet-Metadaten und, nur bei ausdrücklicher Zustimmung zur Stichprobe, begrenzte dekodierte Zeilen. |
| `--from-avro PATH` | Falls angegeben | Metadaten und Datensätze des Avro-Containers; zum Ermitteln der Zeilenzahl wird der Container durchlaufen. |
| `--batch-manifest PATH` | Falls angegeben | Manifest sowie alle darin referenzierten lokalen Eingabe-, Anmeldedaten-, Token- und TLS-Pfade. |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Falls angegeben | Bundle-TOML und relative Blueprint-Dateien, die zum Auflisten, Extrahieren oder Packen benötigt werden. |
| steuerndes Terminal/Konsole (`/dev/tty` auf Unix-ähnlichen Systemen) | Wenn keine Passwortquelle angegeben ist | Eingabeaufforderung mit deaktivierter Anzeige. |
| (nur zur Build-Zeit) `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.dbwarp-source-revision` in vendorten Releases, `vendor/*`, `vendor-crates/*` in Offline-Bundles | Nur bei Ausführung von `./build.sh` | Toolchain-, Quellprovenienz- und übliche Cargo-Build-Eingaben |

Die Anwendung besitzt keinen ausdrücklichen Pfad zum Lesen von:
- `~/.pgpass`, `~/.my.cnf`, `~/.aws/credentials`, `~/.azure/credentials`
- beliebige Dateien unter `~/.ssh/*`
- `/etc/passwd` oder `/etc/shadow` als Blueprint-Eingaben (Plattformbibliotheken für Identität und Authentifizierung können weiterhin Betriebssystemkontodaten konsultieren)
- beliebige Datenbank-Anmeldevariablen außer der jeweils mit `--password-env`, `--user-env` oder `--azure-token-env` benannten. Builds mit integriertem Kerberos können außerdem bewirken, dass der GSSAPI-/Kerberos-Stack der Plattform eigene Konfiguration, Cache, Keytab und Umgebungswerte konsultiert. Locale- und Terminalvariablen sind unten beschrieben.

## Dateisystem-Schreibzugriffe

Das Werkzeug schreibt nur die vom aktiven Modus ausgewählten Ausgaben:

| Datei | Wann | Inhalt |
|---|---|---|
| `--out PATH` (Standard `./blueprint.toml`) | Live-Datenbank-, Parquet-, Avro-, Bundle-Extraktions- und Bundle-Pack-Läufe | Blueprint- oder gepacktes Bundle-TOML. Wird in reinen Präsentations-, Bundle-Listen-, Trockenlauf-, Hilfe- oder Versionsmodi nicht geschrieben. |
| `--deck PATH` | Nur falls angegeben | Eine PowerPoint-Präsentation (.pptx), die den anonymisierten Blueprint zusammenfasst. Sie wird lokal aus demselben im Speicher befindlichen Blueprint oder der Eingabe `--from-toml` erstellt — kein zusätzlicher Datenbankzugriff, kein Netzwerk, keine Drittanbieterbibliothek. |
| `--audit-log PATH` | Nur falls angegeben | Eine atomar ersetzte Kopie des auf stderr ausgegebenen Auditprotokolls; vorhandener Inhalt wird nicht angehängt. |
| `--out-dir DIR` | Batch-Modus ohne Trockenlauf | `bundle.toml`, `blueprints/` und `audits/` pro Quelle, eine Eigentumsmarkierung und nach einem Teilfehler `errors.txt`. Die Veröffentlichung verwendet ein benachbartes Staging-Verzeichnis und eine Wiederherstellungsmarkierung. |
| (nur zur Build-Zeit) `./target/`, `./build/` | Nur bei Ausführung von `./build.sh` | Übliche Cargo-Build-Ausgaben |

Die Anwendung besitzt keinen ausdrücklichen Pfad zum Schreiben nach:
- `/var/log/*`
- `~/.cache/*`, `~/.local/*`, `~/.config/*`
- kein implizites temporäres Systemverzeichnis (der Benutzer kann eine Ausgabe oder ein Batch-Verzeichnis weiterhin ausdrücklich dorthin verweisen)

## Gelesene Umgebungsvariablen

Das Audit listet nur von DBWarp Blueprint selbst abgefragte Variablen. Wenn `--lang` keine
unterstützte Sprache festlegt, kann die Sprachauswahl `DBWARP_BLUEPRINT_LANG`, `LC_ALL`,
`LC_MESSAGES` und `LANG` in dieser Reihenfolge lesen. Die Terminaldarstellung kann `NO_COLOR`, `TERM`,
`COLORTERM` und `COLUMNS` lesen; diese beeinflussen nur die Darstellung.

Wenn `--password-env VAR_NAME` oder `--user-env VAR_NAME` angegeben ist, liest das Werkzeug genau diese benannte Variable. Es gibt keinen Fallback auf übliche Standardwerte wie `PGPASSWORD`, `MYSQL_PWD`, `MSSQL_PASSWORD`, `USER` oder `LOGNAME` — solche Fallbacks sind bewusst nicht implementiert.

Plattformbibliotheken für Datenbank, TLS, DNS und integrierte Authentifizierung können eigene Variablen und Konfiguration außerhalb dieser Anwendungsliste konsultieren. Verwenden Sie eine Betriebssystemablaufverfolgung, wenn die Richtlinie ein vollständiges Prozess- und Bibliotheksinventar verlangt.

Bei der Ausführung von `./build.sh` liest das Skript `PINNED_RUST` (Überschreibung), `ALLOW_NETWORK` (Opt-in für den Download von rustup-init), `TARGET` (Cross-Compile-Ziel) sowie die üblichen Cargo-/rustup-Variablen. Das Werkzeug selbst liest keine davon zur Laufzeit.

## Auditprotokoll pro Lauf

Das Werkzeug gibt bei jedem Lauf ein Auditprotokoll auf stderr mit stabilem Klartextlayout aus. Leiten Sie es mit `2>audit.txt` in eine Datei um oder verwenden Sie `--audit-log PATH` für eine explizite Kopie.

Beispiel (Tier 1):

```
=== dbwarp-blueprint audit ===
build_source_revision: 0123456789abcdef0123456789abcdef01234567
build_source_dirty:    false
build_toolchain:     1.94.0 (vendored)
mode:                tier-1
started_at_unix_ms:  1745596800000
outcome:             ok
anonymization_key:   ephemeral-random
schema_selector_count: 1

connection:
  - postgresql://app@db.example:5432/payments
    auth: scram-sha-256-or-md5
    tls: yes (protocol version unavailable from driver)
    tls_ca_only: false

auth:
  user_source:        file:/etc/dbwarp/db.user
  password_source:    file:/etc/dbwarp/db.pass (mode 0o600)
  password_persisted: false
  password_logged:    false
  authenticated_principal: (not observed)
  effective_server_principal: (not observed)
  database_principal: (not observed)
  expected_server_principal: (not requested)
  principal_assertion: not-observed

topology_and_scope:
  topology:
    deployment: unknown
    local_role: unknown
    visibility: partial
    member_count: 2
    identifiers_redacted: true
    role_counts: primary=1, secondary=1
    features: postgresql-streaming-replication
    catalogs_read: pg-is-in-recovery, pg-stat-replication
    catalogs_unreadable: (none)
  dataset_scope:
    layout: full-copy
    table_inventory_completeness: complete
    row_count_completeness: complete
    size_completeness: complete
    row_count_method: postgres-planner-estimate
    size_method: postgres-local-relation-size
    limitations: row-counts-statistical

blueprint_fidelity_estimate:
  basis: evidence-coverage-v1
  overall_score: 79/100
  band: good
  structure_score: 90/100
  sizing_score: 100/100
  column_statistics_score: 68/100
  relationship_score: 75/100
  artifact_score: 50/100
  limitations: biased-column-sampling, cardinality-lower-bounds
  qualification: evidence estimate, not source-truth accuracy or a confidence interval

artifact_inventory:
  detail: summary
  visibility: full
  objects: 42
  dependency_edges: 0
  external_prerequisites: 3
  inventory_complete: false
  dependencies_complete: false
  requirements_complete: false
  analysis_complete: false

database_operations_observed:
  1. [succeeded, 14ms, 28 rows]   server version lookup
  2. [succeeded, 9ms, 312 rows]   column catalog lookup
  ... (every observed catalog operation enumerated)

wire_bytes_observed:
  catalog_responses: unknown (driver does not expose wire-byte totals)
  row_data:          unknown (driver does not expose wire-byte totals)

local_sample_processing:
  encoded_rowframe_bytes: 0 B

sampling_work:
  compression_workers: 0
  compression_queue_capacity: 0
  compression_jobs_submitted: 0
  compression_jobs_completed: 0
  compression_pipeline_wall_ms: 0
  compression_worker_ms: 0
  tables_skipped_proven_empty: 0
  chunk_level_3_attempts: 0
  table_level_3_attempts: 0
  column_level_3_attempts: 0

files_read_local:
  - /etc/dbwarp/db.pass        (mode 0o600 ✓)

files_written_local:
  - ./blueprint.toml         (12 KiB, sha256: 7f3e2af1...)

warnings:
  - (none)

network_egress:
  - db.example:5432 (database-driver session; DNS may use the configured resolver)

env_vars_read:
  - (none)

trust_assertions:
  - no row content was read
  - no telemetry was sent anywhere
  - length policy balanced: declared capacities and index prefixes exact; sampled lengths relatively rounded
  - identifier ordering uses domain-separated HMAC-SHA256 with a fresh process-local key; labels intentionally vary between runs
  - the anonymization key and source identifiers are not written to the Blueprint
  - artifact summary stores bounded counts and external-prerequisite classes; no object identities or definitions
  - artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries
  - credential entered through the Secret wrapper and its buffer is zeroized on drop; driver APIs may retain copies as documented under 'Driver-owned credential copies' in SECURITY.md

run_duration_ms:    142
finished_at_unix_ms: 1745596800142
=== end audit ===
```

MySQL-Läufe geben eine modusspezifische Aussage `length policy balanced|strict|exact` aus. Sie gibt unabhängig an, ob strukturelle und aus Stichproben ermittelte Längen exakt oder gerundet sind, sodass das Audit bei einem balanced- oder exact-Lauf niemals behauptet, alle numerischen Werte seien gerundet.

Das Auditprotokoll:

- zeichnet nur die Anzahl der wiederholbaren Live-Selektoren `--schema` auf; ihre Werte werden in der interaktiven Vorabansicht angezeigt, aber nicht in das Audit aufgenommen. Der bestehende Verbindungs-URI, aus dem sensible Angaben entfernt wurden, identifiziert weiterhin die verbundene Datenbank, die bei MySQL zugleich der Schemaname ist. Eine ausgewählte Blueprint ist in `dataset_scope` als `selection-limited` markiert;
- nennt die beim Kompilieren eingebettete Quellrevision und den Zustand des Arbeitsbaums; der endgültige Binär-SHA-256 bleibt ein externer Release-/Registry-Prüfwert, da eine Binärdatei ihren eigenen endgültigen Hash nicht einbetten kann;
- zeichnet die **Quelle** der Anmeldedaten auf (Dateipfad, Name der Umgebungsvariable, TTY), niemals deren Wert;
- zeichnet bei SQL Server die exakten Sitzungsidentitäten aus
  `ORIGINAL_LOGIN()`, `SUSER_SNAME()` und `USER_NAME()` auf. Wenn
  `--expect-server-principal` angegeben ist, werden auch der erwartete Wert und
  das Ergebnis des serverseitigen Vergleichs vor der Katalogerfassung erfasst;
- listet jede beobachtete Datenbankoperation mit Ergebnis, Laufzeit und, sofern vom Treiber geliefert, Zeilenzahl auf; fehlgeschlagene Endoperationen erhalten eine begrenzte kennungsfreie Bezeichnung;
- weist Datenbank-Wire-Bytes als `unknown` aus, sofern der Treiber sie nicht bereitstellt, und meldet lokal codierte Stichprobenbytes separat;
- meldet die Gesamtzahl der lokal geschriebenen Bytes (mit sha256 jeder Datei);
- zeichnet nicht schwerwiegende Verschlechterungen der Erfassung und Stichproben mit stabilen DBP-Warncodes auf; ein leerer Abschnitt bedeutet, dass keine bekannte Verschlechterung beobachtet wurde;
- kopiert validierte Nachweise aus `[database_topology]` und `[dataset_scope]` nach `topology_and_scope`, ausschließlich mit geschlossenen Token und Anzahlen; Knotennamen, Endpunkte, Cluster- und Datenbankkennungen können nicht erscheinen;
- bewahrt `DBP1411W`, `DBP1412W` und `DBP1413W` bei unvollständiger Topologie- oder Datensatzabdeckung, sodass eine erfolgreiche Erfassung keinen Größenhinweis verbergen kann;
- zeichnet eine deterministische, nach Dimensionen aufgeschlüsselte Schätzung der Blueprint-Fidelity auf. Der Wert beschreibt die Abdeckung der erfassten Evidenz für Struktur, Größenbestimmung, Spaltenstatistiken, Beziehungen und Artefakte. Er ist weder ein gemessener Fehler gegenüber den Quelldaten noch ein statistisches Konfidenzintervall. Als veraltet oder nie analysiert gemeldete PostgreSQL-Statistiken senken den Wert der Größenbestimmungsdimension und erscheinen ausdrücklich als Einschränkungen `table-statistics-stale` oder `table-statistics-never-analyzed`. Aktualität kann eine fehlende Abdeckung der Zeilenzahlen nicht ausgleichen. Bei Engines, die laufend aktualisierte Zähler verwenden oder deren Statistikaktualität nicht festgestellt werden kann, wird keine Evidenz zur Aktualität erfunden;
- erklärt dem Modus entsprechende Vertrauensaussagen (Tier 1 bzw. Tier 2);
- verwendet ein stabiles Textformat, doch Werte können sich mit Datenbankzustand, Zeitablauf, Warnungen und dem standardmäßig neuen Anonymisierungsschlüssel ändern. Verwenden Sie für genehmigte Vergleiche denselben geschützten `--anonymization-key-file` und schreiben Sie `--generated-at` fest; Zeitfelder variieren weiterhin.

**Bedingte Ausgabe der Vertrauensaussage.** Die Zeile „credential entered through the Secret wrapper...“ wird nur bei Läufen ausgegeben, in denen tatsächlich Anmeldedaten gelesen wurden. Fehlerpfade, die vor der Erfassung von Anmeldedaten abbrechen (URI-Parsingfehler, Ablehnung von in URIs eingebetteten Passwörtern, Probelauf usw.), geben diese Zeile bewusst *nicht* aus — über Anmeldedaten, die nie abgerufen wurden, kann keine Aussage getroffen werden. Verwenden Sie das Vorhandensein/Fehlen der Zeile zusammen mit `auth.password_source`, um festzustellen, ob die Verarbeitung von Anmeldedaten in einem bestimmten Lauf ausgeübt wurde.

Der Audit-Prozess wird bei erfolgreichen und fehlgeschlagenen Abläufen ausgeführt, einschließlich Fehlern bei der Parsierung von Befehlszeilen nach dem Start. Wenn das Tool während der Ausführung fehlschlägt, wird das Audit-Protokoll weiterhin auf stderr und auf `--audit-log PATH` ausgegeben, falls angegeben, wobei `outcome: error: <stage>` verwendet wird, damit Sie einen Nachweis darüber haben, was versucht wurde, bevor der Fehler auftrat.

Beispiel für eine Fehlerzeile:

```
outcome:             error: parsing --connect URI (value redacted to avoid logging embedded credentials)
```

Die Terminalausgabe enthält außerdem eine codierte Bedienerzusammenfassung wie `DBP1001E` oder `DBP0001E` mit der Ursachenkette. Das Auditergebnis ist begrenzt und kann langen Text abschneiden; verwenden Sie zur Support-Triage die Terminalausgabe zusammen mit dem Meldungscode. Siehe `docs/MESSAGES.md`.

Optionale RTT-, Komprimierungs- und Textstilprüfungen können fehlschlagen, ohne die primäre Katalogerfassung ungültig zu machen. Solche Fälle werden als `DBP1405W` bis `DBP1408W` ausgegeben und unter `warnings:` beibehalten, sodass ein erfolgreiches, aber partielles Tier-2-Ergebnis von einem vollständigen Ergebnis unterschieden werden kann. Wiederholte identische Warnungen werden dedupliziert und mehrzeilige Treiberdetails abgeflacht, damit das Audit begrenzt und maschinenlesbar bleibt.

## Lesezugriffe auf Nicht-Tabellenartefakte

Die Artefakterfassung ist von Tier-2-Zeilenstichproben unabhängig:

- `--artifact-detail none` überspringt Artefaktinventarkataloge und Definitionen;
  die reine Zählprüfung der Topologie wird dennoch ausgeführt.
- `summary` liest modellierte Objektkataloge, aber keinen Definitionstext.
- `graph` liest zusätzlich Abhängigkeitskataloge, aber keinen Definitionstext.
- `analyzed` liest verfügbare SQL-/Prozedurdefinitionen zusätzlich zur lexikalischen Analyse in begrenzten Prozessspeicher.

Das Audit zeichnet Detailgrad, Sichtbarkeit, Objekt-/Abhängigkeits-/Extern-Anzahlen und alle Vollständigkeitsflags auf. Jede Artefaktkatalogoperation erscheint in `database_operations_observed`. Ein fehlgeschlagener optionaler Katalog gibt `DBP1410W` aus, erscheint unter `warnings` und verhindert eine falsche Vollständigkeitsbehauptung.

Im Analysemodus werden Definitionen in einen Nullwert-Eigentümer eingeschlossen, bereinigt und auf begrenzte Bereiche und geschlossene Feature-Token reduziert. Definitionstexte, Namen von Quellobjekten, externe Endpunkte, Artefakt-Prinzipale, Zugangsdaten, Schlüssel-/Zertifikatmaterial, Paket-/Bibliotheksnamen und Binärdateien werden niemals in die Blueprint oder das Auditprotokoll geschrieben. Die einzigen exakten Prinzipalnamen, die beibehalten werden, sind die drei SQL Server-Sitzungsidentitäten im expliziten `auth`-Auditblock; diese werden niemals in die Blueprint, Präsentations- oder Bundle-Dateien geschrieben. Die Graph- und Analysemodi erfordern `--yes`, da eine anonyme Topologie dennoch eine Anwendung identifizieren kann.

- summary: nur begrenzte Anzahlen, keine Objektidentitäten oder Definitionen;
- graph: anonymer Abhängigkeitsgraph, keine Definitionen;
- analyzed: Definitionen vorübergehend gelesen, nur begrenzte Feature-Bänder aufbewahrt.

Siehe [`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) für Objektfamilienabdeckung und Vollständigkeitsinterpretation.

## Ergänzungen in Tier 2

Wenn die Kompressionsmessung interaktiv oder nichtinteraktiv mit `--measure-compression --yes` bestätigt wurde, führt das Werkzeug zusätzlich Folgendes aus:

- Für jede nicht nachweislich leere Tabelle wird ein Engine-spezifischer,
  begrenzter Stichprobenpfad ausgeführt. PostgreSQL beginnt mit einem adaptiven
  Prozentsatz für `TABLESAMPLE SYSTEM`, abgeleitet aus der geschätzten Zeilenzahl
  und der angeforderten Stichprobengröße, zusammen mit `LIMIT N`, und fällt
  bei Bedarf auf `LIMIT N` zurück;
  MySQL verwendet vier begrenzte Bereichsfenster für numerische Primärschlüssel,
  wenn dieser sichere Zugriffspfad verfügbar ist, andernfalls `LIMIT N`; SQL
  Server verwendet `TOP N`. Verzerrte Pfade setzen in der Ausgabe
  `sampled_with_bias = true`.
- Einlesen der Stichprobenzeilen in einen lokalen In-Memory-Puffer.
- Die Datenbankzugriffe bleiben sequenziell. Mit `--compression-workers N`
  können 1–32 begrenzte lokale Komprimierungs-Worker ausgeführt werden
  (Standardwert 1 zur Minimierung der Auswirkungen auf den Quellhost). Erhöhen
  Sie ihn ausdrücklich, um mehr lokale CPU zu verwenden. Jeder Worker besitzt
  eigene zstd-Kontexte; es gibt keine gemeinsam genutzte zstd-Sperre.
- Komprimierung mit zstd auf Stufe 3.
- Aufzeichnung der resultierenden Verhältnisse und Standardabweichung.
- **Verwerfen jedes Puffers nach Abschluss seines begrenzten lokalen
  Komprimierungsjobs**. Die Bytes werden weder auf die Festplatte geschrieben
  noch übertragen. Der Worker-Pool hält höchstens N wartende und N aktiv
  komprimierte Stichproben.

`local_sample_processing.encoded_rowframe_bytes` zeigt die lokal für die
Kompression codierten Bytes, nicht Datenbank-Wire-Bytes. Nicht vom Treiber
bereitgestellte Wire-Bytes bleiben `unknown`. Der `[compression]`-Block enthält
die Verhältniswerte. `--max-wall-secs` ist eine harte Frist für die gesamte
Live-Erfassung einschließlich Verbindung, Katalogen, RTT und Tier 2.
PostgreSQL setzt außerdem das sitzungslokale `statement_timeout`, MySQL das
sitzungslokale `max_execution_time` für schreibgeschützte `SELECT`-Anweisungen
und SQL Server das sitzungslokale `LOCK_TIMEOUT`, weil dort keine gleichwertige
sitzungsweite Begrenzung der Anweisungslaufzeit existiert. Beim Ablauf der
äußeren Frist trennt der Client die Verbindung. Das Audit wertet diese Trennung
nicht als Beleg dafür, dass SQL Server einen Abbruch bestätigt hat; ein
Bediener muss vor einem erneuten Versuch prüfen, ob die Serverarbeit beendet
ist.

`sampling_work` ist kennungsfreie Betriebsevidenz. Der Abschnitt erfasst die
lokalen Worker- und Warteschlangengrenzen, die Obergrenze von 16 MiB für die
projizierte Nutzlast je Tabelle, eingereichte und abgeschlossene Jobs,
Komprimierungsversuche und Tabellen, deren Stichprobe ausgelassen wurde,
weil der Engine-Katalog sie zum Zeitpunkt des Kataloglesens nachweislich als
leer auswies. `compression_worker_ms` ist die aggregierte Worker-Wandzeit,
nicht die Prozess-CPU-Zeit, und kann bei überlappenden Workern größer als
`compression_pipeline_wall_ms` sein. Die Pipeline-Wandzeit kann sich mit den
weiterhin sequenziellen Datenbankzugriffen überlappen. Diese Zähler beschreiben
ausgeführte Arbeit; sie sind keine Datenbankzeilenzahlen, Wire-Byte-Messungen
oder Aussagen zur Quellgenauigkeit.

## Verifizierungsprotokoll

Ein heruntergeladenes Plattformarchiv und ein reproduzierter Build aus dem
Quellcode sind zwei unterschiedliche Vertrauensprüfungen. Prüfen Sie zuerst das
Archiv und die ausführbare Datei anhand der Prüfsumme, die mit demselben Release
veröffentlicht wurde. Das Plattformarchiv ist ein Betreiberpaket und kein
Quellcodebaum: Die enthaltene Dokumentation und `verify.sh` dienen als Referenz
für einen passenden Quellcode-Checkout. Beziehen Sie das exakte Release-Tag aus
dem öffentlichen Repository oder verwenden Sie das Quellcodearchiv mit
gebündelten Abhängigkeiten, bevor Sie ein Quellcode-Audit oder einen Neuaufbau
versuchen. Ein Plattformarchiv kann nicht an Ort und Stelle neu gebaut werden.

Wenn Sie *nachweisen* möchten, dass das Werkzeug nur die dokumentierten Aktionen ausführt:

1. **Download-Integrität**: Prüfen Sie das Plattformarchiv anhand des passenden
   Eintrags in `SHA256SUMS.txt` und anschließend die entpackte ausführbare Datei
   anhand ihrer Datei `*.binary.sha256`. Beide Prüfsummendateien müssen vom
   selben unveränderlichen Release-Tag stammen. Siehe
   [Binärdateien herunterladen](BINARIES.md).
2. **Quellcode-Audit**: Lesen Sie im passenden Quellcode-Checkout oder im
   Quellcodearchiv mit gebündelten Abhängigkeiten `src/secret.rs` und suchen Sie
   anschließend außerhalb dieser Datei nach `\.expose\(\)`. Wenn `rg` installiert
   ist, lautet der kurze Befehl:
   ```
   $ rg -n '\.expose\(\)' src --glob '!secret.rs'
   ```
   Verwenden Sie andernfalls das für Ihre Plattform zugelassene rekursive
   Textsuchwerkzeug; `rg` ist keine Build- oder Verifizierungsvoraussetzung.
   Die Produktionsaufrufstellen übergeben den offengelegten `&str` unmittelbar
   an den Verbindungs-Builder eines Treibers. MySQL ruft zusätzlich
   `.to_string()` auf, weil die API von `mysql_async` einen `String` verlangt.
   Diese nicht mit Nullen überschriebene Kopie geht in treibereigene Optionen
   über und kann den `OptsBuilder` überdauern: Sie bleibt für die Lebensdauer
   der Optionen/Verbindung erhalten. Das Verwerfen des Builders beweist keine
   Löschung. Tier 1 und Tier 2 verwenden dieselbe
   MySQL-Verbindung. Die vollständige Erläuterung finden Sie unter
   **Treibereigene Kopien von Anmeldedaten** in SECURITY.md.
3. **Aus Quellcode erstellen**: `./build.sh`. Bei Verwendung des Quellcodearchivs
Führen Sie `DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh` aus. Jede Version wird zweimal erstellt, und eine Byte-Differenz führt zum Fehlschlagen der Version. Ein lokaler Vergleich ist nur mit derselben Quellversion, dem gleichen Ziel, den gleichen Funktionen, der gleichen fixierten Rust-Toolchain, dem gleichen Linker und den gleichen Build-Optionen aussagekräftig.
4. **Mit dem Release vergleichen**: Führen Sie im passenden Quellcode-Checkout
   oder Quellcodebaum `./verify.sh /path/to/extracted/dbwarp-blueprint` aus.
   Die erforderlichen Ziel-, Feature-, Toolchain-, Linker-, Source-Date-Epoch-
   und Build-Flag-Werte sind unter **Eine Release-Binärdatei reproduzieren** in
   BUILD.md beschrieben.
5. **Laufzeit-Trace**: Führen Sie das Werkzeug unter Linux in einer Sandbox mit
   `strace -f -e trace=open,connect,read,write` aus. Wenn `strace` oder `rg` nicht
   verfügbar ist, verwenden Sie das entsprechende Datei-/Netzwerk-Trace- und
   rekursive Textsuchwerkzeug Ihrer Plattform. Gleichen Sie die Ausgabe mit den
   obigen Listen ab.
6. **Netzwerk-Trace**: Führen Sie `tcpdump` auf dem Host aus. Verifizieren Sie
   bei einem passwortauthentifizierten Live-Lauf die Datenbanksitzung sowie den
   erwarteten DNS-Verkehr. Berücksichtigen Sie bei integrierter Authentifizierung
   außerdem den erwarteten Verkehr zum KDC beziehungsweise Domänencontroller.
   Gleichen Sie im Batch-Modus eine Datenbanksitzung pro Datenbankquelle ab.

Wenn eine dieser Angaben nicht mit dem hier dokumentierten übereinstimmt, melden Sie die Diskrepanz über den in SECURITY.md angegebenen Kanal und fügen Sie die kleinste, sichere Nachverfolgung hinzu, die erforderlich ist, um sie zu reproduzieren. Geben Sie keine Zugangsdaten, identifizierende Namen oder sensible Treiber-Ausgaben in einem öffentlichen Problem an.
