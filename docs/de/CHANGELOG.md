# Änderungsprotokoll

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../CHANGELOG.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../CHANGELOG.md) | **Deutsch** | [Français](../fr/CHANGELOG.md) | [Español](../es/CHANGELOG.md) | [Polski](../pl/CHANGELOG.md) | [日本語](../ja/CHANGELOG.md) | [中文](../zh/CHANGELOG.md)

Release-Versionen bezeichnen den Collector. Die Blueprint-Schemaversion und
die Kodierung der Komprimierungsstichproben sind getrennte
Kompatibilitätsverträge; siehe [FORMAT.md](FORMAT.md) und
[Komprimierungsmessung](COMPRESSION_MEASUREMENT.md).

## 1.6.0

### Blueprint-Schema v7

- Gibt Schema v7 mit ausdrücklicher Tabellenklassifizierung,
  Speicherorganisation, Partitionierung, Segmentzustand, Spaltenordnungen
  sowie reichhaltigeren Typ-, NULL-, generierten Wert- und Identitätssemantiken
  aus.
- Fügt je Tabelle und für die gesamte Erfassung Nachweise zu Struktur,
  Zeilenzahlen, zugeordneten Bytes, Statistiken und Beobachtungen der
  Quellumgebung hinzu. Fehlende, verweigerte, fehlerhafte oder durch die
  Auswahl begrenzte Nachweise bleiben ausdrücklich sichtbar, statt als
  gemessene Null oder vollständiges Inventar dargestellt zu werden.
- Fügt dem Artefaktinventar kataloggestützte Anforderungszustände je Objekt und
  begrenzte Komplexitätsnachweise hinzu. Die Gesamtvollständigkeit bleibt
  falsch, wenn ein erforderlicher Katalog oder die gewählte Prüfpopulation
  nicht nachweislich vollständig ist.
- Beibehalten der Kompatibilität mit den Eingabeformaten schema-v1 bis schema-v6. Ältere Versionen können möglicherweise keine Dateien im Format schema-v7 lesen.
### Erfassungstreue und Sicherheit

- Hält vollständige begrenzte Lesevorgänge, Teilstichproben und
  Katalogschätzungen für PostgreSQL, MySQL und SQL Server auseinander,
  einschließlich Zeilensicherheit, Vererbungsgrenzen, adaptiver MySQL-
  Bereichsfenster und speicheroptimierter SQL-Server-Tabellen.
- Verschärft die Konsistenz von Kardinalität, NULL-Anzahlen, Partitionen,
  Beziehungen und Aggregaten, damit gerundete oder unvollständige Nachweise
  nicht zu exakten Aussagen werden.
- Meldet die Abdeckung externer SQL-Server-Tabellen als ausdrückliche
  Einschränkung, wenn PolyBase nicht installiert ist.
- Begrenzt die Erfassung nach Zeilen, Bytes, Werten und Fristen. Nicht fatale
  Herabstufungen bleiben im Blueprint und Audit mit stabilen Meldungscodes
  sichtbar.

### Oracle-Geltungsbereich

- Diese Version fügt die Katalogerfassung für Oracle Basic für Oracle 12c, 19c,
  21c und 23ai/26ai als Vorschau hinzu: Die Erfassung erfolgt nur für den
  Katalog, wobei keine Tabellenzeilen gelesen werden und eine Bestätigung
  erforderlich ist. Weitere Informationen zu den Einschränkungen finden Sie
  unter `sql/grants/ORACLE_PREVIEW.md`.

### Release-Artefakte und Authentifizierung

- Linux-Release-Archive enthalten die Kerberos/GSSAPI-Authentifizierung für SQL
  Server. Sie laden die Kerberos-Laufzeitbibliothek der Plattform erst, wenn
  integrierte Authentifizierung ausgewählt wird. Der Collector startet daher
  ohne Kerberos-Bibliotheken; eine fehlende Laufzeit wird nur bei integrierter
  Authentifizierung mit `DBP1604E` gemeldet. Windows-Release-Binärdateien
  enthalten weiterhin die SQL-Server-SSPI-Authentifizierung.
- Beide integrierten Verfahren verwenden die Anmeldeinformationen des
  Betriebssystems und verweigern die Verbindung, wenn der Serverprinzipal nicht
  dem erwarteten entspricht.

### Betrieb und Kompatibilität

