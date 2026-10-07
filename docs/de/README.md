<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../.github/assets/dbwarp-logo-dark.png">
    <img src="../../.github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

# dbwarp-blueprint

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../README.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet. Siehe die [Richtlinie für Dokumentationsübersetzungen](../TRANSLATIONS.md).

[`MACHINE_TRANSLATIONS.md`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/MACHINE_TRANSLATIONS.md).

**Sprachen:** [English](../../README.md) | **Deutsch** | [Français](../fr/README.md) | [Español](../es/README.md) | [Polski](../pl/README.md) | [日本語](../ja/README.md) | [中文](../zh/README.md)

## Was es ist

DBWarp Blueprint ist ein Datenbank-Blueprint-Kollektor, bei dem Vertrauen an erster Stelle steht. Sie führen ihn in Ihrer eigenen Umgebung mit PostgreSQL, MySQL oder SQL Server aus. Er liest Katalogmetadaten und, wenn Sie eine Komprimierungsmessung anfordern, zusätzlich eine begrenzte Zeilenstichprobe. Anschließend schreibt er einen anonymisierten strukturellen Blueprint Ihrer Datenbank: Tabellengrößen, Zeilenzahlen, Typfamilien sowie die Struktur von Indizes und Fremdschlüsseln.

Identifikatoren werden durch schlüsselgebundene, anonymisierte HMAC-Kennzeichnungen ersetzt, und keine Zeilenwerte werden in die Blueprint geschrieben. Ein neuer, prozesslokaler Schlüssel verhindert standardmäßig Offline-Dictionary-Prüfungen; `--anonymization-key-file` ermöglicht es Ihnen, die Kennzeichnungen über genehmigte Vergleichsläufe hinweg beizubehalten. Lesen Sie [`SECURITY.md`](SECURITY.md), bevor Sie eine Ausgabe weitergeben: Es wird genau beschrieben, welche Informationen jeder Modus preisgibt und welche Optionen dies erweitern.

Die Ausgabe ist eine Klartextdatei. Sie können jede Zeile lesen, bevor Sie entscheiden, ob Sie sie weitergeben.

DBWarp Blueprint ist kostenlos und Open Source und wird vollständig in Ihrer eigenen Umgebung ausgeführt. Damit können Sie uns Fakten über Ihre Datenbank übermitteln, ohne uns Ihre Datenbank zu überlassen.

## Warum Sie es ausführen sollten

Wenn Sie Ihre Blueprint-Ausgabe an uns weitergeben, können wir Ihnen sagen, wie viel schneller DBWarp Ihre Daten übertragen würde und was dies für die Zeitpläne Ihrer Migration, Ihrer CI/CD-Testdaten und Ihrer Analysen bedeutet.

Die Entfernung ist besonders wichtig. Je weiter Ihre Daten übertragen werden müssen, desto größer ist die Verbesserung, die DBWarp Ihnen zeigen kann.

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; Zürich, Schweiz

---

Führen Sie `dbwarp-blueprint` in Ihrer eigenen Umgebung aus, um eine begrenzte, anonymisierte und überprüfbare `blueprint.toml`-Datei zu erstellen, die DBWarp für die Dimensionierung und Planung der Migration verwenden kann, ohne Zugriff auf die Datenbank, Dumps, Schemanamen oder Zeilendaten zu benötigen.

Das Werkzeug verbindet sich mit PostgreSQL, MySQL oder SQL Server, liest Katalogmetadaten, misst optional die lokale Komprimierung anhand einer begrenzten Zeilenstichprobe und schreibt TOML als Klartext. Es kann einen Blueprint auch offline aus lokalen Parquet- oder Avro-Dateien ableiten, wenn die Eingabe bereits als strukturierte Datendatei und nicht als Live-Datenbank vorliegt. Sie können die Ausgabe öffnen, jede Zeile prüfen und selbst entscheiden, ob Sie sie weitergeben.

