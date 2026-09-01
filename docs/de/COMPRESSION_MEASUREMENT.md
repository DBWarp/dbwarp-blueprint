# Komprimierungsmessung

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../COMPRESSION_MEASUREMENT.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../COMPRESSION_MEASUREMENT.md) | **Deutsch** | [Français](../fr/COMPRESSION_MEASUREMENT.md) | [Español](../es/COMPRESSION_MEASUREMENT.md) | [Polski](../pl/COMPRESSION_MEASUREMENT.md) | [日本語](../ja/COMPRESSION_MEASUREMENT.md) | [中文](../zh/COMPRESSION_MEASUREMENT.md)

`dbwarp-blueprint` kann optional messen, wie gut sich repräsentative Tabellendaten komprimieren lassen. Dadurch werden DBWarp-Schätzungen genauer, denn WAN-Übertragungsdauer und Egress-Kosten hängen von den komprimierten Bytes und nicht von der rohen Tabellengröße ab.

Die Komprimierungsmessung ist freiwillig und erfordert eine ausdrückliche Zustimmung. Interaktive Live-Läufe können den Preflight-Dialog bestätigen; unbeaufsichtigte Läufe und strukturierte Dateien verwenden:

```bash
--measure-compression --yes
```

Ohne diese Flags liest das Werkzeug ausschließlich Katalogmetadaten.

## Was beprobt wird

Das Werkzeug liest für jede Benutzertabelle eine begrenzte Anzahl von Zeilen in den Arbeitsspeicher, kodiert sie in einen deterministischen Zeilenrahmenpuffer, komprimiert diesen Puffer lokal mit zstd auf Stufe 3, zeichnet gerundete Verhältnisse auf und verwirft den Puffer.

Tier 2 kann ausgewählte Text-/Binärspalten außerdem einzeln beproben. Dadurch können nachgelagerte Planungswerkzeuge die Entropie einzelner Spalten nachbilden, statt sich ausschließlich auf Durchschnittswerte der gesamten Tabelle zu stützen.

Jede Messung ist ein unabhängiger zstd-Frame in einem Schritt mit zugesagter Eingabegröße. Die Verhältnis-Varianz (`ratio_stddev`) wird über zeilenbündige 64-KiB-Blöcke desselben Puffers gemessen, sodass die Varianz die vom Schätzer vorhergesagte Übertragung und nicht nur einen einzigen Durchschnitt des gesamten Puffers beschreibt. Da die Eingabegröße zugesagt wird, wählt zstd größenangepasste Parameter, die mit dem Übertragungsmodell des Schätzers übereinstimmen. Bei kleinen Stichproben (unter etwa 1 MiB) können sich die Verhältnisse gegenüber Erfassungen früherer Versionen, die über einen Streaming-Kontext ohne Größenzusage maßen, spürbar verschieben; kleine Tabellen sind über diese Grenze hinweg nicht direkt vergleichbar. Maßgeblich ist die zugesagte Messung, denn sie entspricht der Übertragung.

Die beprobten Bytes werden nicht auf den Datenträger geschrieben, nicht in `blueprint.toml` oder das Auditprotokoll aufgenommen und über keine andere Verbindung als die vom Datenbankserver zu dem von Ihnen ausgeführten lokalen Prozess übertragen.

## Lokale Worker-Parallelität

Die Datenbankstichprobe verwendet immer eine einzelne sequenzielle Verbindung.
Die optionale Einstellung `--compression-workers N` parallelisiert nur die
lokale Komprimierung bereits gelesener In-Memory-Stichproben. Zulässig sind
1–32 Worker; der Standardwert ist 1, um die Auswirkungen auf den Quellhost zu
minimieren. Erhöhen Sie ihn ausdrücklich, um mehr lokale CPU zu verwenden:

```bash
--measure-compression --yes \
--compression-workers 4
```

Höhere Werte können die Laufzeit verkürzen, wenn zstd der Engpass ist, erhöhen
jedoch lokale CPU-Last und maximalen Speicherbedarf. Sie erzeugen keine
gleichzeitigen Datenbank-Stichprobenverbindungen. Jeder Worker besitzt eigene
zstd-Kontexte, und die Eingabewarteschlange ist auf die Workerzahl begrenzt.
Ausgabereihenfolge und v6-Blueprint-Werte bleiben deterministisch.

Der Collector vermeidet Zeilen- und Stilabfragen nur, wenn ein von der Engine
verwalteter Katalogwert zum Zeitpunkt des Kataloglesens sicher beweist, dass
eine Tabelle leer ist. PostgreSQL verlangt aktuelle analysierte Statistiken
ohne nachfolgende Änderungen; SQL Server verwendet seinen Partitionszeilenzähler.
MySQL-Tabellenzeilenschätzungen können bei einer nicht leeren Tabelle null
melden, daher verwendet der Collector sie nicht zum Überspringen. Dieser
konservative Unterschied schützt die Datenqualität.

