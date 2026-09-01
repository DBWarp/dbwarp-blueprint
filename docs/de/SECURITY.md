# Sicherheitsmodell

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../SECURITY.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../SECURITY.md) | **Deutsch** | [Français](../fr/SECURITY.md) | [Español](../es/SECURITY.md) | [Polski](../pl/SECURITY.md) | [日本語](../ja/SECURITY.md) | [中文](../zh/SECURITY.md)

`dbwarp-blueprint` besitzt getrennte Modi für Live-Datenbanken, strukturierte Dateien, Batch-Verarbeitung, Bundles und Präsentationen. Der ausgewählte Modus bestimmt den Netzwerk- und Dateisystemumfang. Das Werkzeug besitzt keinen Telemetrie-, Updateprüfungs-, Lizenzprüfungs-, Analyse- oder Uploadpfad.

Diese Seite erläutert die Sicherheitsgrenzen, damit Ihr Team entscheiden kann, ob es das Werkzeug ausführen darf.

## Melden einer Sicherheitslücke

Melden Sie vermutete Sicherheitslücken bitte vertraulich über
[GitHub Private Vulnerability Reporting](https://github.com/DBWarp/dbwarp-blueprint/security/advisories/new).
Veröffentlichen Sie keine sicherheitsrelevanten Details in einem öffentlichen
Issue. Geben Sie die genaue Release-Version, das Betriebssystem, Schritte zur
Reproduktion und nur den kleinsten sicheren Nachweis an, der für die Bewertung
der Meldung erforderlich ist.

## Netzwerk

| Modus | Netzwerknutzung zur Laufzeit |
|---|---|
| Live `--connect` | Eine Datenbanktreibersitzung zum angegebenen Datenbankendpunkt. Die DNS-Auflösung kann den konfigurierten Resolver kontaktieren. Die integrierte Kerberos-/SSPI-Authentifizierung kann außerdem konfigurierte Identitätsinfrastruktur wie einen KDC oder Domänencontroller kontaktieren. |
| `--batch-manifest` | Eine Datenbanktreibersitzung für jede Datenbankquelle im Manifest, die sequenziell verarbeitet wird. Lokale Parquet- und Avro-Quellen verwenden kein Netzwerk. Die obigen DNS- und integrierten Authentifizierungsbedingungen gelten weiterhin. |
| `--from-toml`, `--from-parquet`, `--from-avro`, `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Keine von der Anwendung initiierte Netzwerkverbindung. Eingaben auf Netzwerk-Dateisystemen bleiben eine Angelegenheit des Betriebssystems beziehungsweise Speichersystems. |

Das Werkzeug ruft weder einen DBWarp-Dienst noch eine Cloud-API auf. Datenbanktreiber und Host-Betriebssystem können den oben beschriebenen Protokollhilfsverkehr erzeugen.

`--max-wall-secs` setzt zwei unabhängige Schutzmechanismen. PostgreSQL verwendet
ein sitzungslokales `statement_timeout`, MySQL ein sitzungslokales
`max_execution_time` für die schreibgeschützten `SELECT`-Anweisungen des
Kollektors. SQL Server besitzt keine gleichwertige Sitzungseinstellung für die
gesamte Laufzeit einer Anweisung; der Kollektor setzt daher das sitzungslokale
`LOCK_TIMEOUT`, um Sperrwartezeiten zu begrenzen, und behält für andere
Stillstände die Client-Frist bei. Läuft diese Client-Frist ab, trennt das
Werkzeug seine Verbindung. Es behauptet nicht, dass SQL Server eine
serverseitige Abbruchanforderung bestätigt hat. Vergewissern Sie sich vor einem
erneuten Versuch, dass die Serverarbeit beendet ist.

## Gelesene Dateien

Zur Laufzeit liest das Werkzeug nur Eingaben, die auf der Befehlszeile ausgewählt oder von einer Batch-/Bundle-Eingabe referenziert werden:

| Datei | Zeitpunkt/Zweck |
|---|---|
| `--user-file` | Quelle des Benutzernamens |
| `--password-file` | Quelle des Passworts |
| `--anonymization-key-file` | optionaler kundenseitig verwahrter HMAC-Schlüssel, der anonyme Objektkennzeichnungen über genehmigte Läufe hinweg erhält; der Dateimodus darf unter Unix keinen Lesezugriff für Gruppe/Andere zulassen |
| `--azure-token-file` | Quelle des SQL-Server-Entra-ID-Tokens |
| `--tls-ca` | vertrauenswürdiges CA-Bundle |
| `--tls-cert` | TLS-Clientzertifikat |
| `--tls-key` | privater TLS-Clientschlüssel |
| `--from-toml` | vorhandene dbwarp-blueprint-TOML-Datei zur Offline-Erstellung einer Präsentation |
| `--from-parquet` | Parquet-Metadaten und, bei ausdrücklicher Zustimmung zur Stichprobe, begrenzte dekodierte Zeilen |
| `--from-avro` | Metadaten und Datensätze eines Avro-Objektcontainers; zum Zählen der Datensätze muss der Container durchlaufen werden |
| `--batch-manifest` | Batch-Manifest sowie jede darin referenzierte lokale strukturierte Datei, Anmeldedaten-, Token- und TLS-Datei |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Bundle-TOML und alle für die ausgewählte Operation erforderlichen relativen Blueprint-Dateien |
| `/dev/tty` | interaktive Passworteingabe auf Unix-ähnlichen Systemen |

Es liest weder `~/.pgpass`, `~/.my.cnf`, Cloud-Anmeldedateien, SSH-Schlüssel, Shell-Verläufe noch standardmäßige Passwort-Umgebungsvariablen.

Bei PostgreSQL und MySQL ersetzt ein bereitgestelltes `--tls-ca`-PEM-Bundle die
einkompilierten Mozilla-Stammzertifikate. SQL Server verwendet den Trust Store
des Betriebssystems, wenn `--tls-ca` nicht angegeben ist; eine bereitgestellte
`.pem`- oder `.crt`-Datei muss genau ein CA-Zertifikat enthalten und ersetzt
diese Stammzertifikate. SQL Server prüft den Hostnamen in beiden Modi mit
Zertifikatsprüfung und lehnt `--tls-cert`/`--tls-key` mit `DBP1015E` ab, weil
sein Treiber keine Authentifizierung mit Clientzertifikat implementiert.

## Geschriebene Dateien

Zur Laufzeit kann das Werkzeug Folgendes schreiben:

| Datei | Zeitpunkt/Zweck |
|---|---|
| `--out` | Blueprint-Ausgabe für Live-Datenbank-, strukturierte Datei-, Bundle-Extraktions- oder Bundle-Pack-Modi |
| `--deck` | optionale PowerPoint-Zusammenfassung (.pptx), lokal aus dem anonymisierten Blueprint oder der Eingabe `--from-toml` erzeugt (kein zusätzlicher Datenbankzugriff, kein Netzwerk, keine Drittanbieterbibliothek) |
| `--audit-log` | optionale Kopie des Auditprotokolls |
| `--out-dir` | Batch-Verzeichnis mit `bundle.toml`, `blueprints/*.blueprint.toml`, `audits/*.audit.txt`, einer Eigentumsmarkierung und `errors.txt`, wenn eine oder mehrere Quellen fehlschlagen; bei der atomaren Veröffentlichung wird ein benachbartes Staging-Verzeichnis verwendet und bei einem behandelten Fehler entfernt |

Das Auditprotokoll wird außerdem auf stderr ausgegeben.

Behandeln Sie jedes Auditprotokoll und jede Batch-Datei `errors.txt` als zugriffsgeschützten Betriebsnachweis. Sie können Endpunktnamen, lokale Pfade, Manifest-Quell-IDs, Treiberfehler und Zeitangaben enthalten. Für SQL Server enthält das Audit den exakten authentifizierten Login (`ORIGINAL_LOGIN()`),
den effektiven Server-Principal (`SUSER_SNAME()`) und den Datenbank-Principal
(`USER_NAME()`) sowie optional einen erwarteten Principal und das
Assertion-Ergebnis. Diese Identitäten werden nicht in einen Blueprint aus einer einzelnen Quelle oder eine Präsentation geschrieben. Bundle-Metadaten behalten vom Bediener angegebene Quell-IDs, Tags und Datensatzgruppen-IDs bei; wählen Sie daher anonyme Werte und prüfen Sie das Bundle-TOML vor der Übertragung.

## Umgebungsvariablen

Standardmäßig werden zur Laufzeit keine Umgebungsvariablen für Anmeldedaten gelesen.

Wenn Sie `--password-env NAME`, `--user-env NAME` oder `--azure-token-env NAME` übergeben, liest das Werkzeug genau diese benannte Variable. Es fällt nicht auf übliche Standardwerte wie `PGPASSWORD`, `MYSQL_PWD` oder `MSSQL_PASSWORD` zurück.

## Anmeldedaten

Anmeldedaten werden in einen Typ `Secret` eingeschlossen, der bewusst weder `Debug`, `Display`, `Clone` noch Serialisierung implementiert. Dadurch sind versehentliche Protokollierungen schwer kompilierbar.

Anmeldedaten werden nur zum Verbindungsaufbau an den Datenbanktreiber übergeben. Sie werden weder in die Ausgabedatei noch in das Auditprotokoll geschrieben. Das Auditprotokoll zeichnet die Quelle der Anmeldedaten auf, etwa `file:/etc/dbwarp/db.pass`, nicht deren Wert.

## Abgelehnte Anmeldedatenmuster

In die Verbindungs-URI eingebettete Passwörter werden abgelehnt. Das folgende Beispiel wird nicht akzeptiert:

```text
postgresql://user:password@host/db
```

Verwenden Sie stattdessen `--password-file`, `--password-env` oder die interaktive Eingabeaufforderung. Dadurch werden Passwortlecks über Shell-Verlauf, Prozesslisten oder Terminal-Scrollback vermieden.

## Ausgabesicherheit

Die Blueprint-Datei ist menschenlesbar und prüfbar gestaltet:

- echte Bezeichner werden durch schlüsselgebundene anonyme Namen wie `table-001` und `col-1` ersetzt;
- numerische Werte werden auf dokumentierte Buckets gerundet;
- Kommentare sind fest vorgegeben und werden nicht als Datenkanal verwendet;
- Zeilenwerte werden niemals ausgegeben;
- Komprimierungsstichproben werden, falls aktiviert, lokal komprimiert und verworfen.

Live-Tier-2 wendet eine harte Obergrenze von 16 MiB für die projizierte Nutzlast
je Tabelle an, bevor der Datenbanktreiber Zeilendaten empfängt. Bei extrem
breiten Tabellen wird die angeforderte Zeilenanzahl reduziert; Spalten
variabler Breite werden über Engine-eigene serverseitige Kürzung projiziert.
Stilprüfungen besitzen eigene Grenzen in ihrer SQL-Projektion. Der lokale
Zeilenframe-Encoder erzwingt unabhängig dieselbe Tabellenobergrenze. Dadurch
kann ein kleiner Wert für `--sample-rows` keine unbegrenzte LOB-Nutzlast
übertragen. Sehr große Werte tragen daher nur mit ihren begrenzten Präfixen zu
Komprimierungs- und Längenschätzungen bei.

Die Reihenfolge von Tabellen, Schemas, Indizes und Nicht-Tabellenobjekten
verwendet domänengetrenntes HMAC-SHA256. Standardmäßig bezieht das Werkzeug vom
Betriebssystem einen neuen prozesslokalen Schlüssel und gibt ihn niemals aus.
Dadurch kann ein Offline-Leser mögliche Quellnamen nicht prüfen. Verwenden Sie
`--anonymization-key-file` nur, wenn dieselben anonymen Kennzeichnungen über
genehmigte Vergleichsläufe hinweg erhalten bleiben müssen. Die Datei muss
genau 32 Rohbytes oder 64 Hexadezimalzeichen enthalten und wie ein
Zugangsdatengeheimnis geschützt werden. Das Audit zeichnet nur auf, ob ein
temporärer oder kundenseitig verwahrter Schlüssel verwendet wurde, niemals den
Schlüsselwert.

Dies reduziert das Offenlegungsrisiko, macht jedoch nicht jede Ausgabe für jeden Empfänger sicher. Anonyme Schemastrukturen, Abhängigkeitsgraphen, Engine-Versionen, exakte Opt-in-Felder und ungewöhnliche Größenverteilungen können einen Workload identifizieren. Prüfen Sie Blueprint- und Bundle-Ausgaben vor der Weitergabe gemäß der Datenklassifizierungsrichtlinie Ihrer Organisation. Senden Sie Auditprotokolle oder `errors.txt` nicht so, als wären sie anonymisierte Blueprints.

Die genauen Felder finden Sie in [`FORMAT.md`](FORMAT.md).

## Auditprotokoll

Jeder Lauf erzeugt ein Auditprotokoll, das Folgendes auflistet:

- kontaktierter Datenbankendpunkt;
- verwendete Quelle der Anmeldedaten;
- die von SQL Server gemeldeten authentifizierten, effektiven Server- und
  Datenbank-Principals, wenn die Sitzung sie melden kann;
- TLS-Modus;
- gelesene Dateien;
- geschriebene Dateien;
- ausgeführte Abfragen;
- ob Zeilenstichproben aktiviert waren;
- endgültiges Ergebnis.

Siehe [`AUDIT.md`](AUDIT.md).

## Ausgangspunkte für die Quellcodeprüfung

Für eine gezielte Prüfung:

- `src/secret.rs`: Wrapper für Anmeldedaten;
- `src/main.rs`: CLI, Zustimmungsprüfungen, Auditausgabe;
- `src/audit.rs`: Darstellung des Auditprotokolls;
- `src/format.rs`: anonymisiertes Ausgabeformat;
- `src/tls.rs`: TLS-Konfiguration;
- `src/engine_pg.rs`, `src/engine_mysql.rs`, `src/engine_mssql.rs`: datenbankspezifische Katalogleser.