Optional schreibt `--deck blueprint.pptx` zusätzlich eine PowerPoint-Zusammenfassung desselben anonymisierten Blueprints. Die Präsentation kann während eines Live-Datenbanklaufs oder später aus einer geprüften TOML-Datei mit `--from-toml blueprint.toml --deck blueprint.pptx` geschrieben werden. Der Präsentationsersteller ist in die Rust-Binärdatei integriert und stellt keine Netzwerkverbindung her.

## Verwendungszweck

DBWarp benötigt genügend strukturelle Informationen, um eine Übertragung zu schätzen und zu planen:

- Anzahl der Tabellen;
- ungefähre Zeilenzahlen;
- Tabellen- und Indexgrößen;
- Spaltentypfamilien, exakte strukturelle Kapazitäten/Indexpräfixe und standardmäßig datenschutzgerecht gerundete beobachtete Breiten;
- Struktur von Indizes und Fremdschlüsseln;
- begrenzte, namenfreie Anzahlen von Nicht-Tabellenartefakten und externe Bereitstellungsvoraussetzungen;
- optionale Komprimierungszusammenfassungen für Tabellen und Spalten aus einer kleinen lokalen Stichprobe;
- optionale Messung der Round-Trip-Zeit vom Collector zu Ihrer Datenbank.

Diese Fakten reichen aus, um die Übertragungsgröße abzuschätzen und eine Übertragung zu planen. Quellnamen und Zeilenwerte werden weggelassen, aber die charakteristische Struktur und die Statistiken können dennoch ein Arbeitslastprofil erstellen; Anonymisierung ist eine Risikominderung, aber keine Garantie für Unumkehrbarkeit.

## Was das Werkzeug nicht tut

`dbwarp-blueprint` tut Folgendes nicht:

- Telemetrie senden;
- DBWarp-Server aufrufen;
- die Blueprint-Datei hochladen;
- `~/.pgpass`, `~/.my.cnf`, Cloud-Anmeldedaten oder SSH-Schlüssel lesen;
- standardmäßige Passwort-Umgebungsvariablen wie `PGPASSWORD` oder `MYSQL_PWD` lesen;
- implizite Systemverzeichnisse für temporäre Dateien, Cache oder Konfiguration verwenden; es schreibt die ausdrücklich ausgewählten Ausgabedateien, während der Batch-Modus für die atomare Veröffentlichung außerdem ein benachbartes Staging- oder Wiederherstellungsverzeichnis neben `--out-dir` verwendet;
- echte Tabellen-, Spalten-, Index- oder Schemanamen, Namen von Nicht-Tabellenobjekten, SQL-Definitionen, externe Endpunkte, Anmeldedaten, Schlüssel, Zertifikate, Binärdateien oder Zeilenwerte in die Ausgabe aufnehmen.

Live-Blueprint-Läufe öffnen eine Datenbanksitzung zu dem von Ihnen angegebenen Endpunkt. DNS kann den konfigurierten Resolver verwenden, und die integrierte Kerberos-/SSPI-Authentifizierung kann Identitätsinfrastruktur kontaktieren. Im Batch-Modus gilt diese Grenze für jede Datenbankquelle. Lokale TOML-, Parquet-, Avro- und Bundle-Operationen öffnen keine von der Anwendung initiierte Netzwerkverbindung.

## Herunterladen oder erstellen

| Weg | Am besten geeignet für | Link |
|---|---|---|
| Laden Sie eine Binärdatei herunter. | schneller Testlauf, isolierter Testhost | [`binaries/README.md`](BINARIES.md) |
| Aus einem kleinen Quellcode-Klon erstellen | Sicherheitsprüfung, Produktionsrichtlinien, Reproduzierbarkeitsprüfung | [`BUILD.md`](BUILD.md) |
| Aus einem gebundelten Quellcodepaket erstellen | strenge Offline-Abhängigkeitsprüfung | GitHub Releases |
| SQL-Fallback prüfen und ausführen | DBA-Richtlinie lehnt eine Drittanbieter-Binärdatei ab | [`sql/blueprint.pg.sql`](../../sql/blueprint.pg.sql), [`sql/blueprint.mysql.sql`](../../sql/blueprint.mysql.sql), [`sql/blueprint.sqlserver.sql`](../../sql/blueprint.sqlserver.sql) und [`blueprint_format.py`](../../blueprint_format.py) |

