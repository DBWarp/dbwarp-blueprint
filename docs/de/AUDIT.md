# Was dbwarp-blueprint liest und schreibt

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../AUDIT.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../AUDIT.md) | **Deutsch** | [Français](../fr/AUDIT.md) | [Español](../es/AUDIT.md) | [Polski](../pl/AUDIT.md) | [日本語](../ja/AUDIT.md) | [中文](../zh/AUDIT.md)

Dieses Dokument führt jede Aktion auf, die das Werkzeug ausführen kann. Gleichen Sie es mit Ihrer Sicherheitsrichtlinie ab.

## Ausgehender Netzwerkverkehr

Der Live-Modus `--connect` öffnet eine Datenbanktreibersitzung zum angegebenen Endpunkt. Der konfigurierte Resolver kann für die DNS-Auflösung verwendet werden, und die integrierte Kerberos-/SSPI-Authentifizierung kann einen KDC oder Domänencontroller kontaktieren. Der Batch-Modus verarbeitet seine Quellen sequenziell und öffnet eine Sitzung für jede Datenbankquelle. Offline-Operationen mit TOML, Parquet, Avro und Bundles öffnen keine von der Anwendung initiierte Netzwerkverbindung; ein Pfad auf einem Netzwerk-Dateisystem unterliegt jedoch weiterhin dem Speicher-Stack des Hosts.

Die Binärdatei besitzt keinen Telemetrie-, Lizenzprüfungs-, Versionsaktualisierungs-, Cloud-API- oder Uploadpfad.

Sie können dies je nach Plattform mit `strace -f -e trace=connect,sendto,recvfrom`, `tcpdump` oder eBPF überprüfen.

## Dateisystem-Lesezugriffe

Das Werkzeug liest die vom aktiven Modus ausgewählten Eingaben:

| Datei | Wann | Inhalt |
|---|---|---|
| `--user-file PATH` | Falls angegeben | Nur Benutzername. Nachfolgender Leerraum wird entfernt; eine leere Datei ist ein Fehler. |
| `--password-file PATH` | Falls angegeben | Wird einmal gelesen und nach der Verwendung mit Nullen überschrieben. Wird abgelehnt, wenn der Modus Lesezugriff für Gruppe/Alle erlaubt. |
| `--azure-token-file PATH` | Falls angegeben | SQL-Server-Entra-ID-Token. Wird einmal gelesen und nach der Verwendung mit Nullen überschrieben. Wird abgelehnt, wenn der Modus Lesezugriff für Gruppe/Alle erlaubt. |
| `--tls-ca PATH` | Falls angegeben | Vertrauenswürdiges CA-PEM, das beim Verbindungsaufbau gelesen wird. PostgreSQL/MySQL akzeptieren ein Bundle; SQL Server akzeptiert genau ein Zertifikat. Die bereitgestellte Datei ersetzt die Standard-Stammzertifikate der Engine. |
| `--tls-cert PATH` | Falls angegeben | PostgreSQL-/MySQL-TLS-Clientzertifikat (PEM), das beim Verbindungsaufbau gelesen wird. Wird bei SQL Server mit `DBP1015E` abgelehnt. |
| `--tls-key PATH` | Falls angegeben | PostgreSQL-/MySQL-TLS-Clientschlüssel (PEM). Wird abgelehnt, wenn der Modus Lesezugriff für Gruppe/Alle erlaubt. Wird beim Verbindungsaufbau gelesen und bei SQL Server mit `DBP1015E` abgelehnt. |
| `--from-toml PATH` | Falls angegeben | Vorhandene dbwarp-blueprint-TOML-Datei, die lokal gelesen wird, um ohne Datenbankverbindung eine Präsentation zu erstellen. |
| `--from-parquet PATH` | Falls angegeben | Parquet-Metadaten und, nur bei ausdrücklicher Zustimmung zur Stichprobe, begrenzte dekodierte Zeilen. |
| `--from-avro PATH` | Falls angegeben | Metadaten und Datensätze des Avro-Containers; zum Ermitteln der Zeilenzahl wird der Container durchlaufen. |
| `--batch-manifest PATH` | Falls angegeben | Manifest sowie alle darin referenzierten lokalen Eingabe-, Anmeldedaten-, Token- und TLS-Pfade. |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Falls angegeben | Bundle-TOML und relative Blueprint-Dateien, die zum Auflisten, Extrahieren oder Packen benötigt werden. |
| `/dev/tty` | Wenn keine Passwortquelle angegeben ist | Eingabeaufforderung mit deaktivierter Anzeige. |
| (nur zur Build-Zeit) `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.dbwarp-source-revision` in vendorten Releases, `vendor/mysql_async`, `vendor-crates/*` in Offline-Bundles | Nur bei Ausführung von `./build.sh` | Toolchain-, Quellprovenienz- und übliche Cargo-Build-Eingaben |

