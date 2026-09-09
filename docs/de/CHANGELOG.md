# Änderungsprotokoll

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../CHANGELOG.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../CHANGELOG.md) | **Deutsch** | [Français](../fr/CHANGELOG.md) | [Español](../es/CHANGELOG.md) | [Polski](../pl/CHANGELOG.md) | [日本語](../ja/CHANGELOG.md) | [中文](../zh/CHANGELOG.md)

Release-Versionen bezeichnen den Collector. Die Blueprint-Schemaversion und
die Kodierung der Komprimierungsstichproben sind getrennte
Kompatibilitätsverträge; siehe [FORMAT.md](FORMAT.md) und
[Komprimierungsmessung](COMPRESSION_MEASUREMENT.md).

## 1.5.1

### Erfassung und Datentreue

- Blueprint-Schema v6 bleibt erhalten; Katalogschätzungen, Beobachtungen aus
  Stichproben und nicht verfügbare Nachweise zur Aktualität von Statistiken
  werden unterschieden.
- Verbesserte Messung binärer Nutzlasten, Profilierung bereits komprimierter
  Nutzlasten und begrenzte Komprimierungsprüfpuffer. Verhältnisse mit
  unterschiedlichen `sample_encoding`-Werten sind nicht austauschbar;
  Verbraucher müssen die Kodierung erkennen, bevor sie eine Messung verwenden.
- Begrenzte adaptive Wiederholungen der MySQL- und SQL-Server-Stichproben,
  einschließlich übergroßer Werte und Zeichensatzerweiterung. Verbleibende
  Präfixverzerrung wird dokumentiert, während die Metadaten zu den
  ursprünglichen Längen der Stichprobenwerte erhalten bleiben.
- Korrigierte Erkennung von MySQL-Kürzungen, wenn der Verbindungszeichensatz
  die zurückgegebene Bytelänge verändert.
- Verbesserte Behandlung synthetischer Wertlängen, Kardinalität, Verteilung
  und Datenlokalität im gemeinsamen Blueprint-Kern.

### Betrieb und Prüfung

- Präzisierte Einrichtung dedizierter Konten mit minimalen Berechtigungen,
  Anleitungen zum Build aus Quellen, Verifikation von Vergleichsbuilds und
  Matrix der qualifizierten Datenbankversionen.
- Aktualisierte maschinell übersetzte Dokumentation und Laufzeitformulierungen;
  Englisch bleibt maßgeblich, Übersetzungen bleiben ergänzend.
- Gehärtete Prüfungen der Ausführbarkeitsrechte öffentlicher Quelldateien und
  der Release-Archive.
- Versionshistorie sowie Support- und Beitragsrichtlinien werden mit Quellen
  und Binärdateien ausgeliefert.
- Nicht verwendete ASCII-Grafiken wurden entfernt, ohne die unterstützten
  Bannermodi zu ändern.

### Kompatibilität und Einführung

Vorhandene Blueprints bleiben für den neuen Collector lesbar. Die umgekehrte
Richtung ist nicht garantiert: Der Reader von 1.5.0 lehnt das neue optionale
Feld `sample_layout` ab, und ältere Verbraucher können die neuen Kodierungen
für Komprimierungsstichproben ablehnen. Ein Parser für Schema v6 allein belegt
daher keine Vorwärtskompatibilität. Aktualisieren und validieren Sie die
verbrauchenden Werkzeuge gemeinsam mit dem Collector, bevor Sie neue
Erfassungen für Präsentationen, Datenerzeugung oder komprimierungsbasierte
Planung verwenden.

Schreiben Sie das genaue Release-Artefakt und seine Prüfsumme fest. Die
Qualifikation einer früheren Version belegt nicht, dass eine andere Binärdatei
identische Ergebnisse erzeugt.

## 1.5.0

Die vorherige Version bietet Erfassung mit Schema v6 für PostgreSQL, MySQL und
SQL Server, die Prüfung strukturierter Dateien, lokale Blueprint- und
Präsentationsausgabe sowie versionsabhängige Berechtigungsskripte. Die genauen
Quellen und Artefakte finden Sie beim
[Release-Tag](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0).

Für Problemmeldungen siehe [SUPPORT.md](SUPPORT.md). Richtlinien für Beiträge
finden Sie in [CONTRIBUTING.md](CONTRIBUTING.md).
