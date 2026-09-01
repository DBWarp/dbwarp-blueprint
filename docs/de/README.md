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

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../README.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet. Siehe [`docs/TRANSLATIONS.md`](../TRANSLATIONS.md).

**Sprachen:** [English](../../README.md) | **Deutsch** | [Français](../fr/README.md) | [Español](../es/README.md) | [Polski](../pl/README.md) | [日本語](../ja/README.md) | [中文](../zh/README.md)

## Was es ist

DBWarp Blueprint ist ein Datenbank-Blueprint-Kollektor, bei dem Vertrauen an erster Stelle steht. Sie führen ihn in Ihrer eigenen Umgebung mit PostgreSQL, MySQL oder SQL Server aus. Er liest Katalogmetadaten und, wenn Sie eine Komprimierungsmessung anfordern, zusätzlich eine begrenzte Zeilenstichprobe. Anschließend schreibt er einen anonymisierten strukturellen Blueprint Ihrer Datenbank: Tabellengrößen, Zeilenzahlen, Typfamilien sowie die Struktur von Indizes und Fremdschlüsseln.

Bezeichner werden durch schlüsselgebundene anonyme Kennzeichnungen ersetzt,
und es werden keine Zeilenwerte in den Blueprint geschrieben. Standardmäßig
verhindert ein neuer prozesslokaler Schlüssel Offline-Wörterbuchprüfungen;
`--anonymization-key-file` ermöglicht es dem Kunden, die Kennzeichnungen über
genehmigte Vergleichsläufe hinweg beizubehalten. Lesen Sie vor der Weitergabe
einer Ausgabe [`SECURITY.md`](SECURITY.md). Dort ist genau beschrieben, welche
Informationen die einzelnen Modi offenlegen und welche Optionen diesen Umfang
erweitern.

Die Ausgabe ist eine Klartextdatei. Sie können jede Zeile lesen, bevor Sie entscheiden, ob Sie sie weitergeben.

DBWarp Blueprint ist kostenlos und Open Source und wird vollständig in Ihrer eigenen Umgebung ausgeführt. Damit können Sie uns Fakten über Ihre Datenbank übermitteln, ohne uns Ihre Datenbank zu überlassen.

## Warum Sie es ausführen sollten

Wenn Sie Ihre Blueprint-Ausgabe an uns weitergeben, können wir Ihnen sagen, wie viel schneller DBWarp Ihre Daten übertragen würde und was dies für die Zeitpläne Ihrer Migration, Ihrer CI/CD-Testdaten und Ihrer Analysen bedeutet.

Die Entfernung ist besonders wichtig. Je weiter Ihre Daten übertragen werden müssen, desto größer ist die Verbesserung, die DBWarp Ihnen zeigen kann.

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; Zürich, Schweiz

---

`dbwarp-blueprint` ist das kundenseitige Werkzeug zur Erfassung der Datenstruktur für DBWarp. Es wird innerhalb der eigenen Kundenumgebung ausgeführt und erzeugt eine bereinigte, prüfbare Datei `blueprint.toml`, die DBWarp zur Dimensionierung von Migrationen, zur Erzeugung synthetischer Testdaten und zur Vorabplanung verwenden kann, ohne Zugriff auf die Datenbank, Dumps, Schemanamen oder Zeilendaten zu erhalten.

Das Werkzeug verbindet sich mit PostgreSQL, MySQL oder SQL Server, liest Katalogmetadaten, misst optional die lokale Komprimierung anhand einer begrenzten Zeilenstichprobe und schreibt TOML als Klartext. Es kann einen Blueprint auch offline aus lokalen Parquet- oder Avro-Dateien ableiten, wenn die Eingabe bereits als strukturierte Datendatei und nicht als Live-Datenbank vorliegt. Sie können die Ausgabe öffnen, jede Zeile prüfen und selbst entscheiden, ob Sie sie weitergeben.

