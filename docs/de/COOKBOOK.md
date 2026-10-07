# Rezeptbuch

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../COOKBOOK.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../COOKBOOK.md) | **Deutsch** | [Français](../fr/COOKBOOK.md) | [Español](../es/COOKBOOK.md) | [Polski](../pl/COOKBOOK.md) | [日本語](../ja/COOKBOOK.md) | [中文](../zh/COOKBOOK.md)

Aufgabenorientierte Rezepte für häufige Arbeitsabläufe mit `dbwarp-blueprint`.

## Rezept: Lokalisierte Bedienersitzung

Wählen Sie einen der vollständigen eingebetteten Sprachkataloge aus, während Befehle, Werte, Bezeichner und Ausgabeschemata kanonisch bleiben:

```bash
./dbwarp-blueprint --lang de --help
./dbwarp-blueprint --lang ja \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail none \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

Setzen Sie für unbeaufsichtigte Läufe `DBWARP_BLUEPRINT_LANG=fr` oder ein übliches Prozessgebietsschema. Ein explizites `--lang` hat immer Vorrang. DBP-Codes und technische Treiberdetails bleiben kanonisch, damit ein lokalisierter Fehler gesucht und an den Support weitergegeben werden kann.

## Rezept: PostgreSQL mit interner CA

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out pg-appdb.blueprint.toml \
  --audit-log pg-appdb.audit.txt
```

Verwenden Sie dies für die normale Prüfung einer produktiven PostgreSQL-Datenbank. Wenn die Hostnamenprüfung fehlschlägt, korrigieren Sie das Serverzertifikat oder verwenden Sie den richtigen DNS-Namen; verwenden Sie `--tls-skip-verify` ausschließlich für Loopback-Tests.

## Rezept: MySQL mit Benutzernamendatei

Nützlich, wenn der Benutzername Zeichen enthält, die sich nur umständlich URI-codieren lassen.

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

Die oben genannte Vorgehensweise verwendet bereits die Standard-Ausgewogenheitsrichtlinie: exakte MySQL-declaration/index-Metadaten und eng gerundete Stichprobenwerte.

Bestätigen Sie `declared_length_fidelity = "exact"`, `index_length_fidelity = "exact"` und `observed_length_fidelity = "relative-rounded-v2"`. Verwenden Sie `--length-fidelity exact --yes` nur, nachdem Ihr Unternehmen die Weitergabe genauer, stichprobenbasierter Längenangaben genehmigt hat. Namen und Werte bleiben ausgeschlossen.

Bei Datenbanken mit Tausenden von Tabellen sollte `--max-wall-secs` bei Bedarf über den Standardwert von 300 Sekunden erhöht werden. Fidelity-Marker beschreiben die Richtlinie; sie zeigen nicht, dass die Stichproben alle Tabellen erreicht haben.

## Rezept: SQL Server mit SQL-Authentifizierung

```bash
./dbwarp-blueprint \
  --connect sqlserver://sql-blueprint@sql-primary.internal,1433/appdb \
  --password-file /etc/dbwarp/sql-blueprint.pass \
  --auth-mode sql-auth \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-appdb.blueprint.toml \
  --audit-log mssql-appdb.audit.txt
```

TLS-Modi mit Zertifikatsprüfung für SQL Server verwenden den Trust Store des
Betriebssystems, wenn `--tls-ca` nicht angegeben ist. Eine bereitgestellte
`.pem`- oder `.crt`-Datei muss genau ein CA-Zertifikat enthalten und ersetzt
diese Stammzertifikate. Sowohl `verify-ca` als auch `verify-full` prüfen den
Verbindungshostnamen.

## Rezept: SQL Server mit Entra-ID-Token

Erzeugen Sie das Token außerhalb des Werkzeugs und übergeben Sie es anschließend per Datei:

```bash
install -d -m 700 "$HOME/.cache/dbwarp-blueprint"
TOKEN_FILE="$HOME/.cache/dbwarp-blueprint/sql-token"
install -m 600 /dev/null "$TOKEN_FILE"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

Azure SQL stellt ein Zertifikat einer öffentlichen CA bereit. Deshalb bleibt
`--tls-ca` in diesem Rezept ungesetzt und der Trust Store des Betriebssystems
wird verwendet. Eine angegebene Datei `--tls-ca` ersetzt diesen Trust Store
durch ein Zertifikat; siehe [TLS](TLS.md).

## Rezept: Sicherheitsprüfung nur anhand des Katalogs

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

Dies ist der Modus mit dem geringsten Aufwand für die Überprüfung. Er vermeidet die Stichprobenentnahme von Zeilen, erzeugt aber weniger genaue Kompressions- und Datenübertragungs-Schätzungen.

## Nicht-Tabellen-Migrationskomplexität bewerten

Beginnen Sie mit der Standardzusammenfassung, um Anzahlen und externe Voraussetzungen zu erfassen, ohne Definitionen zu lesen:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail summary \
  --out appdb-summary.blueprint.toml \
  --audit-log appdb-summary.audit.txt \
  --yes
```


Erfassen Sie nach der Sicherheitsfreigabe anonyme Abhängigkeiten und begrenzte Nachweise der Sprachkomplexität:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail analyzed \
  --out appdb-analyzed.blueprint.toml \
  --audit-log appdb-analyzed.audit.txt \
  --yes