### Grenze des SQL-Fallbacks

Der SQL-Fallback ist eine prüfbare Katalog-Mindestlösung und kein funktionsgleicher Ersatz für den Rust-Collector. Jedes SQL-Skript schreibt ein JSON-Zwischendokument mit echten Schema-, Tabellen-, Spalten- und Indexnamen; MySQL `COLUMN_TYPE` kann außerdem deklarierte enum/set-Mitglieder enthalten. Behandeln Sie dieses JSON als sensibles Schemamaterial, behalten Sie es in der Quellumgebung, normalisieren Sie es dort mit `blueprint_format.py` und geben Sie nur die geprüfte TOML-Ausgabe weiter.

Der Fallback besitzt keinen `--schema`-Selektor: PostgreSQL erfasst gewöhnliche Tabellen in allen Nicht-Systemschemata der verbundenen Datenbank; MySQL und SQL Server erfassen gewöhnliche lokale Benutzertabellen der gewählten Datenbank. Verwenden Sie ihn nicht, wenn nur ein Teilbereich genehmigt ist. Seine TOML enthält den Teilbestand gewöhnlicher Tabellen mit Tabellen-/Spalten-/Index-/FK-Struktur und ungefähren lokalen Größen, inventarisiert aber nicht jede Tabellenart aus v7. Alle Strukturfamilien werden als unvollständig markiert; MySQL-FEDERATED- und externe SQL-Server-Tabellen werden ausgeschlossen, statt ihre entfernten Daten fälschlich als lokal auszuweisen. Der Fallback enthält außerdem keine Zeilenstichprobe, RTT-Evidenz, Nicht-Tabellenartefakt-Inventarisierung oder Live-Topologieabfragen; Topologie und Vollständigkeit des Datensatzes werden ausdrücklich als `unknown` ausgewiesen.

Der vertrauenswürdigste Weg ist die Erstellung aus dem Quellcode. Das normale Repository bleibt klein und verwendet `Cargo.lock`, um Abhängigkeitsversionen festzuschreiben. Für strengere Offline-Prüfungen veröffentlicht jedes Release außerdem ein gebundeltes Quellcodepaket mit jeder Abhängigkeitsquelldatei. Release-Binärdateien werden der Einfachheit halber mit SHA256-Prüfsummen bereitgestellt.

## Schnellstart

Lassen Sie vor jedem Live-Lauf von einem DBA ein dediziertes
Collector-Konto mit minimalen Rechten bereitstellen. Verwenden Sie dafür unter
[`sql/grants/`](../../sql/grants/) das zur Engine, Version und Stufe passende
Skript und genehmigen Sie den exakten Schemaumfang. Beginnen Sie niemals mit
einem Anwendungseigentümer- oder Administratorkonto. Übergeben Sie jedes
genehmigte Schema mit `--schema` und entfernen Sie das dedizierte Konto nach
der Erfassung mit dem passenden Skript unter
[`sql/revoke/`](../../sql/revoke/). Das vollständige Erstverfahren steht in
[`docs/QUICKSTART.md`](QUICKSTART.md).

Wählen Sie bei Bedarf eine Anzeigesprache. Englisch ist die Standardsprache; vollständige Kataloge sind für Deutsch, Französisch, Spanisch, Polnisch, Japanisch und vereinfachtes Chinesisch eingebettet:

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --schema app --dry-run
```

Nur menschenlesbare Hilfetexte, Eingabeaufforderungen, Diagnosen, Fortschrittsmeldungen und Beschriftungen der PowerPoint-Präsentation werden übersetzt. Befehls- und Optionsnamen, zulässige Werte, URI-Schemata, Namen von Umgebungsvariablen, Selektoren, DBP-Codes, Audit-Schlüssel und erzeugtes TOML bleiben kanonische englische Token. Dadurch bleiben Automatisierung und Supportverfahren in jeder Sprache identisch. Siehe [`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

