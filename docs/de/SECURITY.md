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
| Live-Datenbankverbindung `--connect` für PostgreSQL, MySQL oder SQL Server | Eine Datenbanktreibersitzung zum angegebenen Datenbankendpunkt. Die DNS-Auflösung kann den konfigurierten Resolver kontaktieren. Die integrierte Kerberos-/SSPI-Authentifizierung kann außerdem konfigurierte Identitätsinfrastruktur wie einen KDC oder Domänencontroller kontaktieren. |
| Bestätigungspflichtige Oracle-Vorschau | Startet nur die vom Operator ausgewählte Programmdatei `--oracle-sqlplus` als Kindprozess (einschließlich einer begrenzten `-V`-Abfrage, sofern verfügbar) und verwendet sie für die Katalogsitzung. Die Anmeldeinformationen werden über die Standardeingabe gesendet und niemals in Prozessargumenten abgelegt. Der Kindprozess erhält ein leeres privates Verzeichnis als `TNS_ADMIN`; seine Umgebung wird geleert und nur die vorhandenen Variablen `PATH`, `SystemRoot`, `WINDIR`, `ORACLE_HOME`, `LD_LIBRARY_PATH`, `DYLD_LIBRARY_PATH`, `LIBPATH` und `SHLIB_PATH` werden weitergereicht. Feste Locale-, Zeitzonen- und private `TNS_ADMIN`-Werte werden separat gesetzt. |
| `--batch-manifest` | Eine Datenbanktreibersitzung für jede Datenbankquelle im Manifest, die sequenziell verarbeitet wird. Lokale Parquet- und Avro-Quellen verwenden kein Netzwerk. Die obigen DNS- und integrierten Authentifizierungsbedingungen gelten weiterhin. |
| `--from-toml`, `--from-parquet`, `--from-avro`, `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Keine von der Anwendung initiierte Netzwerkverbindung. Eingaben auf Netzwerk-Dateisystemen bleiben eine Angelegenheit des Betriebssystems beziehungsweise Speichersystems. |

Das Werkzeug ruft weder einen DBWarp-Dienst noch eine Cloud-API auf. Datenbanktreiber und Host-Betriebssystem können den oben beschriebenen Protokollhilfsverkehr erzeugen und System-Trust-Stores, DNS-Konfiguration, dynamische Bibliotheken sowie Konfiguration oder Anmeldedatencaches der integrierten Authentifizierung konsultieren.

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

Zur Laufzeit liest die Anwendung direkt nur Eingaben, die auf der Befehlszeile ausgewählt oder von einer Batch-/Bundle-Eingabe referenziert werden:

| Datei | Zeitpunkt/Zweck |
|---|---|
| `--user-file` | Quelle des Benutzernamens |
| `--password-file` | Quelle des Passworts |
| `--anonymization-key-file` | Optionaler HMAC-Schlüssel, den Sie besitzen und der vom binären oder SQL-Fallback-Normalisierer verwendet wird, um anonyme Objektbezeichnungen über genehmigte Ausführungen hinweg beizubehalten; der Modus muss group/other verhindern, dass auf Unix-Systemen gelesen wird. |
| `--azure-token-file` | Quelle des SQL-Server-Entra-ID-Tokens |
| `--tls-ca` | vertrauenswürdiges CA-Bundle |
| `--tls-cert` | TLS-Clientzertifikat |
| `--tls-key` | privater TLS-Clientschlüssel |
| `--from-toml` | vorhandene dbwarp-blueprint-TOML-Datei zur Offline-Erstellung einer Präsentation |
| `--from-parquet` | Parquet-Metadaten und, bei ausdrücklicher Zustimmung zur Stichprobe, begrenzte dekodierte Zeilen |
| `--from-avro` | Metadaten und Datensätze eines Avro-Objektcontainers; zum Zählen der Datensätze muss der Container durchlaufen werden |
| `--batch-manifest` | Batch-Manifest sowie jede darin referenzierte lokale strukturierte Datei, Anmeldedaten-, Token- und TLS-Datei |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Bundle-TOML und alle für die ausgewählte Operation erforderlichen relativen Blueprint-Dateien |
| steuerndes Terminal oder Konsole | interaktive Passworteingabe (`/dev/tty` auf Unix-ähnlichen Systemen) |

Die Anwendung besitzt keinen ausdrücklichen Fallback zum Lesen von `~/.pgpass`, `~/.my.cnf`, Cloud-Anmeldedateien, SSH-Schlüsseln, Shell-Verläufen oder standardmäßigen Datenbankpasswort-Umgebungsvariablen. Plattformbibliotheken für Datenbank, TLS, DNS und Identität können weiterhin eigene Systemkonfiguration und Anmeldedatencaches konsultieren; verwenden Sie eine Betriebssystemablaufverfolgung, wenn Ihre Richtlinie ein vollständiges Prozess- und Bibliotheksinventar verlangt.

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

## Treibereigene Kopien von Anmeldedaten

Die Nullung deckt den von DBWarp Blueprint verwalteten `Secret`-Puffer ab; sie kann die Löschung von Kopien nicht garantieren, die ein Datenbanktreiber, eine TLS-Bibliothek, ein Betriebssystem-Authentifizierungsanbieter oder der Allocator erstellt. Die aktuelle MySQL-Treiber-API verlangt einen eigenen `String`, daher kopiert `src/engine_mysql.rs` das Passwort ausdrücklich aus `Secret` in `OptsBuilder`. Diese Kopie wird nicht genullt und bleibt erhalten, bis Builder/Optionen verworfen werden. PostgreSQL, SQL Server und Plattform-Authentifizierungsbibliotheken können ebenfalls interne Kopien außerhalb des Wrappers erstellen. Schränken Sie Prozessinspektion und Swap auf Hosts mit sensiblen Anmeldedaten angemessen ein.

## Abgelehnte Anmeldedatenmuster

In die Verbindungs-URI eingebettete Passwörter werden abgelehnt. Das folgende Beispiel wird nicht akzeptiert:

```text
postgresql://user:password@host/db
```

Verwenden Sie stattdessen `--password-file`, `--password-env` oder die interaktive Eingabeaufforderung. Dadurch werden Passwortlecks über Shell-Verlauf, Prozesslisten oder Terminal-Scrollback vermieden.

## Ausgabesicherheit

Die Blueprint-Datei ist menschenlesbar und prüfbar gestaltet:

- echte Bezeichner werden durch schlüsselgebundene anonyme Namen wie `table-001` und `col-1` ersetzt;
- numerische Werte verwenden die für jedes Feld dokumentierte exakte oder gerundete Genauigkeit; exakte Längenmodi erfordern ausdrückliche Zustimmung;
- Kommentare sind fest vorgegeben und werden nicht als Datenkanal verwendet;
- Zeilenwerte werden niemals ausgegeben;
- Komprimierungsstichproben werden, falls aktiviert, lokal komprimiert und verworfen.

Live-Tier-2 budgetiert höchstens 16 MiB projizierte Stichprobenwert-Nutzlast je
Tabelle. Bei extrem breiten Tabellen wird die angeforderte Zeilenanzahl
reduziert; Zellen variabler Breite werden über Engine-eigene serverseitige
Kürzung projiziert, auch bei adaptiven Wiederholungen für MySQL und SQL Server.
Stilprüfungen besitzen eigene Grenzen in ihrer SQL-Projektion. Der lokale
Prüfpuffer-Encoder erzwingt seine Tabellenobergrenze unabhängig davon.

Dies ist keine Obergrenze von 16 MiB für Netzwerkverkehr oder Prozessspeicher:
Protokollrahmen, getrennt zurückgegebene Metadaten der ursprünglichen Länge,
die hexadezimale Binärdarstellung von PostgreSQL, Wiederholungen und
Treiberpuffer verursachen zusätzlichen Aufwand. Sehr große Werte können nur
mit begrenzten Präfixen zu Komprimierungs- und Wertzusammenfassungsmessungen
beitragen. Der Collector ermittelt die ursprünglichen Längen der
Stichprobenwerte getrennt auf dem Server und wendet die ausgewählte
Längentreuerichtlinie auf diese Längen an, nicht nur auf die Längen der
zurückgegebenen Präfixe. Die Stichprobenherkunft dokumentiert die geltenden
Einschränkungen; eine begrenzte Ausgabe belegt keine Messung vollständiger Werte.

Tabelle, Schema, Index und die Reihenfolge von Nicht-Tabellen-Objekten verwenden eine durch Domänen getrennte HMAC-SHA256-Verschlüsselung. Standardmäßig erhält das Tool einen neuen, prozesslokalen Schlüssel vom Betriebssystem und gibt diesen niemals aus, wodurch verhindert wird, dass ein Offline-Reader potenzielle Quellnamen überprüfen kann. Verwenden Sie `--anonymization-key-file` nur, wenn dieselben anonymen Bezeichnungen über genehmigte Vergleichsläufe hinweg erhalten bleiben müssen. Die Datei muss genau 32 rohe Bytes oder 64 hexadezimale Zeichen enthalten und muss wie ein Anmeldeinformation geschützt werden. Die Prüfprotokolle zeigen, ob ein temporärer Schlüssel oder ein von Ihnen bereitgestellter Schlüssel verwendet wurde, niemals den Schlüsselwert selbst.

Der nur die Standardbibliothek verwendende Normalisierer `blueprint_format.py` des SQL-Fallbacks besitzt denselben Vertrag für schlüsselgebundene Reihenfolge. Er bezieht standardmäßig einen neuen zufälligen Betriebssystemschlüssel, sofern `--anonymization-key-file` nicht angegeben ist, und fügt nach dem kanonischen Header einen festen Kommentar mit Producer/Schlüsselquelle hinzu, damit seine Ausgabe nicht mit der des Rust-Collectors verwechselt werden kann.

Vor der Normalisierung enthält das Zwischen-JSON des SQL-Fallbacks echte Schema-, Tabellen-, Spalten- und Indexnamen. MySQL `COLUMN_TYPE` kann außerdem deklarierte enum/set-Mitglieder enthalten. Die SQL-Skripte besitzen keinen Selektor für eine Teilmenge von Schemata und erzeugen kein Anwendungsaudit. Behalten Sie das Zwischen-JSON als sensibles Schemamaterial in der Quellumgebung, normalisieren Sie es lokal und übertragen Sie nur die geprüfte TOML-Datei. Verwenden Sie den Rust-Collector, wenn nur ausgewählte Schemata genehmigt sind oder vollständige Audit-, Topologie-, Artefakt- oder Stichprobenevidenz erforderlich ist.

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
