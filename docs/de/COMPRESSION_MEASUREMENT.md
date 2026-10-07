# Komprimierungsmessung

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../COMPRESSION_MEASUREMENT.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../COMPRESSION_MEASUREMENT.md) | **Deutsch** | [Français](../fr/COMPRESSION_MEASUREMENT.md) | [Español](../es/COMPRESSION_MEASUREMENT.md) | [Polski](../pl/COMPRESSION_MEASUREMENT.md) | [日本語](../ja/COMPRESSION_MEASUREMENT.md) | [中文](../zh/COMPRESSION_MEASUREMENT.md)

`dbwarp-blueprint` kann optional messen, wie gut sich repräsentative Tabellendaten komprimieren lassen. Dadurch werden DBWarp-Schätzungen genauer, denn WAN-Übertragungsdauer und Egress-Kosten hängen von den komprimierten Bytes und nicht von der rohen Tabellengröße ab.

Die Komprimierungsmessung ist freiwillig und erfordert eine ausdrückliche Zustimmung. Interaktive Live-Läufe können den Preflight-Dialog bestätigen; unbeaufsichtigte Läufe und strukturierte Dateien verwenden:

```bash
--measure-compression --yes
```

Bei deaktivierter Komprimierungsmessung erfasst die Live-Datenbankaufnahme keine
Zeilenwerte aus Benutzertabellen als Stichproben. Strukturierte Dateien
verhalten sich anders: Avro-Datensätze müssen weiterhin durchlaufen werden,
um Zeilenzahlen, Längen und NULL-Metadaten zu erfassen; siehe
[Strukturierte Dateien](STRUCTURED_FILES.md).

## Was beprobt wird

Für jede geeignete Benutzertabelle, deren Leerheit nicht sicher bewiesen ist,
liest das Werkzeug eine begrenzte Anzahl von Zeilen in den Arbeitsspeicher,
kodiert sie in stabile flüchtige Prüfpuffer, komprimiert diese Puffer lokal mit
zstd auf Stufe 3 und leitet aggregierte Komprimierungs-, NULL-Dichte-,
Kardinalitäts-/Häufigkeits-, Längen- und Stilmessungen ab, bevor es beprobte
Werte und temporäre Fingerprints verwirft.

Für ausgewählte text/binary-Spalten kann Tier 2 auch diese Spalte einzeln abtasten. Dies ermöglicht eine Komprimierbarkeit pro Spalte anstatt nur Tabellen-weite Durchschnittswerte.

Die Verhältnisse von Tabellen in Live-Datenbanken verwenden eine neutrale Sequenz von begrenzten Gruppen mit jeweils 1.000 Zeilen, wobei jede Spalte durch einen Deskriptor gekennzeichnet ist, die Werte haben eine feste Breite und die Datenblöcke sind spaltenweise angeordnet. Dies misst die Struktur, die für die Komprimierung relevant ist, ohne dabei Datenbank- oder Übertragungsprotokolle zu erfassen. Die Verhältnisse pro Spalte behalten `blueprint-compression-probe-v2`, wobei die mit Längenpräfix versehenen Werte, die mit Tags versehen sind, den spezifischeren Entropie-Eingang darstellen.

PostgreSQL-Tabellenblöcke verwenden `blueprint-columnar-transfer-probe-v2`, wobei Gruppierungen von Zeilen durch einen persistenten zstd-Kontext der Stufe 3 geleitet und nach jeder Gruppe geleert werden. MySQL und SQL Server verwenden `blueprint-columnar-transfer-probe-v3`: die gleichen neutralen Bytes und einen persistenten Kontext, mit zusätzlichen Leerungen an den Grenzen von 256-KB-Proben. Die `nvarchar`, `nchar` und `ntext`-Samples von SQL Server werden als UTF-16LE-Byt-Verteilungen gemessen. Die `varchar`, `char` und `text`-Samples von SQL Server behalten ihre gemessene, schmale Bytebreite; das Blueprint speichert den Katalog-Codepage des Quell-Sortierverfahrens als `utf-8`, `windows-N` oder `code-page-N`. Der Datenbanktreiber stellt dekodierte Strings weiterhin dem Sampler zur Verfügung, sodass die Byte-Identität einer alten Codepage nicht beansprucht wird. Die Tabelle `ratio_stddev` wird über die äußeren Ausgaben von Zeilengruppen gemessen. Pro-Spalte-Projektionsblöcke bleiben unabhängige, einmalige Entropiemessungen und geben `0.0` aus. Blueprints aus früheren Versionen können `blueprint-columnar-transfer-probe-v1` enthalten; Verhältnisse mit unterschiedlichen Tags sind nicht vergleichbar.

Die beprobten Bytes gelangen ausschließlich über die ausgewählte
Datenbanksitzung in den lokalen Prozess. Sie werden weder auf den Datenträger
geschrieben noch in `blueprint.toml` oder das Auditprotokoll aufgenommen,
hochgeladen oder an DBWarp-Infrastruktur gesendet.

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
Die Workerzahl ändert die Messwerte nicht. Die anonyme Bezeichnerreihenfolge
variiert absichtlich mit dem standardmäßig neuen Schlüssel; verwenden Sie eine
geschützte `--anonymization-key-file` nur für genehmigte laufübergreifende
Vergleiche erneut.

Der Collector vermeidet Zeilen- und Stilabfragen nur, wenn ein von der Engine
verwalteter Katalogwert zum Zeitpunkt des Kataloglesens sicher beweist, dass
eine Tabelle leer ist. PostgreSQL verlangt aktuelle analysierte Statistiken
ohne nachfolgende Änderungen; SQL Server verwendet seinen Partitionszeilenzähler.
MySQL-Tabellenzeilenschätzungen können bei einer nicht leeren Tabelle null
melden, daher verwendet der Collector sie nicht zum Überspringen. Dieser
konservative Unterschied schützt die Datenqualität.