Was es **NICHT** liest:
- `~/.pgpass`, `~/.my.cnf`, `~/.aws/credentials`, `~/.azure/credentials`
- beliebige Dateien unter `~/.ssh/*`
- `/etc/passwd`, `/etc/shadow`
- beliebige Datenbank-Anmeldevariablen außer der jeweils mit `--password-env`, `--user-env` oder `--azure-token-env` benannten. Builds mit integriertem Kerberos können außerdem `KRB5CCNAME` beobachten, weil libgssapi den Kerberos-Ticketcache verwendet. Locale- und Terminalvariablen sind unten beschrieben.

## Dateisystem-Schreibzugriffe

Das Werkzeug schreibt nur die vom aktiven Modus ausgewählten Ausgaben:

| Datei | Wann | Inhalt |
|---|---|---|
| `--out PATH` (Standard `./blueprint.toml`) | Live-Datenbank-, Parquet-, Avro-, Bundle-Extraktions- und Bundle-Pack-Läufe | Blueprint- oder gepacktes Bundle-TOML. Wird in reinen Präsentations-, Bundle-Listen-, Trockenlauf-, Hilfe- oder Versionsmodi nicht geschrieben. |
| `--deck PATH` | Nur falls angegeben | Eine PowerPoint-Präsentation (.pptx), die den anonymisierten Blueprint zusammenfasst. Sie wird lokal aus demselben im Speicher befindlichen Blueprint oder der Eingabe `--from-toml` erstellt — kein zusätzlicher Datenbankzugriff, kein Netzwerk, keine Drittanbieterbibliothek. |
| `--audit-log PATH` | Nur falls angegeben | Eine atomar ersetzte Kopie des auf stderr ausgegebenen Auditprotokolls; vorhandener Inhalt wird nicht angehängt. |
| `--out-dir DIR` | Batch-Modus ohne Trockenlauf | `bundle.toml`, `blueprints/` und `audits/` pro Quelle, eine Eigentumsmarkierung und nach einem Teilfehler `errors.txt`. Die Veröffentlichung verwendet ein benachbartes Staging-Verzeichnis und eine Wiederherstellungsmarkierung. |
| (nur zur Build-Zeit) `./target/`, `./build/` | Nur bei Ausführung von `./build.sh` | Übliche Cargo-Build-Ausgaben |

Was es **NICHT** schreibt:
- `/var/log/*`
- `~/.cache/*`, `~/.local/*`, `~/.config/*`
- kein implizites temporäres Systemverzeichnis (der Benutzer kann eine Ausgabe oder ein Batch-Verzeichnis weiterhin ausdrücklich dorthin verweisen)

## Gelesene Umgebungsvariablen

