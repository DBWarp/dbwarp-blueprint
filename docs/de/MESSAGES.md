# Bedienermeldungscodes

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../MESSAGES.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../MESSAGES.md) | **Deutsch** | [Français](../fr/MESSAGES.md) | [Español](../es/MESSAGES.md) | [Polski](../pl/MESSAGES.md) | [日本語](../ja/MESSAGES.md) | [中文](../zh/MESSAGES.md)

`dbwarp-blueprint` verwendet stabile Bezeichner für Bedienermeldungen bei DBWarp-eigenen Validierungs- und Arbeitsablauffehlern. Das Format ist von IBM-artigen Bedienermeldungen inspiriert: ein Subsystempräfix, eine numerische Kennung und ein Schweregradsuffix. Die IBM-CICS-Dokumentation beschreibt eine Programmkennung zusammen mit einer vierstelligen Meldungsnummer und einem Schweregradbuchstaben; IBM MQ verwendet ähnlich Komponenten-/Präfixfelder, eine numerische Kennung und einen abschließenden Meldungstypcode. Microsofts Richtlinien für Fehlermeldungen bekräftigen die praktische Regel, dass ein Fehler das Problem beschreiben und eine vom Benutzer ausführbare Maßnahme angeben sollte.

Referenzen:

- IBM-CICS-Meldungsformat: https://www.ibm.com/docs/en/cics-pa/5.3.0?topic=messages-message-format
- Aufbau der IBM-CICS-Meldungsinformationen: https://www.ibm.com/docs/en/cics-ts/6.x?topic=messages-format-cics-message-information
- IBM-MQ-for-z/OS-Meldungsformat: https://www.ibm.com/docs/SSFKSJ_9.2.0/com.ibm.mq.ref.doc/q050270_.htm
- Microsoft-Richtlinien für Fehlermeldungen: https://learn.microsoft.com/en-us/windows/win32/uxguide/mess-error

## Format

```text
DBPnnnnS message text. Next: corrective action.
```

Felder:

- `DBP` bedeutet DBWarp Blueprint.
- `nnnn` ist eine stabile vierstellige Meldungsnummer.
- `S` ist der Schweregrad: `E` Fehler, `W` Warnung, `I` Information.

Der Code ist stabil und sprachneutral. Seine Zusammenfassung, Ursache und Korrekturmaßnahme werden lokalisiert, wenn `--lang` oder das Prozessgebietsschema eine unterstützte Sprache auswählt. Dynamische Details zu Betriebssystem, Datenbanktreiber, Pfad und Ursachenkette bleiben unverändert, damit Supporttechniker nach dem ursprünglichen Fehler suchen können. Meldungstext darf keine Geheimnisse oder unbereinigten Verbindungs-URIs enthalten.

## Bereiche

| Bereich | Gebiet |
|---|---|
| `DBP0001E` | Tatsächlich nicht klassifizierter umschlossener Fehler mit Ursachenkette |
| `DBP10xxE` | Validierung von Befehl, Verbindungseingabe und Erfassungsrichtlinie |
| `DBP11xxE` | Validierung von Batch-Manifest und Quelleneingabe |
| `DBP12xxE` | Bundle-Selektoren und Blueprint-URI-Selektoren |
| `DBP13xxE` | Offline-TOML-/Präsentations-/Schemavalidierung |
| `DBP14xxE/W` | Fehler bei der Live-Datenbankerfassung und nicht schwerwiegende Verschlechterung der Stichprobe |
| `DBP15xxE/W` | Ausgabe für strukturierte Dateien, Blueprint, Präsentation und Audit |
| `DBP16xxE/W` | Richtlinie für Anmeldedaten, Authentifizierung, TLS und vertrauliche Dateien |
| `DBP17xxE` | Bedienerzustimmung |
| `DBP18xxE` | Initialisierung der Prozesslaufzeit |

## Aktuelle Codes