Optional schreibt `--deck blueprint.pptx` zusätzlich eine PowerPoint-Zusammenfassung desselben anonymisierten Blueprints. Die Präsentation kann während eines Live-Datenbanklaufs oder später aus einer geprüften TOML-Datei mit `--from-toml blueprint.toml --deck blueprint.pptx` erzeugt werden. Der Präsentationsgenerator ist in die Rust-Binärdatei integriert und stellt keine Netzwerkverbindung her.

## Verwendungszweck

DBWarp benötigt genügend strukturelle Informationen, um eine Übertragung zu schätzen und zu planen:

- Anzahl der Tabellen;
- ungefähre Zeilenzahlen;
- Tabellen- und Indexgrößen;
- Spaltentypfamilien, exakte strukturelle Kapazitäten/Indexpräfixe und standardmäßig datenschutzgerecht gerundete beobachtete Breiten;
- Struktur von Indizes und Fremdschlüsseln;
- datenschutzgerechte Anzahlen von Nicht-Tabellenartefakten und externe Bereitstellungsvoraussetzungen;
- optionale Komprimierungszusammenfassungen für Tabellen und Spalten aus einer kleinen lokalen Stichprobe;
- optionale, kundenseitig ermittelte Datenbank-RTT-Werte.

Diese Fakten reichen aus, um die Übertragungsgröße zu schätzen, einen Ausgangsplan für DBWarp-Massenübertragungen auszuwählen und einen repräsentativen synthetischen Benchmark-Datensatz zu erzeugen. Sie reichen nicht aus, um das Schema oder die Daten des Kunden zu rekonstruieren.

## Was das Werkzeug nicht tut

`dbwarp-blueprint` tut Folgendes nicht:

- Telemetrie senden;
- DBWarp-Server aufrufen;
- die Blueprint-Datei hochladen;
- `~/.pgpass`, `~/.my.cnf`, Cloud-Anmeldedaten oder SSH-Schlüssel lesen;
- standardmäßige Passwort-Umgebungsvariablen wie `PGPASSWORD` oder `MYSQL_PWD` lesen;
- etwas anderes als die für den aktiven Modus ausgewählten Ausgaben schreiben; im Batch-Modus wird ein Bundle-Verzeichnis mit untergeordneten Blueprints, untergeordneten Auditprotokollen und optionalen Fehlernachweisen geschrieben;
- echte Tabellen-, Spalten-, Index- oder Schemanamen, Namen von Nicht-Tabellenobjekten, SQL-Definitionen, externe Endpunkte, Anmeldedaten, Schlüssel, Zertifikate, Binärdateien oder Zeilenwerte in die Ausgabe aufnehmen.

Live-Blueprint-Läufe öffnen eine Datenbanksitzung zu dem von Ihnen angegebenen Endpunkt. DNS kann den konfigurierten Resolver verwenden, und die integrierte Kerberos-/SSPI-Authentifizierung kann Identitätsinfrastruktur kontaktieren. Im Batch-Modus gilt diese Grenze für jede Datenbankquelle. Lokale TOML-, Parquet-, Avro- und Bundle-Operationen öffnen keine von der Anwendung initiierte Netzwerkverbindung.

## Herunterladen oder erstellen

| Weg | Am besten geeignet für | Link |
|---|---|---|
| Binärdatei herunterladen | schneller Test, Sales-Engineering-Termin, isolierter Testhost | [`binaries/README.md`](BINARIES.md) |
| Aus einem kleinen Quellcode-Klon erstellen | Sicherheitsprüfung, Produktionsrichtlinien, Reproduzierbarkeitsprüfung | [`BUILD.md`](BUILD.md) |
| Aus einem gebundelten Quellcodepaket erstellen | strenge Offline-Abhängigkeitsprüfung | GitHub Releases |