Das Audit listet nur tatsächlich abgefragte Variablen. Wenn `--lang` keine
unterstützte Sprache festlegt, kann die Sprachauswahl nacheinander `DBWARP_BLUEPRINT_LANG`, `LC_ALL`,
`LC_MESSAGES` und `LANG` lesen. Die Terminaldarstellung kann `NO_COLOR`, `TERM`,
`COLORTERM` und `COLUMNS` lesen; diese beeinflussen nur die Darstellung.

Wenn `--password-env VAR_NAME` oder `--user-env VAR_NAME` angegeben ist, liest das Werkzeug genau diese benannte Variable. Es gibt keinen Fallback auf übliche Standardwerte wie `PGPASSWORD`, `MYSQL_PWD`, `MSSQL_PASSWORD`, `USER` oder `LOGNAME` — solche Fallbacks sind bewusst nicht implementiert.

Bei der Ausführung von `./build.sh` liest das Skript `PINNED_RUST` (Überschreibung), `ALLOW_NETWORK` (Opt-in für den Download von rustup-init), `TARGET` (Cross-Compile-Ziel) sowie die üblichen Cargo-/rustup-Variablen. Das Werkzeug selbst liest keine davon zur Laufzeit.

## Auditprotokoll pro Lauf

Das Werkzeug gibt bei jedem Lauf ein Auditprotokoll auf stderr aus. Das Format ist deterministischer Klartext. Leiten Sie es mit `2>audit.txt` in eine Datei um oder verwenden Sie `--audit-log PATH` für eine explizite Kopie.

Beispiel (Tier 1):

```
=== dbwarp-blueprint audit ===
build_source_revision: 0123456789abcdef0123456789abcdef01234567
build_source_dirty:    false
build_toolchain:     1.94.0 (vendored)
mode:                tier-1
started_at_unix_ms:  1745596800000
outcome:             ok
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
  - db.example:5432 (the DB connection only)

env_vars_read:
  - (none)

trust_assertions:
  - no row content was read
  - no telemetry was sent anywhere
  - all numeric statistics rounded to documented precision
  - identifier ordering is deterministic (sha256-based)
  - no random or pseudorandom data in output
  - artifact summary stores bounded counts only; no object identities or definitions
  - artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries
  - credential read once via Secret wrapper, zeroized when dropped at end of engine run; see SECURITY.md for driver-owned copy lifetimes (MySQL clones to non-zeroizing String for the driver API)

run_duration_ms:    142
finished_at_unix_ms: 1745596800142
=== end audit ===
```

MySQL-Läufe geben eine modusspezifische Aussage `length policy balanced|strict|exact` aus. Sie gibt unabhängig an, ob strukturelle und aus Stichproben ermittelte Längen exakt oder gerundet sind, sodass das Audit bei einem balanced- oder exact-Lauf niemals behauptet, alle numerischen Werte seien gerundet.

Das Auditprotokoll:

- zeichnet nur die Anzahl der wiederholbaren Live-Selektoren `--schema` auf; ihre Werte werden in der interaktiven Vorabansicht angezeigt, aber nicht in das Audit aufgenommen. Der bestehende redigierte Verbindungs-URI identifiziert weiterhin die verbundene Datenbank, die bei MySQL zugleich der Schemaname ist. Eine ausgewählte Blueprint ist in `dataset_scope` als `selection-limited` markiert;
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
- zeichnet eine deterministische, nach Dimensionen aufgeschlüsselte Schätzung der Blueprint-Fidelity auf. Der Wert beschreibt die Abdeckung der erfassten Evidenz für Struktur, Größenbestimmung, Spaltenstatistiken, Beziehungen und Artefakte. Er ist weder ein gemessener Fehler gegenüber den Quelldaten noch ein statistisches Konfidenzintervall;
- erklärt dem Modus entsprechende Vertrauensaussagen (Tier 1 bzw. Tier 2);
- ist für dieselbe Eingabe deterministisch — gleiche DB, gleiche Argumente → gleiches Audit, abgesehen von den Zeitfeldern.