| Code | Bedeutung |
|---|---|
| `DBP0001E` | Nicht klassifizierter Fehler; die Ursachenkette folgt. |
| `DBP1000E` | `--connect` fehlt außerhalb der Offline-Modi. |
| `DBP1001E` | In URI eingebettetes Passwort abgelehnt. |
| `DBP1002E` | Nicht unterstütztes URI-Schema für `--connect`. |
| `DBP1003E` | Nicht unterstützte Überschreibung des TLS-Servernamens. |
| `DBP1004E` | Azure-Token-Option mit einer anderen Engine als SQL Server verwendet. |
| `DBP1005E` | Der Authentifizierungsmodus ist für die ausgewählte Engine nicht verfügbar. |
| `DBP1006E` | Komprimierungsstichprobe für strukturierte Dateien ohne explizites `--yes` angefordert. |
| `DBP1007E` | Expliziter Längentreuemodus für eine Engine angefordert, die diesen Vertrag noch nicht bereitstellt. |
| `DBP1008E` | Legacy-Alias für exakte Länge steht in Konflikt mit strenger Längentreue. |
| `DBP1009E` | Exakte Treue der Stichprobenlänge ohne explizites `--yes` angefordert. |
| `DBP1010E` | Eingebetteter Lokalisierungskatalog ist unvollständig oder inkonsistent. |
| `DBP1011E` | Befehlszeilenargumente sind ungültig. |
| `DBP1012E` | Eine unterstützte Datenbankverbindungs-URI ist fehlerhaft. |
| `DBP1013E` | `--source-kind` ist leer oder wird nicht unterstützt. |
| `DBP1014E` | Anonymer Artefaktgraph oder Definitionsanalyse ohne ausdrückliche Zustimmung angefordert. |
| `DBP1015E` | TLS-Clientzertifikat-Optionen mit SQL Server verwendet, dessen Treiber sie nicht implementiert. |
| `DBP1101E` | Batch-Manifest kann nicht gelesen werden. |
| `DBP1102E` | Batch-Manifest kann nicht geparst werden. |
| `DBP1103E` | Batch-Manifest enthält keine `[[source]]`-Einträge. |
| `DBP1104E` | Batch-Modus benötigt ein explizites `--yes`. |
| `DBP1105E` | Eine Quelle innerhalb eines Batches ist fehlgeschlagen. |
| `DBP1106E` | Nicht unterstützte Art der Batch-Quelle. |
| `DBP1107E` | Dateiquelle hat keine Eingabedateien aufgelöst. |
| `DBP1108E` | Nicht unterstützter Dateidatensatzmodus. |
| `DBP1109E` | Bezeichner der Batch-Quelle enthält keinen verwendbaren ASCII-Buchstaben oder keine Ziffer. |
| `DBP1110E` | Datenbankquelle besitzt die falsche Anzahl von Verbindungsquellen. |
| `DBP1111E` | Variable `connect_env` fehlt oder kann nicht gelesen werden. |
| `DBP1112E` | `connect_file` fehlt oder kann nicht gelesen werden. |
| `DBP1113E` | Batch-Ausgabe, Audit, Bericht oder Verzeichnis konnte nicht abgeschlossen werden. |
| `DBP1114E` | Bestandteile des strukturierten Dateidatensatzes sind inkompatibel. |
| `DBP1115E` | Alle Batch-Quellen sind fehlgeschlagen; nur Diagnoseausgaben wurden veröffentlicht. |
| `DBP1116E` | Ein unvollständiges Batch-Bundle wurde veröffentlicht. |
| `DBP1200E` | Ungültige Selektor- oder `blueprint://`-Syntax. |
| `DBP1201E` | Bundle-Selektor stimmte mit keiner Quelle überein. |
| `DBP1202E` | Bundle-Selektor stimmte mit mehreren Quellen überein. |
| `DBP1203E` | Bundle-Selektor stimmte mit keinem extrahierbaren Blueprint und keiner Tabelle überein. |
| `DBP1204E` | Bundle-Eingabe konnte nicht gelesen werden. |
| `DBP1205E` | Bundle- oder referenzierter Blueprint-Inhalt ist ungültig. |
| `DBP1206E` | Bundle-Ausgabe konnte nicht geschrieben werden. |
| `DBP1301E` | Bei `--from-toml` fehlt `--deck`. |
| `DBP1302E` | Nicht unterstützte Schemaversion des Blueprint-TOML. |
| `DBP1401E` | PostgreSQL-Erfassungsgrenze ist fehlgeschlagen. |
| `DBP1402E` | MySQL- oder MariaDB-Erfassungsgrenze ist fehlgeschlagen. |
| `DBP1403E` | SQL-Server-Erfassungsgrenze ist fehlgeschlagen. |
| `DBP1404W` | PostgreSQL-TLS-Modus `prefer` ist auf Loopback auf Klartext zurückgefallen. |
| `DBP1405W` | Optionale Datenbank-RTT-Prüfung war nicht verfügbar. |
| `DBP1406W` | Zeitbudget der Tier-2-Stichprobe war erschöpft. |
| `DBP1407W` | Eine Komprimierungsstichprobe war nicht verfügbar. |
| `DBP1408W` | Eine Textspalten-Stilstichprobe war nicht verfügbar. |
| `DBP1409W` | Die asynchrone PostgreSQL-Verbindungsaufgabe meldete einen Fehler. |
| `DBP1410W` | Ein optionaler Artefaktkatalog war nicht verfügbar; die Vollständigkeit wird daher ausdrücklich reduziert. |
| `DBP1411W` | Topologienachweise sind nicht verfügbar; Deployment und lokale Rolle bleiben unbekannt. |
| `DBP1412W` | Ein verteiltes oder geshardetes Layout wurde erkannt, aber vollständige Aggregatgrößen waren nicht verfügbar. |
| `DBP1413W` | Tabellen-, Zeilen- oder Byteabdeckung des Datensatzes ist unvollständig oder unbekannt. |
| `DBP1414W` | Die Beziehung der Bundle-Quelle ist unbekannt; quellenübergreifende Berechnungen sind unsicher. |
| `DBP1415W` | Deklarierte Replikate stimmen nicht überein; ein deterministischer Vertreter wurde ohne Mittelwertbildung beibehalten. |
| `DBP1416W` | Eine Shard-Gruppe ist unvollständig und trägt keine Aggregatsummen bei. |
| `DBP1417W` | Bundle-Aggregatsummen wurden unterdrückt. |
| `DBP1418W` | Eine in die Bundle-Berechnung einbezogene Quelle hat unvollständige oder unbekannte Datensatzabdeckung. |
| `DBP1419E` | Die Live-Erfassung hat `--max-wall-secs` überschritten; der Client trennte die Verbindung und meldet die Engine-spezifische Servergrenze. |
| `DBP1420E` | Mindestens ein angefordertes `--schema` war nicht sichtbar; daher wurde keine Blueprint mit mehrdeutigem Umfang geschrieben. |
| `DBP1421W` | SQL-Server-Sitzungsidentitäten waren nicht verfügbar; die Erfassung wurde ohne Identitätsaussage fortgesetzt. |
| `DBP1501E` | Erfassungsgrenze für strukturierte Dateien ist fehlgeschlagen. |
| `DBP1502E` | Blueprint- oder Bundle-Ausgabe ist fehlgeschlagen. |
| `DBP1503E` | Erzeugung der PowerPoint-Präsentation ist fehlgeschlagen. |
| `DBP1504W` | Auditprotokoll konnte nicht geschrieben werden. |
| `DBP1601E` | Erfassung der Anmeldedaten ist fehlgeschlagen. |
| `DBP1602E` | TLS-Konfiguration ist fehlgeschlagen. |
| `DBP1603E` | Erfassung des Datenbankbenutzernamens ist fehlgeschlagen. |
| `DBP1604E` | Die Datenbank-Authentifizierungskonfiguration ist ungültig. |
| `DBP1605W` | Durchsetzung von Berechtigungen für vertrauliche Dateien ist auf dieser Plattform nicht verfügbar. |
| `DBP1606E` | Die Assertion für den authentifizierten SQL-Server-Principal ist vor der Katalogerfassung fehlgeschlagen. |
| `DBP1607E` | Der HMAC-Schlüssel für die Anonymisierung konnte nicht sicher initialisiert werden. |
| `DBP1701E` | Vorgang wurde vor der expliziten Zustimmung abgebrochen. |
| `DBP1702E` | Zustimmungsantwort konnte nicht aus der Standardeingabe gelesen werden. |
| `DBP1801E` | Asynchrone Laufzeit konnte nicht initialisiert werden. |