Der vertrauenswürdigste Weg ist die Erstellung aus dem Quellcode. Das normale Repository bleibt klein und verwendet `Cargo.lock`, um Abhängigkeitsversionen festzuschreiben. Für strengere Offline-Prüfungen veröffentlicht jedes Release außerdem ein gebundeltes Quellcodepaket mit jeder Abhängigkeitsquelldatei. Release-Binärdateien werden der Einfachheit halber mit SHA256-Prüfsummen bereitgestellt.

## Schnellstart

Wählen Sie bei Bedarf eine Anzeigesprache. Englisch ist die Standardsprache; vollständige Kataloge sind für Deutsch, Französisch, Spanisch, Polnisch, Japanisch und vereinfachtes Chinesisch eingebettet:

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --dry-run
```

Nur menschenlesbare Hilfetexte, Eingabeaufforderungen, Diagnosen, Fortschrittsmeldungen und Beschriftungen der PowerPoint-Präsentation werden übersetzt. Befehls- und Optionsnamen, zulässige Werte, URI-Schemata, Namen von Umgebungsvariablen, Selektoren, DBP-Codes, Audit-Schlüssel und erzeugtes TOML bleiben kanonische englische Token. Dadurch bleiben Automatisierung und Supportverfahren in jeder Sprache identisch. Siehe [`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

