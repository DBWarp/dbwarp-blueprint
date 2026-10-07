# Bedienermeldungscodes

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../MESSAGES.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../MESSAGES.md) | **Deutsch** | [Français](../fr/MESSAGES.md) | [Español](../es/MESSAGES.md) | [Polski](../pl/MESSAGES.md) | [日本語](../ja/MESSAGES.md) | [中文](../zh/MESSAGES.md)

`dbwarp-blueprint` verwendet stabile Operator-Nachrichten-IDs für DBWarp-eigene Validierungs- und Workflow-Fehler. Jede Nachricht hat ein Subsystem-Präfix, eine numerische ID und ein Schweregrad-Suffix und beschreibt das Problem sowie eine korrigierende Maßnahme.

## Format

```text
DBPnnnnS message text. Next: corrective action.
```

Felder:

- `DBP` bedeutet DBWarp Blueprint.
- `nnnn` ist eine stabile vierstellige Meldungsnummer.
- `S` ist der Schweregrad: `E` Fehler, `W` Warnung, `I` Information.

Der Code ist stabil und sprachneutral. Seine Zusammenfassung, Ursache und die vorgeschlagene Maßnahme werden lokalisiert, wenn `--lang` oder die Prozesssprache eine unterstützte Sprache auswählt. Dynamische Details des Betriebssystems, des Datenbanktreibers, des Pfads und der Kausalkette bleiben unverändert, damit der ursprüngliche Fehler gefunden werden kann. Der Text der Meldung darf keine Geheimnisse oder unbearbeiteten Verbindungs-URIs enthalten.

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
| `DBP1007E` | Es wurde ein expliziter Modus für die Längenprüfung angefordert, der von der verwendeten Engine nicht unterstützt wird. |
| `DBP1008E` | `--preserve-exact-lengths` steht im Konflikt mit der strikten Einhaltung der Länge. |
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
| `DBP1402E` | MySQL-Erfassungsgrenze ist fehlgeschlagen. |
| `DBP1403E` | SQL-Server-Erfassungsgrenze ist fehlgeschlagen. |
| `DBP1404W` | PostgreSQL-TLS-Modus `prefer` ist auf Loopback auf Klartext zurückgefallen. |
| `DBP1405W` | Optionale Datenbank-RTT-Prüfung war nicht verfügbar. |
| `DBP1406W` | Zeitbudget der Tier-2-Stichprobe war erschöpft. |
| `DBP1407W` | Eine Kompressionsstichprobe war unvollständig oder nicht verfügbar; verwendbare Zeilen aus einer Teilstichprobe wurden möglicherweise beibehalten. |
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
| `DBP1422W` | Die Bewertung der Artefaktkomplexität ist fehlgeschlagen; das Inventar blieb erhalten und die betroffenen aggregierten Dimensionen sind unbekannt. |
| `DBP1423W` | Ein Strukturkatalog für Indizes oder Beziehungen war nicht verfügbar; zentrale Tabellen und Spalten blieben mit ausdrücklich unvollständiger Abdeckung erhalten. |
| `DBP1424W` | Die vollständige Sichtbarkeit des SQL-Server-Katalogs für Sicherheitsrichtlinien konnte nicht nachgewiesen werden; für jede betroffene Tabelle wurde die Tier-2-Stichprobe übersprungen. |
| `DBP1425W` | SQL Server meldete einen aktiven Zeilensicherheitsfilter; die Tier-2-Stichprobe wurde bewusst übersprungen, statt eine gefilterte Teilmenge zu messen. |
| `DBP1426E` | Die grundlegende Erfassung von Oracle ist während der Konfiguration, beim Start von SQL*Plus, bei der Auflösung des Besitzers, der Erfassung oder der Zuordnung fehlgeschlagen. |
| `DBP1427W` | Die Herkunft der Oracle-SQL*Plus-Clientversion konnte nicht vollständig bestätigt werden; die Erfassung wurde mit einer ausdrücklichen Einschränkung fortgesetzt. |
| `DBP1428W` | Oracle Basic wurde gestoppt, bevor jede der geplanten Katalogabfragen abgeschlossen war; die bereits gelesenen Tabellen, Spalten, Zeilen und Größen wurden beibehalten, und das Blueprint wurde als unvollständig markiert. |
| `DBP1429W` | Oracle Basic behielt die grundlegenden Daten zu Tabellen, Spalten, Zeilen und Größe bei, während eine oder mehrere zusätzliche Katalogabfragen nicht verfügbar waren. |
| `DBP1430W` | Oracle Basic hat sein Blueprint veröffentlicht, konnte aber die optionale Offline-Stream-Datei nicht schreiben. |
| `DBP1501E` | Erfassungsgrenze für strukturierte Dateien ist fehlgeschlagen. |
| `DBP1502E` | Blueprint- oder Bundle-Ausgabe ist fehlgeschlagen. |
| `DBP1503E` | Erzeugung der PowerPoint-Präsentation ist fehlgeschlagen. |
| `DBP1504W` | Auditprotokoll konnte nicht geschrieben werden. |
| `DBP1505E` | Der Oracle Basic Offline-Stream ist bei der Validierung von Dateiberechtigungen, Größenbeschränkungen, Prüfsummen, Abfragesätzen oder der Katalogstruktur fehlgeschlagen. |
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

Jede unterstützte Sprache enthält jede DBWarp-Zusammenfassung, Ursache und Maßnahme. Das Programm prüft dies beim Start und schlägt mit `DBP1010E` fehl, anstatt stillschweigend auf Englisch umzuschalten.

Nicht schwerwiegende Warnungen bei Datenbankstichproben werden mit ihrem stabilen Warncode ausgegeben und im Lauf-Audit aufgezeichnet. Dadurch wird eine vollständige Tier-2-Erfassung von einer erfolgreichen Erfassung mit nur teilweise erhobenen Stichproben unterschieden, ohne den Fehler einer optionalen Prüfung zu einem vollständigen Erfassungsfehler zu machen.

## Support-Prüfliste

Wenn Sie Unterstützung für einen Fehler anfordern, stellen Sie Folgendes bereit:

- die vollständige Terminalausgabe einschließlich des `DBP`-Codes;
- das Auditprotokoll, falls `--audit-log` verwendet wurde;
- die bereinigte Befehlszeile;
- bei Bundle-Fehlern die Ausgabe von `dbwarp-blueprint --bundle-list ...`.

Fordern Sie keine Passwortdateien, Tokendateien, privaten Schlüssel oder rohen Datenbank-Zeilenstichproben an.