Jede angegebene Sprache muss für jeden aktuellen DBP-Code Zusammenfassung, Ursache und Maßnahme enthalten. Die Binärdatei validiert dies beim Start und schlägt mit `DBP1010E` fehl, statt stillschweigend auf Englisch zurückzufallen.

Vorhersehbare Fehler an Entscheidungsgrenzen werden durch eine adversariale CLI-Matrix geprüft. Eine bekannte Bedingung muss ihren spezifischen Code als ersten Bedienercode ausgeben und darf nicht auf `DBP0001E` zurückfallen. Der Renderer durchsucht außerdem die vollständige Fehlerkette, sodass ein nicht codierter Implementierungskontext eine codierte innere Ursache nicht verbergen kann.

Nicht schwerwiegende Warnungen bei Datenbankstichproben werden mit ihrem stabilen Warncode ausgegeben und im Lauf-Audit aufgezeichnet. Dadurch wird eine vollständige Tier-2-Erfassung von einer erfolgreichen Erfassung mit nur teilweise erhobenen Stichproben unterschieden, ohne den Fehler einer optionalen Prüfung zu einem vollständigen Erfassungsfehler zu machen.

## Support-Prüfliste

Wenn ein Kunde einen Fehler meldet, fordern Sie Folgendes an:

- die vollständige Terminalausgabe einschließlich des `DBP`-Codes;
- das Auditprotokoll, falls `--audit-log` verwendet wurde;
- die bereinigte Befehlszeile;
- bei Bundle-Fehlern die Ausgabe von `dbwarp-blueprint --bundle-list ...`.

Fordern Sie keine Passwortdateien, Tokendateien, privaten Schlüssel oder rohen Datenbank-Zeilenstichproben an.