## Inhalt der Blueprint-Datei

Es werden nur aggregierte Zusammenfassungen ausgegeben. Bei textartigen Spalten kann der Tier-2-Durchlauf eine begrenzte Stilbezeichnung wie `json`, `xml`, `natural-text`, `base64`, `hex`, `numeric-text` oder `mixed` ausgeben.

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
sample_method = "LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "blueprint-compression-probe-v2"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "blueprint-columnar-transfer-probe-v3"
```

Diese Werte werden verwendet, um die Größe des Netzwerktransfers abzuschätzen.

## Bedeutung

Zwei Datenbanken mit derselben rohen Tabellengröße können sich während einer Migration sehr unterschiedlich verhalten:

- JSON, XML, wiederholte Geschäftscodes, dünn besetzter Text und natürlichsprachiger Text lassen sich häufig gut komprimieren.
- Verschlüsselte Werte, bereits komprimierte Blobs, zufällige Token und Binärdaten mit hoher Entropie lassen sich nicht gut komprimieren.
- Unicode- und schmale Textdaten in SQL Server haben unterschiedliche Byteverteilungen. Der Sampler modelliert `nvarchar` als UTF-16LE und zeichnet die Kollations-Codepage auf, die zur Interpretation von `varchar` erforderlich ist, statt jede Textspalte als UTF-8 zu behandeln.

Eine kleine lokale Messung ist üblicherweise hilfreicher als eine Schätzung anhand der Spaltentypen.

## Verzerrung und Transparenz

Einige Engines bieten keine vollkommen gleichmäßige Tabellenstichprobe. MySQL
verteilt eine begrenzte Stichprobe auf vier Bereiche des numerischen
Primärschlüssels, wenn dieser Zugriffspfad verfügbar ist, und fällt sonst auf
`LIMIT N` zurück. Beide Verfahren bleiben ausdrücklich als verzerrt markiert, da
keines eine statistische Zufallsstichprobe darstellt. Nicht abschließende
Bereichsfenster haben eine exklusive Obergrenze. Dünn oder ungleichmäßig belegte
Primärschlüsselbereiche können daher ein Fenster unterfüllen, aber ein Fenster
kann keine Zeilen aus dem nächsten erneut lesen. Andere weniger geeignete
Engine-Fallbacks werden ebenso mit `sampled_with_bias` und `bias_reason`
aufgezeichnet.

Blueprint speichert die Struktur einer begrenzten Stichprobe getrennt von den entsprechenden Textfeldern. Die Bereichsabtastung numerischer Primärschlüssel in MySQL gibt `sample_layout = "primary-key-range-windows"` aus und sortiert jedes Fenster nach dem vollständigen Primärschlüssel.

Voreingenommene Stichproben sind zwar weiterhin nützlich, aber sie haben eine geringere Zuverlässigkeit. Das Audit-Protokoll protokolliert, dass die Zeilenstichproben aktiviert wurden und die Anzahl der lokal codierten Prüfbytes. Datenbank-Sitzungs-Byte-Gesamtwerte werden als `unknown` gemeldet, wenn der Treiber diese nicht bereitstellt.

## Praktische Stichprobeneinstellungen

Erster produktionssicherer Durchlauf:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

Genauere Messung, wenn eine Read-Replika oder ein Wartungsfenster verfügbar ist:

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

Große Datenbanken benötigen keine riesigen Stichproben. Ziel ist ein stabiles Komprimierungssignal und keine exakte Profilierung auf Zeilenebene. `--max-wall-secs` ist eine harte Frist für die gesamte Live-Erfassung einschließlich Verbindung, Katalogen, RTT und Stichproben, nicht ein neues Budget pro Phase.

Live-Datenbankstichproben unterliegen außerdem einer nicht konfigurierbaren
Obergrenze von 16 MiB für die projizierte Nutzlast je Tabelle. Die anfängliche
SQL-Projektion ist nach Typ budgetiert und beobachtet die ursprünglichen
Oktettlängen getrennt. Wenn ein projizierter Wert gekürzt wurde, können MySQL
und SQL Server die Messung mit weniger Zeilen und angepassten Grenzen je
Spalte wiederholen, die weiterhin in das Budget passen. Werte, die für dieses
Budget zu breit sind, bleiben begrenzte Präfixe. Die Herkunftsangaben für
Komprimierung und Wertzusammenfassungen dokumentieren diese Einschränkung;
die Längenstatistiken behalten dagegen die vom Server gemeldeten ursprünglichen
Längen der Stichprobenwerte gemäß der ausgewählten Längentreuerichtlinie bei.

Die Obergrenze begrenzt weder Netzwerkbytes noch Prozessspeicher.
Protokollkodierung, Metadaten der ursprünglichen Länge, Wiederholungen und
Treiberpuffer verursachen zusätzlichen Aufwand. Das Audit zeichnet die
konfigurierte Nutzlastobergrenze, die ausgeführten Abfragen und die genaue
lokal kodierte Byteanzahl des Prüfpuffers auf; es meldet keinen gemessenen
Datenbankverkehr auf der Leitung.

## Wie die Messungen interpretiert werden.

Das Feld `sample_encoding` ist Teil des Vertrags. Verhältnisse sind nur innerhalb eines einzelnen Encoding-Tags vergleichbar, da unterschiedliche Sample-Encodings unterschiedliche Kompressionsraten für die gleichen logischen Daten erzeugen können. Insbesondere sind das tabellenbasierte Verhältnis für den spaltenweisen Datentransfer und die pro-Spalte-Verhältnisse der Version 2 komplementäre Messungen und dürfen nicht gegeneinander ausgetauscht werden.