```


Überprüfen Sie `visibility` sowie alle drei Vollständigkeitsflags `catalogs_unreadable`, `families_not_inventoried` und `counts_by_external_class`. Behandeln Sie jede externe Klasse als eine explizite Migrationaufgabe. Betrachten Sie ein inventarisertes Objekt nicht als Beweis dafür, dass DBWarp es neu erstellen oder übersetzen kann; fragen Sie DBWarp, welche Objekttypen für Ihre Migration unterstützt werden. Siehe [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Rezept: RTT-Prüfung deaktivieren

Standardmäßig führt das Werkzeug nach dem Verbindungsaufbau fünf `SELECT 1`-Prüfungen aus und gibt einen `[network]`-Block aus. Wenn ein DBA Abfragen außerhalb des Katalogs verbietet, deaktivieren Sie diese Prüfung:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --no-rtt-probe \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

Die RTT-Prüfung liest niemals Zeilendaten; jede Abfrage gibt die konstante Ganzzahl `1` zurück.

## Rezept: Komprimierungsstichprobe zeitlich begrenzen

Halten Sie den ersten Lauf bei großen Produktionssystemen konservativ:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal/appdb \
  --password-file /etc/dbwarp/mysql.pass \
  --measure-compression --yes \
  --sample-rows 500 \
  --max-wall-secs 120 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Wenn die Ausgabe viele Stichproben als verzerrt oder fehlend kennzeichnet, führen Sie den Lauf auf einer Lesereplik mit einem größeren Zeitbudget erneut aus.

## Rezept: Mehrere Datenbanken in einem Paket.

Verwenden Sie ein Batch-Manifest, wenn Sie ein einzelnes Paket für mehrere Datenbanken bereitstellen möchten, das überprüft werden soll.

`customer.batch.toml`:

```toml
[defaults]
measure_compression = true
sample_rows = 1000
max_wall_secs = 300
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["erp", "critical"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
tags = ["billing"]

[[source]]
id = "warehouse_sql"
kind = "sqlserver"
connect_env = "WAREHOUSE_SQL_URI"
password_file = "/etc/dbwarp/warehouse.pass"
auth_mode = "sql-auth"
tags = ["warehouse"]
```

Probelauf:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Ausführung:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Dadurch werden `bundle.toml`, eine untergeordnete Blueprint-Datei pro Quelle und ein Audit pro Quelle geschrieben. Die untergeordneten Blueprint-Dateien können unabhängig voneinander geprüft werden.

## Rezept: Mischung aus Datenbanken und Data-Lake-Dateien.

Verwenden Sie strukturierte Dateiquellen im selben Batch, wenn Sie Parquet- oder Avro-Extrakte zusammen mit aktiven Datenbanken haben.

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["database"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` führt Dateien wie `merge_same_schema` zusammen und speichert den angegebenen Modus im Bundle. Halten Sie nicht zusammengehörige Schemata in separaten Quellen.

## Rezept: Nur eine Quelle oder Tabelle aus einem Bundle extrahieren

Listen Sie nach einem Batch-Lauf die Quellen auf:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

Eine Quelle extrahieren:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Eine Tabelle aus einer Quelle extrahieren:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Verwenden Sie dies, wenn nur ein Teil eines Pakets zur Weitergabe freigegeben ist.

## Anleitung: Ein geprüftes Bundle zum Teilen zusammenstellen.

Das Arbeitsverzeichnis des Bundles enthält untergeordnete Blueprints und audits mit Zugriffskontrolle. Übertragen Sie es nicht vollständig. Nachdem Sie die Werte des Manifests und die untergeordneten Blueprints überprüft haben, erstellen Sie eine einzelne Datei zum Weitergeben:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

Die gepackte Datei behält vom Operator bereitgestellte Quell-IDs, Tags,
Datensatzgruppen-IDs und Auditpfad-Metadaten bei. Verwenden Sie anonyme Werte,
prüfen Sie das gepackte TOML und übertragen Sie es ausschließlich über den
freigegebenen Kanal.

## Rezept: Batch-Paket zum Teilen.

Befolgen Sie die [Anleitung zur Überprüfung und Freigabe](QUICKSTART.md#review-and-share). Behalten Sie das Arbeits-Manifest, die Protokolle und die Befehlseinträge lokal; erstellen Sie dieses separate Verzeichnis nur aus dem überprüften, komprimierten Blueprint.

```text
blueprint-share/
  customer-blueprint-bundle.packed.toml
```

## Rezept: Offline-Präsentation aus geprüftem TOML

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

Dieser Modus liest nur die TOML-Datei und schreibt die Präsentation. Er lehnt Optionen für Live-Datenbanken ab, statt sie stillschweigend zu ignorieren.

## Rezept: Byte-identische Reproduzierbarkeit

Schreiben Sie den Zeitstempel fest und verwenden Sie denselben geschützten
Anonymisierungsschlüssel, den Sie verwahren, erneut:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --anonymization-key-file /etc/dbwarp/anonymization.key \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

Die Schlüsseldatei muss genau 32 rohe Bytes oder 64 hexadezimale Zeichen enthalten, darf unter Unix nicht "group/world-readable" sein und darf niemals weitergegeben werden. Ohne diese Option ändert ein frisch generierter, betriebssystem-zufälliger Schlüssel absichtlich die Reihenfolge der anonymen Kennzeichnungen bei jedem Ausführungsvorgang. Das Festlegen von nur "`--generated-at`" ist nicht ausreichend. Verwenden Sie das vollständige Verfahren für zugelassene forensische Snapshots; ein Deck, das zweimal aus genau demselben, überprüften Blueprint generiert wurde, bleibt byte-identisch, wenn sein Zeitstempel und seine Sprache unverändert sind.

## Rezept: Paket zum Teilen mit DBWarp.

Befolgen Sie die [Anleitung zur Überprüfung und Freigabe](QUICKSTART.md#review-and-share). Das Standardpaket enthält nur das genehmigte Blueprint:

```text
blueprint-share/
  blueprint.toml
```

Fügen Sie `blueprint.pptx` nur nach separater Prüfung und Genehmigung hinzu. Bewahren Sie Protokolle, Befehlsprotokolle und credential/key-Materialien nicht im freigegebenen Verzeichnis auf; senden Sie Protokolle nur für einen bestimmten Support-Zweck über einen genehmigten, sicheren Kanal.