Bevor Sie eine Verbindung zu einer Datenbank herstellen, sollten Sie die Beispiele unter [`samples/`](../../samples/) einsehen. Sie sind einfache Blueprint-TOML-Dateien und erfordern keine spezielle Einrichtung, um sie zu überprüfen. Nachdem Sie eine Binärdatei erhalten haben, kann ein erster Offline-Durchlauf eine davon als Präsentation darstellen, ohne dass Datenbank- oder Netzwerkzugriff erforderlich ist:

```bash
./dbwarp-blueprint --from-toml samples/sqlserver-v6-analyzed.toml --deck sample.pptx
```

Beginnen Sie mit den kleinen Blueprint-Beispielen, die in [`samples/README.md`](../../samples/README.md) beschrieben sind: PostgreSQL (nur Katalog), MySQL (mit Stichproben) und SQL Server (mit Stichproben und analysierten Artefakten). Dies sind manuell erstellte Beispiele, keine echten Erfassungen. Die größeren Schema-v1-Beispiele zeigen das ältere Format, das aber noch lesbar ist. Verwenden Sie [`FORMAT.md`](FORMAT.md), um die vollständige Ausgabe zu überprüfen, bevor Sie eine Erfassung genehmigen; kein Beispiel deckt alle optionalen Felder ab.
Führen Sie zuerst einen Probelauf aus. Er zeigt den Plan an, ohne eine Verbindung herzustellen:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --dry-run
```

Empfohlener produktionsnaher Lauf mit TLS, Auditprotokoll und Komprimierungsmessung:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Mit `--measure-compression --yes` umfasst die Ausgabe zstd-Verhältnisse auf Tabellenebene sowie Kompressionsprognosen pro Spalte. Die Blöcke pro Spalte werden aus derselben begrenzten Stichprobe berechnet wie das Verhältnis auf Tabellenebene; sie verfeinern die Übertragungsschätzung und schreiben keine Stichprobenwerte auf die Festplatte. Eine überwiegend binäre Stichprobe mit erkennbaren Standard-Kompressionscontainer-Signaturen erhält nur das grobe Label `style = "precompressed"`; kein Dateityp, keine Signatur oder Stichprobenwert wird serialisiert. Schema v3 und neuerer Versionen geben außerdem begrenzte, namenlose Aggregationswerte pro Spalte (cardinality/skew) und abgeleitete Zusammenfassungen (index-prefix/relationship) aus. Temporäre Hashes pro Wert sind im Speicher begrenzt und werden verworfen; Stichprobenwerte und Hashes pro Wert erscheinen niemals in der Blueprint-TOML-Datei, während die dokumentierten Aggregationswerte dies tun.

Seit Schema v4 inventarisieren Blueprints außerdem Nicht-Tabellenobjekte. Die Voreinstellung
`--artifact-detail summary` speichert begrenzte Anzahlen nach Objekt- und
externer Voraussetzungsklasse, ohne Definitionen zu lesen. `graph` liefert eine
anonyme Abhängigkeitstopologie; `analyzed` liefert begrenzte Sprachmerkmal- und
Komplexitätsbänder. Beide erfordern `--yes`, weil selbst ein anonymer Graph eine
Anwendung identifizieren kann:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```