- Die Enhanced-Berechtigungsskripte für SQL Server fügen in einem separaten
  Batch eine Berechtigung auf Serverebene hinzu: `VIEW SERVER STATE` für SQL
  Server 2019 und `VIEW SERVER PERFORMANCE STATE` für 2022 und 2025. Damit kann
  eine Enhanced-Erfassung mit `--artifact-detail graph` oder `analyzed` grobe
  CPU- und Speicherbereiche für einen selbstverwalteten Server melden. Basic
  und Standard gewähren diese Berechtigung nicht und melden diese Bereiche als
  unbekannt; ein DBA kann den Batch entfernen, um Enhanced ohne sie
  beizubehalten.
- Aktualisiert den Abhängigkeitsstapel für SQL Server, PostgreSQL, Parquet und
  die Windows-Authentifizierung auf Versionen, die veröffentlichte
  Sicherheitshinweise beheben. Der SQL-Server-Treiber wechselt zu 0.13;
  `--tls-ca` behält seine restriktive Bedeutung und vertraut ausschließlich der
  bereitgestellten CA. `--max-wall-secs` bleibt die einzige Frist für die
  gesamte Erfassung.
- SQL Server liest Betriebssystemkapazität nur, wenn die Analyse von
  Nicht-Tabellen-Objekten angefordert wird (`--artifact-detail graph` oder
  `analyzed`). Andere Erfassungen zeichnen die Kapazitätsbereiche als nicht
  angefordert statt als fehlgeschlagenen Lesevorgang auf.
- Die englische Dokumentation bleibt maßgeblich. Übersetztes Markdown ist
ergänzend und enthält eine eigene Übersetzungshinweis.

### Präsentation

- Fügt der Präsentation eine Folie für Nicht-Tabellen-Objekte und eine separate
  Folie zur Artefaktkomplexität hinzu. Die Komplexitätsfolie erscheint nur,
  wenn Komplexität erfasst wurde, und zeigt die Abdeckung neben jedem Bereich,
  sodass unvollständige Nachweise nie als geringe Komplexität dargestellt
  werden.

## 1.5.1

### Erfassung und Datentreue

- Blueprint-Schema v6 bleibt erhalten; Katalogschätzungen, Beobachtungen aus
  Stichproben und nicht verfügbare Nachweise zur Aktualität von Statistiken
  werden unterschieden.
- Verbesserte Messung binärer Nutzlasten, Profilierung bereits komprimierter
begrenzte Kompressionsprüfungen. Verhältnisse aus unterschiedlichen `sample_encoding`-Werten sind nicht austauschbar; überprüfen Sie die Kodierung, bevor Sie Verhältnisse vergleichen.
- Begrenzte adaptive Wiederholungen der MySQL- und SQL-Server-Stichproben,
  einschließlich übergroßer Werte und Zeichensatzerweiterung. Verbleibende
  Präfixverzerrung wird dokumentiert, während die Metadaten zu den
  ursprünglichen Längen der Stichprobenwerte erhalten bleiben.
- Korrigierte Erkennung von MySQL-Kürzungen, wenn der Verbindungszeichensatz
  die zurückgegebene Bytelänge verändert.

### Betrieb und Prüfung

- Präzisierte Einrichtung dedizierter Konten mit minimalen Berechtigungen,
Vergleichs-Build-Verifizierung und die unterstützten Datenbankversionen.
- Aktualisierte maschinell übersetzte Dokumentation und Laufzeitformulierungen;
  Englisch bleibt maßgeblich, Übersetzungen bleiben ergänzend.
- Versionshistorie sowie Support- und Beitragsrichtlinien werden mit Quellen
  und Binärdateien ausgeliefert.
### Kompatibilität.

Die bestehenden Blueprints bleiben für den neuen Collector lesbar. Das Gegenteil ist nicht garantiert: die Version 1.5.0 des Readers lehnt das neue optionale Feld `sample_layout` ab, und ältere Versionen können die neuen Kompressions-Sample-Codierungen ablehnen. Verwenden Sie die gleiche Version, um eine Datei zu erstellen und daraus eine Präsentation zu erstellen.

Fixieren Sie das exakte Release-Artefakt und die Prüfsumme. Ergebnisse von einer bestimmten Version sind nicht garantiert, mit anderen Versionen übereinzustimmen.

## 1.5.0

Die vorherige Version bietet Erfassung mit Schema v6 für PostgreSQL, MySQL und
SQL Server, die Prüfung strukturierter Dateien, lokale Blueprint- und
Präsentationsausgabe sowie versionsabhängige Berechtigungsskripte. Die genauen
Quellen und Artefakte finden Sie beim
[Release-Tag](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0).

Für Problemmeldungen siehe [SUPPORT.md](SUPPORT.md). Richtlinien für Beiträge
finden Sie in [CONTRIBUTING.md](CONTRIBUTING.md).