## Inhalt der Blueprint-Datei

Es werden nur zusammenfassende Zahlen ausgegeben. Bei textartigen Spalten kann der Tier-2-Durchlauf eine begrenzte Stilbezeichnung wie `json`, `xml`, `natural-text`, `base64`, `hex`, `numeric-text` oder `mixed` ausgeben.

Beispiel:

```toml
[tables.table-001.cols.col-2]
ordinal = 2
type = "json"
nullable = false
len_avg = 430
len_p95 = 0
style = "json"

[tables.table-001.cols.col-2.compression]
measured = true
sample_rows = 1000
sample_bytes = 65536
sample_method = "column LIMIT N (engine-specific bounded sample)"
sampled_with_bias = true
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "dbwarp-blueprint-rowframe-v1"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

Diese Werte helfen genehmigten nachgelagerten Werkzeugen, die Netzwerkübertragungsgröße zu schätzen und synthetische Text-/Binärdaten mit ähnlicher Komprimierbarkeit zu erzeugen.

## Bedeutung

Zwei Datenbanken mit derselben rohen Tabellengröße können sich während einer Migration sehr unterschiedlich verhalten:

- JSON, XML, wiederholte Geschäftscodes, dünn besetzter Text und natürlichsprachiger Text lassen sich häufig gut komprimieren.
- Verschlüsselte Werte, bereits komprimierte Blobs, zufällige Token und Binärdaten mit hoher Entropie lassen sich nicht gut komprimieren.
- SQL-Server-Daten vom Typ `nvarchar` haben eine andere Byteverteilung als UTF-8-Text und werden für die Stichprobe entsprechend kodiert.

Eine kleine lokale Messung ist üblicherweise hilfreicher als eine Schätzung anhand der Spaltentypen.

## Verzerrung und Transparenz

Einige Engines bieten keine vollkommen gleichmäßige Tabellenstichprobe. Wenn das Werkzeug auf ein weniger geeignetes Verfahren zurückfällt, kennzeichnet die Blueprint-Datei dies mit `sampled_with_bias` und `bias_reason`.

Verzerrte Stichproben sind weiterhin nützlich, nachgelagerte Werkzeuge sollten sie jedoch mit geringerem Vertrauen behandeln. Das Audit zeichnet die aktivierte Zeilenstichprobe und die lokal codierten Rowframe-Bytes auf. Wire-Byte-Werte bleiben `unknown`, wenn der Treiber sie nicht bereitstellt.

## Praktische Stichprobeneinstellungen

Erster produktionssicherer Durchlauf:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

Bessere Estimator-Eingabe, wenn ein Lesereplikat oder Wartungsfenster verfügbar ist:

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

Große Datenbanken benötigen keine riesigen Stichproben. Ziel ist ein stabiles Komprimierungssignal und keine exakte Profilierung auf Zeilenebene. `--max-wall-secs` ist eine harte Frist für die gesamte Live-Erfassung einschließlich Verbindung, Katalogen, RTT und Stichproben, nicht ein neues Budget pro Phase.

Live-Datenbankstichproben unterliegen außerdem einer nicht konfigurierbaren
Obergrenze von 16 MiB für die projizierte Nutzlast je Tabelle. Die
SQL-Projektion kürzt Spalten variabler Breite auf dem Server und reduziert bei
außergewöhnlich breiten Tabellen die Zeilenobergrenze, bevor der Treiber Daten
empfängt. Sehr große LOB-Werte tragen daher nur mit begrenzten Präfixen statt
mit ihrem vollständigen Inhalt bei. Das Audit zeichnet die aktive Obergrenze
der Tabellennutzlast und die genaue Byteanzahl des lokal kodierten Zeilenframes
auf.

## Verwendung durch nachgelagerte Verbraucher

Ein nachgelagerter Verbraucher sollte Komprimierungsevidenz in dieser Reihenfolge verwenden:

1. erkannte Komprimierungsblöcke pro Spalte;
2. erkannte Komprimierungsblöcke auf Tabellenebene;
3. Typ-/Stilvorgaben, wenn kein gemessenes Verhältnis vorhanden ist.

Das Feld `sample_encoding` ist Teil des Vertrags. Verbraucher sollten nur Verhältnisse mit einer erkannten Kodierungskennzeichnung verwenden, weil unterschiedliche Stichprobenkodierungen für dieselben logischen Daten unterschiedliche Komprimierungsverhältnisse ergeben können.