Das Vorhandensein eines Artefakts ist Planungsnachweis, aber keine Zusage, dass
DBWarp es automatisch neu erstellen oder übersetzen kann. Siehe
[`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

### Längentreue bei MySQL

Die Standardrichtlinie `balanced` bewahrt deklarierte Zeichen-/Byte-Kapazitäten und Indexpräfixlängen exakt. Durchschnittliche und p95-Längen aus Stichproben verwenden Buckets mit relativem Fehler (maximal etwa 3,2 %, Werte bis 32 Byte bleiben exakt erhalten). Dadurch bleibt ein normalerweise 9 Zeichen langer `VARCHAR(3000)`-Schlüssel nahe bei 9 Zeichen, sodass die Größenbestimmung die tatsächlichen Wertebreiten widerspiegelt und gleichzeitig gültige DDL-/Indexgrenzen der Quelle erhalten bleiben:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

Verwenden Sie exakte Stichprobenstatistiken nur, wenn Ihre Richtlinie diese zusätzliche Genauigkeit zulässt:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

Verwenden Sie `--length-fidelity strict`, um eine grobe Anonymisierung durch Gruppierung für angegebene, beobachtete und Präfixlängen anzuwenden. Der "strenge" Modus reduziert die Genauigkeit der resultierenden Schätzung. Die Schreibweise `--preserve-exact-lengths --yes` ist weiterhin ein Alias für `--length-fidelity exact --yes`.

Neue Blueprints zeichnen separate Felder `declared_length_fidelity`, `index_length_fidelity` und `observed_length_fidelity` auf. Das Feld `length_metadata` wird ebenfalls geschrieben, sodass Tools, die das frühere Format lesen, weiterhin funktionieren. Die Zeichenkapazitäten von PostgreSQL sind exakte Katalogwerte; codierungsabhängige Byte-Obergrenzen und Index-Präfixlängen sind weiterhin nicht verfügbar.

Für die genaueste Schätzung sollte man `--measure-compression` ausführen: Es werden die beobachteten durchschnittlichen und p95-Werte der Länge aufgezeichnet, sodass eine Spalte, die deutlich breiter als ihre tatsächlichen Werte definiert ist, nicht überschätzt wird. Das Standard-Sampling-Zeitlimit beträgt 300 Sekunden; erhöhen Sie `--max-wall-secs` für sehr große Schemata.

Prüfen Sie anschließend die Dateien:

```bash
less blueprint.toml
less audit.txt
```

Befolgen Sie die [Schritte zum Überprüfen und Weiterleiten](QUICKSTART.md#review-and-share), bevor Sie ein beliebiges Artefakt versenden. Teilen Sie nur genehmigte Blueprint-Inhalte und, falls separat geprüft und genehmigt, eine Präsentation; behalten Sie betriebliche Nachweise standardmäßig lokal.

## Modus für strukturierte Dateien

Wenn die Quelle bereits eine lokale strukturierte Datei ist, erzeugen Sie Blueprint-TOML ohne Datenbank-Anmeldedaten:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

```bash
./dbwarp-blueprint \
  --from-avro /data/sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Der Parquet-Modus liest Footer- und Zeilengruppenmetadaten. Avro-Objektcontainer besitzen keine entsprechende Zeilenanzahl im Footer; deshalb durchläuft der Avro-Modus den Container, um Datensätze zu zählen, und verwendet das Writer-Schema für die Spaltenstruktur. Keiner der beiden Modi verbindet sich mit einer Datenbank oder liest Optionen für Anmeldedaten.

Wenn Ihre Richtlinie das Dekodieren von Stichproben erlaubt, kann der Dateimodus auch die begrenzte lokale Komprimierbarkeit für die Übertragungsplanung messen:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Dieselben Optionen funktionieren mit `--from-avro`. Stichprobenwerte werden im Speicher als `blueprint-compression-probe-v2` codiert; der Blueprint speichert aggregierte Messungen für Komprimierung, NULL-Dichte, Kardinalität/Häufigkeit, Länge und Stil, niemals Stichprobenwerte.

## Batch- und Bundle-Modus

Für mehrere Datenbanken, mehrere tables/datasets oder eine Überprüfung eines gesamten Systems, verwenden Sie ein Batch-Manifest und erstellen Sie ein Bundle-Verzeichnis:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Das Arbeitsverzeichnis enthält `bundle.toml`, untergeordnete Blueprint-Dateien pro Quelle und zugriffsgeschützte Auditprotokolle pro Quelle. Übertragen Sie standardmäßig nicht das gesamte Arbeitsverzeichnis. Sie können es auflisten, extrahieren oder ein separat geprüftes, gepacktes Blueprint-Bundle erstellen:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
./dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 --out table-042.blueprint.toml
./dbwarp-blueprint --bundle-pack customer-blueprint-bundle --out customer-blueprint-bundle.packed.toml
```

Siehe [`docs/BATCH_AND_BUNDLES.md`](BATCH_AND_BUNDLES.md) zur Manifestsyntax, zu Datensatzmodi für strukturierte Dateien und zu Selektorregeln.

## Allgemeine Datenbankbefehle

PostgreSQL:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

MySQL:

```bash
./dbwarp-blueprint \
  --connect mysql://app@db.internal/payments \
  --schema payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

SQL Server:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@db.internal,1433/payments \
  --schema dbo \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

Beispiele für Kerberos, SSPI und Entra ID finden Sie in [`AUTH.md`](AUTH.md). Informationen zu internen CAs, mTLS und Hostnamenprüfung finden Sie in [`TLS.md`](TLS.md).

## Reiner Katalogmodus

Wenn Ihre Richtlinie nur Tabellen-/Spalten-/Index-/FK-Kataloge erlaubt, lassen Sie `--measure-compression` weg und deaktivieren Sie ausdrücklich die standardmäßige Zusammenfassung von Nicht-Tabellenobjekten:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

Dieser Katalog-only-Modus liest Metadaten von Tabellen, Statistiken und eine Topologie-Analyse, die nur die Anzahl der Objekte erfasst, aber keine Zeilenwerte oder Inventare von Nicht-Tabellen-Objekten. DBWarp kann immer noch Schätzungen anhand der Tabellengröße, der Anzahl der Zeilen, der Datentypen und der index/FK-Form vornehmen, aber die Kompressionsschätzungen sind schwächer, da die text/binary-Entropie abgeleitet werden muss. Ohne `--artifact-detail none` liest die Standard-Zusammenfassung auch Kataloge von Nicht-Tabellen-Objekten, aber nicht deren Definitionen.

## Ausgabevorschau

```toml
# dbwarp-blueprint v7
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 7
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 1
row_count = 12500000
table_bytes = 4194304000
index_bytes = 1048576000

[database_topology]
contract = "dbwarp-blueprint-topology/v2"
deployment = "unknown"
local_role = "unknown"
visibility = "unknown"
member_count = 0
member_count_scope = "unknown"
identifiers_redacted = true

[dataset_scope]
contract = "dbwarp-blueprint-dataset-scope/v1"
layout = "unknown"
table_inventory_completeness = "unknown"
row_count_completeness = "unknown"
size_completeness = "unknown"
row_count_method = "postgres-planner-estimate"
size_method = "postgres-local-relation-size"
limitations = ["topology-unobserved", "topology-visibility-unknown"]

[structure_scope]
contract = "dbwarp-blueprint-structure-scope/v1"
visibility = "unknown"
table_inventory_completeness = "unknown"
column_inventory_completeness = "unknown"
index_inventory_completeness = "unknown"
relationship_inventory_completeness = "unknown"
limitations = ["metadata-visibility-unknown"]

[source_environment]
contract = "dbwarp-blueprint-source-environment/v1"
evidence_origin = "database-endpoint"
hosting_model = "unknown"
infrastructure_location = "unknown"
capacity_scope = "connected-instance"
capacity_visibility = "partial"
cpu_capacity_band = "unknown"
cpu_capacity_basis = "unknown"
memory_capacity_band = "under-2-gib"
memory_capacity_basis = "database-buffer-cache"
collector_machine_excluded = true
catalogs_read = ["pg-capacity-settings"]

[statistics_evidence]
contract = "dbwarp-blueprint-statistics-evidence/v1"
visibility = "unknown"
table_count = 1
counts_by_statistics_state = { unknown = 1 }
counts_by_row_count_quality = { unknown = 1 }
counts_by_size_quality = { unknown = 1 }
limitations = ["statistics-provenance-unclassified"]

[artifact_inventory]
contract = "dbwarp-blueprint-artifacts/v2"
detail = "none"
scope = "all-visible-schemas"
visibility = "unknown"
inventory_complete = false
dependencies_complete = false
requirements_complete = false
analysis_complete = false
families_not_inventoried = ["non_table_objects"]

[tables.table-001]
rows = 12500000
table_bytes = 4194304000
index_bytes = 1048576000
schema = "schema-A"
has_clustered_index = false
object_kind = "ordinary-table"
storage_organization = "unknown"
partitioning = "none"
segment_state = "unknown"

[tables.table-001.statistics]
row_count_method = "postgres-planner-estimate"
row_count_quality = "unknown"
statistics_state = "unknown"
refresh_age_band = "unknown"
modification_ratio_band = "unknown"
sample_fraction_band = "unknown"
statistics_scope = "unknown"
size_method = "postgres-local-relation-size"
size_quality = "unknown"
size_scope = "unknown"
size_accounting = "unknown"
size_visibility = "unknown"

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false
numeric_model = "integer"
numeric_precision_radix = "decimal"

[tables.table-001.idxs.idx-1]
type = "btree"
primary = true
unique = true
cols = [1]
```

Der vollständige Dateivertrag ist in [`FORMAT.md`](FORMAT.md) dokumentiert. Das Auditprotokoll ist in [`AUDIT.md`](AUDIT.md) dokumentiert.

## Visuelle Zusammenfassungspräsentation

Erzeugen Sie während des Live-Laufs eine Präsentation:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

Oder erstellen Sie sie später aus einer geprüften Blueprint-Datei, ohne Datenbankverbindung:

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx
```

Die Präsentation passt sich an die Schemagröße an: Details pro Tabelle für kleine Schemata, Charakterisierungsfolien für große Schemata, eine Komprimierungszusammenfassung bei vorhandenen Tier-2-Daten und eine Folie zum Vertrauensmodell. Siehe [`DECK.md`](DECK.md).

## Dokumentation

Beginnen Sie hier:

- [`docs/QUICKSTART.md`](QUICKSTART.md): erster sicherer Durchlauf und was geteilt werden soll.
- [`docs/COOKBOOK.md`](COOKBOOK.md): praktische Rezepte für PostgreSQL, MySQL, SQL Server, TLS, Präsentationen und Abläufe ohne Stichproben.
- [`docs/DBA_REVIEW_GUIDE.md`](DBA_REVIEW_GUIDE.md): was DBA- und Sicherheitsprüfer vor der Ausführung des Werkzeugs wissen müssen.
- [`sql/grants/README.md`](../../sql/grants/README.md): versionsabhängige Skripte für geringstmögliche Berechtigungen und die Entfernung des Kontos nach der Erfassung.
- [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md): häufige Fehler und Lösungen.
- [`docs/MESSAGES.md`](MESSAGES.md): stabile Bedienermeldungscodes `DBPnnnnS`.
- [`docs/COMPRESSION_MEASUREMENT.md`](COMPRESSION_MEASUREMENT.md): Funktionsweise der Tier-2-Komprimierungsstichprobe.
- [`docs/INDEX.md`](INDEX.md): vollständige Dokumentationsübersicht.

Ausgangspunkte für die Sicherheitsprüfung:

- [`SECURITY.md`](SECURITY.md): Sicherheitsmodell und Umgang mit Anmeldedaten.
- [`AUDIT.md`](AUDIT.md): was gelesen, geschrieben, abgefragt und protokolliert wird.
- [`FORMAT.md`](FORMAT.md): Ausgabefelder und Rundungsregeln.
- [`TLS.md`](TLS.md): TLS- und mTLS-Verhalten.
- [`AUTH.md`](AUTH.md): unterstützte Authentifizierungsmodi.
- [`BUILD.md`](BUILD.md): Erstellung aus dem Quellcode und Release-Verifizierung.
- [`DECK.md`](DECK.md): optionale PowerPoint-Zusammenfassungspräsentation.

## Lizenz

Apache-2.0 OR MIT.