**Bedingte Ausgabe der Vertrauensaussage.** Die Zeile „credential read once via Secret wrapper...“ wird nur bei Läufen ausgegeben, in denen tatsächlich Anmeldedaten gelesen wurden. Fehlerpfade, die vor der Erfassung von Anmeldedaten abbrechen (URI-Parsingfehler, Ablehnung von in URIs eingebetteten Passwörtern, Probelauf usw.), geben diese Zeile bewusst *nicht* aus — über Anmeldedaten, die nie abgerufen wurden, kann keine Aussage getroffen werden. Verwenden Sie das Vorhandensein/Fehlen der Zeile zusammen mit `auth.password_source`, um festzustellen, ob die Verarbeitung von Anmeldedaten in einem bestimmten Lauf ausgeübt wurde.

**Das Audit wird bei operativen Erfolgs- und Fehlerpfaden ausgegeben**, einschließlich Befehlszeilen-Parsingfehlern nach dem Start. Hilfe-/Versionsausgaben und Fehler vor dem Laden des eingebetteten Lokalisierungsvertrags erzeugen kein vollständiges Audit. Bei einem späteren Fehler wird das Audit weiterhin auf stderr und gegebenenfalls nach `--audit-log PATH` geschrieben; das Ergebnis hat die Form `outcome: error: <stage>`.

```
outcome:             error: parsing --connect URI (value redacted to avoid logging embedded credentials)
```

Die Terminalausgabe enthält außerdem eine codierte Bedienerzusammenfassung wie `DBP1001E` oder `DBP0001E` mit der Ursachenkette. Das Auditergebnis ist begrenzt und kann langen Text abschneiden; verwenden Sie zur Support-Triage die Terminalausgabe zusammen mit dem Meldungscode. Siehe `docs/MESSAGES.md`.

Optionale RTT-, Komprimierungs- und Textstilprüfungen können fehlschlagen, ohne die primäre Katalogerfassung ungültig zu machen. Solche Fälle werden als `DBP1405W` bis `DBP1408W` ausgegeben und unter `warnings:` beibehalten, sodass ein erfolgreiches, aber partielles Tier-2-Ergebnis von einem vollständigen Ergebnis unterschieden werden kann. Wiederholte identische Warnungen werden dedupliziert und mehrzeilige Treiberdetails abgeflacht, damit das Audit begrenzt und maschinenlesbar bleibt.

## Lesezugriffe auf Nicht-Tabellenartefakte

Die Artefakterfassung ist von Tier-2-Zeilenstichproben unabhängig:

- `--artifact-detail none` überspringt Artefaktkataloge und Definitionen.
- `summary` liest modellierte Objektkataloge, aber keinen Definitionstext.
- `graph` liest zusätzlich Abhängigkeitskataloge, aber keinen Definitionstext.
- `analyzed` liest verfügbare SQL-/Prozedurdefinitionen zusätzlich zur lexikalischen Analyse in begrenzten Prozessspeicher.

Das Audit zeichnet Detailgrad, Sichtbarkeit, Objekt-/Abhängigkeits-/Extern-Anzahlen und alle Vollständigkeitsflags auf. Jede Artefaktkatalogoperation erscheint in `database_operations_observed`. Ein fehlgeschlagener optionaler Katalog gibt `DBP1410W` aus, erscheint unter `warnings` und verhindert eine falsche Vollständigkeitsbehauptung.

Im analysierten Modus werden Definitionen von einem zeroisierenden Besitzer gehalten, bereinigt und auf begrenzte Bänder und geschlossene Feature-Token reduziert. Definitionstext, Quellobjektnamen, externe Endpunkte, Artefakt-Principals, Anmeldedaten, Schlüssel-/Zertifikatsmaterial, Paket-/Bibliotheksnamen und Binärdateien werden niemals in Blueprint oder Auditprotokoll geschrieben. Die einzigen exakten Principal-Namen sind die drei oben im ausdrücklichen Auditblock `auth` beschriebenen SQL-Server-Sitzungsidentitäten; sie werden niemals in Blueprint, Präsentation oder Publikationsartefakte geschrieben. Die Modi graph und analyzed erfordern `--yes`, weil anonyme Topologie eine Anwendung identifizieren kann.

