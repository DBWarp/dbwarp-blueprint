# Fehlerbehebung

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../TROUBLESHOOTING.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../TROUBLESHOOTING.md) | **Deutsch** | [Français](../fr/TROUBLESHOOTING.md) | [Español](../es/TROUBLESHOOTING.md) | [Polski](../pl/TROUBLESHOOTING.md) | [日本語](../ja/TROUBLESHOOTING.md) | [中文](../zh/TROUBLESHOOTING.md)

Häufige Fehler von `dbwarp-blueprint` und die nächsten Schritte.

Fehler beginnen mit einem stabilen `DBPnnnnS`-Meldungscode, beispielsweise `DBP1001E`. Verwenden Sie den Code bei der Suche in der Dokumentation oder bei der Meldung eines Problems. Siehe [Operator-Meldungscodes](MESSAGES.md).

## Quellcode-Build endet mit Status 2 oder 3

`build.sh` beendet sich mit dem Status 2, wenn die installierte `rustc`-Version nicht mit der in `rust-toolchain.toml` angegebenen Version übereinstimmt und der Netzwerkzugriff nicht explizit genehmigt wurde. Installieren Sie genau diese Rust-Version, oder erlauben Sie dem Skript, nach Überprüfung der Download-Grenzen seine festgelegte, checksum-überprüfte Bootstrap-Datei abzurufen:

```bash
ALLOW_NETWORK=1 ./build.sh
```

`build.sh` endet mit Status 3, wenn `DBWARP_BLUEPRINT_OFFLINE=1` gesetzt ist,
der Quellbaum aber kein Verzeichnis `vendor-crates/` enthält. Ein normaler
Quellcode-Checkout kann diesen Modus nicht erfüllen. Laden Sie das gebündelte
Quellcodearchiv desselben Releases herunter, prüfen und entpacken Sie es und
führen Sie den Build in diesem entpackten Baum aus:

```bash
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Kopieren Sie keine Abhängigkeiten zwischen Release-Versionen. Die vollständigen
Online- und Offline-Verfahren stehen unter [Aus Quellcode
erstellen](BUILD.md).

## Angeforderte Sprache wird nicht verwendet

Verwenden Sie bei der Diagnose der Sprachauswahl ausdrücklich einen unterstützten Wert:

```bash
dbwarp-blueprint --lang pl --help
```

Unterstützte Werte sind `en`, `de`, `fr`, `es`, `pl`, `ja` und `zh`. Ohne `--lang` prüft das Werkzeug `DBWARP_BLUEPRINT_LANG`, `LC_ALL`, `LC_MESSAGES` und `LANG` in dieser Reihenfolge. Ein nicht unterstützter expliziter Wert wird mit `DBP1011E` abgelehnt; ein unvollständiger eingebetteter Katalog lässt den Start mit `DBP1010E` fehlschlagen, statt auf Englisch zurückzufallen.

Unter Windows fehlen die Locale-Variablen normalerweise; übergeben Sie daher `--lang` oder setzen Sie `DBWARP_BLUEPRINT_LANG`.

## Bannerbreite oder Farben sehen falsch aus

Die Bannerbreite stammt aus `COLUMNS`, sofern gesetzt, sonst aus der Konsole unter Linux und macOS, sonst beträgt sie 80 Spalten. Die Farbfähigkeit stammt aus `NO_COLOR`, `TERM` und `COLORTERM`; fehlt `TERM`, was unter Windows normal ist, werden 16 Farben verwendet. Überschreiben Sie dies mit `--color always`, `--color never` oder einem expliziten `COLUMNS`.

## Passwort in URI wird abgelehnt

Symptom:

```text
DBP1001E refusing to use URI-embedded password
```

Lösung: Entfernen Sie das Passwort aus der URI und verwenden Sie eine der folgenden Möglichkeiten:

```bash
--password-file /path/to/pass
--password-env DBWARP_BLUEPRINT_PASSWORD
```

Unter Unix darf der Dateimodus keinen Lesezugriff für Gruppe/Andere zulassen.

## Berechtigungsfehler bei Passwortdatei

Symptom: Das Werkzeug lehnt `--password-file` oder `--tls-key` ab, weil die Berechtigungen zu weit gefasst sind.

Lösung:

```bash
chmod 600 /etc/dbwarp/db.pass
chmod 600 /etc/dbwarp/client.key
```

Dies verhindert eine versehentliche Offenlegung gegenüber lokalen Benutzern desselben Hosts.

## TLS-Prüfung schlägt fehl

Verwenden Sie `--tls-mode verify-full` mit dem richtigen CA-Bundle und Hostnamen:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

Wenn der Hostname im Zertifikat nicht übereinstimmt, korrigieren Sie den DNS-Namen oder das Zertifikat. `--tls-skip-verify` wird auf Nicht-Loopback-Hosts abgelehnt, sofern nicht zusätzlich `--i-know-what-im-doing` angegeben wird; verwenden Sie diese Option nicht in der Produktion.

## SQL-Server-TLS-Vertrauensanker

Bei SQL Server verwenden die Modi mit Zertifikatsprüfung den Trust Store des
Betriebssystems, wenn `--tls-ca` nicht angegeben ist. Eine bereitgestellte
`.pem`- oder `.crt`-Datei muss genau ein CA-Zertifikat enthalten und ersetzt
diese Stammzertifikate. Der Treiber prüft den Verbindungshostnamen sowohl bei
`verify-ca` als auch bei `verify-full`.

## Zustimmung erforderlich

Symptom bei einem Lauf gegen eine Live-Datenbank, wenn die Bestätigungsfrage
nicht mit `y` oder `yes` beantwortet wird:

```text
DBP1701E aborted (no consent)
```

Symptom, wenn eine Komprimierungsstichprobe für eine strukturierte Datei ohne
`--yes` angefordert wird:

```text
DBP1006E ... --measure-compression requires --yes (consent flag)
```

Lösung: Prüfen Sie die Vorabzusammenfassung und antworten Sie an der
Eingabeaufforderung mit `yes`, oder führen Sie einen nicht interaktiven Lauf mit
`--yes` erneut aus:

```bash
--measure-compression --yes
```

Dies ist bewusst explizit, weil Tier 2 begrenzte Zeilenstichproben in den Speicher liest, bevor sie verworfen werden.

## Stichprobenerfassung dauert zu lange

Reduzieren Sie eine oder beide Optionen:

```bash
--sample-rows 500
--max-wall-secs 120
```

Für die erste Produktionsprüfung ist eine kleinere Tier-2-Stichprobe besser als gar keine Komprimierungsmessung. Wenn die Ergebnisse verzerrt oder unvollständig sind, führen Sie den Lauf auf einer Replik mit einem größeren Budget erneut aus.

## DBA verbietet die katalogfremde Prüfung SELECT 1

Deaktivieren Sie die RTT-Prüfung:

```bash
--no-rtt-probe
```

Die standardmäßige RTT-Prüfung besteht aus fünf `SELECT 1`-Abfragen und liest keine Zeilendaten; einige Richtlinien stufen jedoch jede katalogfremde Abfrage als außerhalb des Geltungsbereichs ein.

## Ausgabe enthält keine Komprimierungsabschnitte

Komprimierungsabschnitte erscheinen nur, wenn beide Optionen vorhanden sind:

```bash
--measure-compression --yes
```

Katalog-spezifische Blueprints sind gültig, aber die Kompressionsschätzungen werden aus den Spaltentypen abgeleitet.

## Einige Komprimierungsstichproben sind als verzerrt markiert

Einige Engines bieten keine einheitliche Tabellenstichprobenentnahme in allen Fällen, und kleine Tabellen benötigen möglicherweise eine `LIMIT`-Fallback-Lösung. Die Blueprint-Datei protokolliert `sampled_with_bias` und `bias_reason`, damit ein Prüfer dies berücksichtigen kann.

Verzerrte Stichproben sind weiterhin nützlich; sie sind lediglich weniger aussagekräftig als gleichmäßige Stichproben.

## Präsentationserzeugung aus TOML schlägt fehl

`--from-toml` muss zusammen mit `--deck` verwendet werden:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

Fügen Sie bei `--from-toml` keine Optionen für Live-Datenbanken hinzu. Das Werkzeug lehnt gemischte Live-/Offline-Modi ab, um die Auditgrenze einfach zu halten.

## Blueprint-Datei wirkt zu klein

Eine normale Blueprint-Datei ist kompakt. Sie enthält strukturelle Metadaten, gerundete Zählwerte, Indizes, die Struktur des Fremdschlüsselgraphen und optionale Komprimierungszusammenfassungen. Sie sollte keine Zeilenwerte oder Bezeichner enthalten.


## Nachweis erforderlich, dass kein Upload stattgefunden hat

Verwenden Sie das Auditprotokoll und Netzwerkwerkzeuge:

```bash
./dbwarp-blueprint ... --audit-log audit.txt
strace -f -e trace=connect ./dbwarp-blueprint ...
tcpdump host db.internal
```

Das erwartete Netzwerkverhalten zur Laufzeit hängt vom aktiven Modus ab. Ein
Live-Lauf mit `--connect` öffnet die angeforderte Datenbanksitzung; DNS kann den
konfigurierten Resolver kontaktieren, und die integrierte Kerberos-/SSPI-
Authentifizierung kann einen KDC oder Domänencontroller kontaktieren. Der
Batch-Modus öffnet pro Datenbankquelle eine Datenbanksitzung. Lokale TOML-,
Parquet-, Avro- und Bundle-Operationen stellen keine Netzwerkverbindung der
Anwendung her, wobei im Netzwerk eingebundene Pfade weiterhin dem Speicherstack
des Hosts unterliegen.
