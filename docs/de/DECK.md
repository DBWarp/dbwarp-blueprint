# Visuelle Zusammenfassungspräsentation

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../DECK.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../DECK.md) | **Deutsch** | [Français](../fr/DECK.md) | [Español](../es/DECK.md) | [Polski](../pl/DECK.md) | [日本語](../ja/DECK.md) | [中文](../zh/DECK.md)

`dbwarp-blueprint --deck blueprint.pptx` schreibt neben der TOML-Datei von `--out` eine optionale PowerPoint-Zusammenfassung (`.pptx`) des Blueprints. Mit `dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx` kann dieselbe Präsentation später aus einer vorhandenen, geprüften Blueprint-Datei erstellt werden, ohne eine Verbindung zu einer Datenbank herzustellen. Sie stellt dieselben anonymisierten Daten dar: Aus Ihrer Datenbank wird nichts Weiteres gelesen oder an sie gesendet. Die Präsentation berechnet nur die dokumentierten lokalen Zusammenfassungen und Projektionen aus Feldern, die bereits im Blueprint vorhanden sind.

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

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx \
  --lang ja
```

`--lang en|de|fr|es|pl|ja|zh` lokalisiert die menschenlesbare Prosa der Präsentation und die PowerPoint-Sprachmetadaten. Anonyme Bezeichner, Datenbanktypnamen, Indexmethoden, Messwerte und das Quell-TOML bleiben kanonisch und sprachneutral. Bei einem fehlenden Präsentationstext bricht die Katalogvalidierung sicher ab, statt ersatzweise Englisch einzusetzen. Siehe [`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## Fußzeile und Vertraulichkeit

Jede Inhaltsfolie verwendet die DBWarp-Hausfußzeile: links ein kleines Logo,
optional ein Trennzeichen und eine Vertraulichkeitsstufe, eine alleinstehende
zentrierte Foliennummer und rechts `DBWarp.com`. Die Titelfolie bleibt unnummeriert.

Mit `--deck-confidentiality public|internal|confidential|restricted` wird eine
der lokalisierten integrierten Klassifizierungskennzeichnungen hinzugefügt.
Jeder andere sichere, nicht leere Wert ist eine benutzerdefinierte Kennzeichnung
und wird unverändert angezeigt; Werte mit Leerzeichen müssen in Anführungszeichen
gesetzt werden, zum Beispiel `--deck-confidentiality "CLIENT // SENSITIVE"`.
Kennzeichnungen dürfen keine Leerzeichen am Anfang oder Ende, keine Steuer- oder
bidirektionalen Formatierungszeichen enthalten und höchstens 48 Anzeigeeinheiten
lang sein. Lassen Sie die Option weg, wenn keine Kennzeichnung erscheinen soll.
Die Einstellung ändert nur die Präsentation; weder die Blueprint-Datei noch die
im Deck zusammengefassten Daten werden verändert. Bei exakt demselben geprüften
Blueprint, derselben Sprache, Kennzeichnung und demselben Zeitstempel sind die
Bytes der Präsentation reproduzierbar.

## Vertrauenseigenschaften

- **Lokal aus dem Arbeitsspeicher erstellt.** Die Präsentation wird aus demselben Blueprint im Arbeitsspeicher erzeugt, aus dem auch `blueprint.toml` entsteht. Es gibt keine zusätzliche Datenbankabfrage und keinen zweiten Durchlauf durch den Katalog. Im Modus `--from-toml` wird der Blueprint im Arbeitsspeicher stattdessen aus der geprüften TOML-Datei geladen.
- **Kein Anwendungsnetzwerk.** Beim Erzeugen der Präsentation wird keine Netzwerkverbindung geöffnet; ein Blueprint auf einem netzwerkgebundenen Pfad unterliegt weiterhin dem Speicher-Stack des Hosts.
- **Keine Drittanbieterbibliothek.** Das OOXML wird direkt in `src/deck.rs` geschrieben; die Datei `.pptx` ist ein gewöhnliches ZIP-Archiv mit XML-Bestandteilen, das Sie mit `unzip` entpacken und lesen können. Es gibt weder PowerPoint-Automatisierung noch einen Rendering-Dienst oder eine zusätzliche Crate im Abhängigkeitsgraphen. Die freigegebenen DBWarp-Logobilder und die statischen DM Sans-Schriftarten sind in die Rust-Binärdatei eingebettet und werden als OOXML-Medien- und Schriftbestandteile geschrieben; die Erzeugung liest keinen Asset-Pfad zur Laufzeit.
- **Keine echten Bezeichner und keine Zeilendaten.** Tabellen, Spalten und Indizes erscheinen mit denselben anonymen Platzhaltern wie in der Blueprint-Datei (`table-001`, `col-1`, `idx-1`, `schema-A`). Quellmessungen behalten ihre dokumentierte Genauigkeit; Projektionen werden nur aus bereits im Blueprint vorhandenen Feldern berechnet. Die Präsentation enthält keine kundenspezifischen Informationen über diese Eingabe hinaus.
- **Aus festgelegter Eingabe reproduzierbar.** Derselbe geprüfte Blueprint erzeugt für dieselbe ausgewählte Sprache, Vertraulichkeitskennzeichnung und denselben festgeschriebenen Zeitstempel eine byteidentische Datei `.pptx` (feste Reihenfolge der Bestandteile und feste Zeitstempel). Zwei Live-Erfassungen werden dadurch nicht identisch; dafür sind auch derselbe geschützte `--anonymization-key-file`, derselbe Quellzustand und dieselben Erfassungsoptionen erforderlich.

## Inhalt

Die Präsentation passt sich an die Schemagröße an:

- **Titel**: DBWarp-Logo und Tagline, Engine, Version, Quellenart, Tabellenanzahl und Erzeugungszeitstempel.
- **Executive Summary**: managementgerechte Signale zu Migrationsumfang, Datenkonzentration, Beziehungskomplexität und Prüfevidenz.
- **Übersicht**: Summen für Tabellen, Zeilen, Datengröße und Indexgröße sowie Anzahl der Spalten, Indizes, Fremdschlüssel und Schemata.
- **Kleine Schemata** (wenige Tabellen): ein größenproportionales Feld je Tabelle (Zeilen, Bytes, Spaltentypen, Indizes) und ein Fremdschlüsseldiagramm.
- **Große Schemata**: Charakterisierung statt Aufzählung:
  - *Größte Tabellen*: die größten Tabellen nach Größe mit einem Restposten `+ N more`.
  - *Schemazusammensetzung*: Verteilung der Spaltentypen sowie Index-/Gesamtstatistiken.
  - *Beziehungen*: Anzahl der Fremdschlüssel, verbundene gegenüber eigenständigen Tabellen und die am häufigsten referenzierten Hub-Tabellen.
- **Gemessene Komprimierung** (nur Tier 2): Anzahl der beprobten Tabellen, gewichtetes zstd-3-Verhältnis, prognostizierter komprimierter Speicherbedarf und die am besten komprimierbaren beprobten Tabellen.
- **Vertrauensmodell**: eine Abschlussfolie, welche die obigen Garantien zusammenfasst.

## Ausgabe prüfen

Die Datei `.pptx` ist ein standardmäßiges OOXML-Paket. So prüfen Sie exakt, was sie enthält:

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

Öffnen Sie sie in PowerPoint, LibreOffice Impress oder Google Slides. Der Generator befindet sich in [`src/deck.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/deck.rs) und ist in die Rust-Binärdatei integriert. Es gibt keinen separaten Präsentationsgenerator, der installiert, geprüft oder synchron gehalten werden müsste.