Führen Sie zuerst einen Probelauf aus. Er zeigt den Plan an, ohne eine Verbindung herzustellen:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --dry-run
```

Empfohlener produktionsnaher Lauf mit TLS, Auditprotokoll und Komprimierungsmessung:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Mit `--measure-compression --yes` enthält die Ausgabe zstd-Verhältnisse auf Tabellenebene und Komprimierungsprognosen pro Spalte. Die spaltenbezogenen Blöcke werden aus derselben begrenzten Stichprobe wie das Verhältnis auf Tabellenebene berechnet; sie sind für die DBWarp-Testdatenschätzung vorgesehen und schreiben keine Stichprobenwerte auf die Festplatte. Schema v3 und neuer gibt außerdem datenschutzgerechte Aggregate für Kardinalität und Verteilung je Spalte sowie abgeleitete Indexpräfix- und Beziehungsstatistiken aus. Temporäre Fingerprints sind im Speicher begrenzt und werden verworfen; Werte und Fingerprints erscheinen niemals in der Blueprint-TOML-Datei.

Seit Schema v4 inventarisieren Blueprints außerdem Nicht-Tabellenobjekte. Die Voreinstellung
`--artifact-detail summary` speichert begrenzte Anzahlen nach Objekt- und
externer Voraussetzungsklasse, ohne Definitionen zu lesen. `graph` liefert eine
anonyme Abhängigkeitstopologie; `analyzed` liefert begrenzte Sprachmerkmal- und
Komplexitätsbänder. Beide erfordern `--yes`, weil selbst ein anonymer Graph eine
Anwendung identifizieren kann:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
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

Die Standardrichtlinie `balanced` bewahrt deklarierte Zeichen-/Byte-Kapazitäten und Indexpräfixlängen exakt. Durchschnittliche und p95-Längen aus Stichproben verwenden Buckets mit relativem Fehler (maximal etwa 3,2 %, Werte bis 32 Byte bleiben exakt erhalten). Dadurch bleiben die erzeugten Daten für einen normalerweise 9 Zeichen langen `VARCHAR(3000)`-Schlüssel nahe bei 9 Zeichen, während gültige DDL-/Indexgrenzen der Quelle erhalten bleiben:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

Verwenden Sie exakte Stichprobenstatistiken nur, wenn Ihre Richtlinie diese zusätzliche Genauigkeit zulässt:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

Verwenden Sie `--length-fidelity strict`, um die ältere, grobe und für die Weitergabe geeignete Bucket-Bildung für deklarierte, beobachtete und Präfixlängen beizubehalten. Der strikte Modus opfert bewusst die Testdaten-/Indextreue und ist nicht für kundenbezogene Benchmarks geeignet. Die ältere Schreibweise `--preserve-exact-lengths --yes` bleibt als Kompatibilitätsalias für `--length-fidelity exact --yes` erhalten.

Neue Blueprint-Dateien zeichnen getrennte Felder `declared_length_fidelity`, `index_length_fidelity` und `observed_length_fidelity` auf. Das ältere Feld `length_metadata` bleibt zur konservativen Kompatibilität mit älteren Verbrauchern erhalten. PostgreSQL-Zeichenkapazitäten sind exakte Katalogwerte; kodierungsabhängige Bytegrenzen und Indexpräfixlängen bleiben nicht verfügbar.

Für einen kundenrepräsentativen erzeugten Benchmark ist `--measure-compression` nicht optional: Es liefert beobachtete durchschnittliche und p95-Wertlängen, damit ein deklarierter, mehrere Kilobyte großer Schlüssel, dessen tatsächliche Werte nur wenige Zeichen umfassen, nicht bis zu seiner Kapazität erzeugt wird. Das standardmäßige Zeitbudget für Stichproben beträgt 300 Sekunden. Erhöhen Sie `--max-wall-secs` für sehr große Schemata. Nachgelagerte Planungswerkzeuge sollten den Blueprint ablehnen, wenn eine nichtleere, variabel breite indizierte Spalte ohne Stichprobe bleibt. Eine Smoke- oder Kompatibilitätserzeugung erfordert dann eine explizite nachgelagerte Ausnahme und muss als nicht repräsentativ gekennzeichnet werden.

Prüfen Sie anschließend die Dateien:

```bash
less blueprint.toml
less audit.txt
```

Wenn es Ihre Richtlinie zulässt, geben Sie `blueprint.toml` an DBWarp weiter. Eine Präsentation kann nach ihrer Prüfung ebenfalls weitergegeben werden. Bewahren Sie das Auditprotokoll als zugriffsgeschützten Betriebsnachweis auf, sofern es nicht für einen konkreten Supportfall über einen genehmigten sicheren Kanal benötigt wird; es enthält Endpunkt-, Identitäts-, Pfad- und Zeitangaben.

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

Wenn Ihre Richtlinie das Dekodieren von Stichproben erlaubt, kann der Dateimodus auch die transportähnliche DBWarp-Komprimierung anhand begrenzter lokaler Stichproben schätzen:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Dieselben Optionen funktionieren mit `--from-avro`. Stichprobenwerte werden im Speicher als `dbwarp-blueprint-rowframe-v1` codiert; nur aggregierte zstd-Komprimierungsverhältnisse werden in das Blueprint-TOML geschrieben.

## Batch- und Bundle-Modus

Verwenden Sie für mehrere Datenbanken, mehrere Tabellen/Datensätze oder die Prüfung einer Kundenlandschaft ein Batch-Manifest und schreiben Sie ein Bundle-Verzeichnis:

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
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

Beispiele für Kerberos, SSPI und Entra ID finden Sie in [`AUTH.md`](AUTH.md). Informationen zu internen CAs, mTLS und Hostnamenprüfung finden Sie in [`TLS.md`](TLS.md).

## Reiner Katalogmodus

Wenn Ihre Richtlinie die Entnahme von Zeilenstichproben verbietet, lassen Sie `--measure-compression` weg:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

Der reine Katalogmodus liest ausschließlich Metadaten. DBWarp kann weiterhin anhand von Tabellengröße, Zeilenzahlen, Typfamilien sowie Index-/Fremdschlüsselstruktur schätzen; Komprimierung und Realitätsnähe synthetischer Testdaten sind jedoch geringer, weil die Entropie von Text-/Binärdaten abgeleitet werden muss.

## Ausgabevorschau

```toml
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 6
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 28
row_count = 12500000
table_bytes = 4200000000
index_bytes = 1100000000

[tables.table-001]
rows = 12500000
table_bytes = 4200000000
index_bytes = 1100000000
schema = "schema-A"
has_clustered_index = false

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false

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

- [`docs/QUICKSTART.md`](QUICKSTART.md): erster sicherer Lauf und erstes Übergabepaket.
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