Das Audit unterscheidet die Datenschutzpositionen durch eine dieser Vertrauensaussagen:

- summary: nur begrenzte Anzahlen, keine Objektidentitäten oder Definitionen;
- graph: anonymer Abhängigkeitsgraph, keine Definitionen;
- analyzed: Definitionen vorübergehend gelesen, nur begrenzte Feature-Bänder aufbewahrt.

Siehe [`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) für Objektfamilienabdeckung und Vollständigkeitsinterpretation.

## Ergänzungen in Tier 2

Wenn die Kompressionsmessung interaktiv oder nichtinteraktiv mit `--measure-compression --yes` bestätigt wurde, führt das Werkzeug zusätzlich Folgendes aus:

- Für jede nicht nachweislich leere Tabelle wird ein Engine-spezifischer,
  begrenzter Stichprobenpfad ausgeführt. PostgreSQL beginnt mit
  `TABLESAMPLE SYSTEM(0.1) LIMIT N` und fällt bei Bedarf auf `LIMIT N` zurück;
  MySQL verwendet `LIMIT N`, SQL Server `TOP N`. Verzerrte Pfade setzen in der
  Ausgabe `sampled_with_bias = true`.
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

Wenn Sie *nachweisen* möchten, dass das Werkzeug nur die dokumentierten Aktionen ausführt:

1. **Quellcode-Audit**: Klonen Sie das Repository, lesen Sie `src/secret.rs` und suchen Sie anschließend außerhalb dieser Datei nach `\.expose\(\)`:
   ```
   $ rg -n '\.expose\(\)' src --glob '!secret.rs'
   ```
   Die Produktionsaufrufstellen übergeben den offengelegten `&str` unmittelbar an den Verbindungs-Builder. MySQL ruft zusätzlich `.to_string()` auf, weil `mysql_async` einen `String` verlangt; diese Kopie wird nicht mit Nullen überschrieben und bleibt bis zum Löschen des `OptsBuilder` bestehen. Tier 1 und Tier 2 verwenden dieselbe MySQL-Verbindung. Siehe SECURITY.md, §2.
2. **Aus Quellcode erstellen**: `./build.sh`. Die Release-CI führt auf demselben Runner einen unabhängigen Neuaufbau in einem separaten Cargo-Zielverzeichnis durch und weist Byte-Abweichungen zurück. Ein lokaler Vergleich ist nur mit derselben Quellrevision, demselben Ziel, denselben Features, derselben angehefteten Rust-Toolchain, demselben Linker und denselben Build-Flags aussagekräftig.
3. **Mit Release vergleichen**: `./verify.sh release/dbwarp-blueprint-X.Y.Z-...`
4. **Laufzeit-Trace**: Führen Sie das Werkzeug in einer Sandbox mit `strace -f -e trace=open,connect,read,write` aus. Gleichen Sie die Ausgabe mit den obigen Listen ab.
5. **Netzwerk-Trace**: Führen Sie `tcpdump` auf dem Host aus. Verifizieren Sie bei einem passwortauthentifizierten Live-Lauf die Datenbanksitzung sowie den erwarteten DNS-Verkehr. Berücksichtigen Sie bei integrierter Authentifizierung außerdem den erwarteten Verkehr zum KDC beziehungsweise Domänencontroller. Gleichen Sie im Batch-Modus eine Datenbanksitzung pro Datenbankquelle ab.

Wenn eine dieser Prüfungen nicht mit der Dokumentation übereinstimmt, erstellen Sie ein Issue mit Ihrem Trace; wir untersuchen es innerhalb von 72 Stunden.
