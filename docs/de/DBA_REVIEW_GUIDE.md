# Leitfaden für die DBA-Prüfung

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../DBA_REVIEW_GUIDE.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../DBA_REVIEW_GUIDE.md) | **Deutsch** | [Français](../fr/DBA_REVIEW_GUIDE.md) | [Español](../es/DBA_REVIEW_GUIDE.md) | [Polski](../pl/DBA_REVIEW_GUIDE.md) | [日本語](../ja/DBA_REVIEW_GUIDE.md) | [中文](../zh/DBA_REVIEW_GUIDE.md)

Dieser Leitfaden richtet sich an DBAs und Sicherheitsprüfer, die entscheiden, ob `dbwarp-blueprint` in einer produktiven oder produktionsähnlichen Umgebung ausgeführt werden darf.

## Ausführungsmodell

`dbwarp-blueprint` ist eine lokale Befehlszeilen-Binärdatei. Im Live-Modus öffnet das Werkzeug eine Datenbankverbindung zu der von Ihnen angegebenen URI und schreibt eine lokale TOML-Datei. Es kontaktiert weder DBWarp-Infrastruktur noch Cloud-APIs, Telemetrieendpunkte, Lizenzserver oder Updateserver.

Im Präsentationsmodus `--from-toml` stellt es überhaupt keine Datenbankverbindung her.

## Empfohlenes Konto

Verwenden Sie ein dediziertes Konto mit geringen Berechtigungen und Lesezugriff auf Katalogmetadaten sowie, falls die Tier-2-Komprimierung aktiviert ist, der Berechtigung, Zeilen aus Benutzertabellen als Stichprobe zu lesen.

Empfohlene Eigenschaften:

- keine Schreibberechtigungen;
- keine DDL-Berechtigungen;
- keine Superuser-/Administratorrolle;
- Lesezugriff ist auf die zu prüfende Datenbank begrenzt;
- Passwort oder Token wird per Datei oder Eingabeaufforderung bereitgestellt und nicht in die URI eingebettet.

Die genauen Berechtigungen unterscheiden sich je nach Engine und Kundenrichtlinie. Wenn das Konto einige Katalogansichten nicht lesen oder aus einigen Tabellen keine Stichprobe nehmen kann, sollte das Werkzeug mit einer klaren Meldung fehlschlagen oder einen reduzierten Blueprint ausgeben; bewahren Sie das Auditprotokoll auf.

Verwenden Sie die versionsabhängigen Skripte und Hinweise in
[`../../sql/grants/README.md`](../../sql/grants/README.md). Entfernen Sie das
dedizierte Erfassungskonto nach der genehmigten Erfassung mit dem passenden
Skript unter `sql/revoke/`; prüfen Sie vor der Ausführung genau die Ziele für
Datenbank, Hostmuster, Rolle und Login.

## Tier 1: Nur Katalog

Tier 1 ist die Standardeinstellung, wenn `--measure-compression` fehlt.

Es liest:

- Engine-Version;
- Tabellenliste und anonymisierte Eingaben für die Sortierung;
- ungefähre Zeilenzahlen;
- Tabellen- und Indexgrößen;
- Spaltentypfamilien, NULL-Zulässigkeit und gerundete Längenstatistiken, soweit verfügbar;
- Indextyp, Eindeutigkeit und anonymisierte Spaltenordnungsnummern;
- Struktur des Fremdschlüsselgraphen, soweit verfügbar;
- optionale kundenseitige RTT-Prüfung, sofern `--no-rtt-probe` nicht gesetzt ist.

Es liest keine Zeilenwerte.

## Inventar der Nicht-Tabellenartefakte

Seit Schema v4 inventarisieren Blueprints Nicht-Tabellenobjekte unabhängig von Zeilenstichproben. Die Voreinstellung `--artifact-detail summary` liest Objektkataloge, aber keine Definitionen, und gibt nur begrenzte Anzahlen sowie Klassen externer Voraussetzungen aus.

`--artifact-detail graph --yes` fügt anonyme Objekt-IDs und Abhängigkeitskanten hinzu. `--artifact-detail analyzed --yes` liest verfügbare Definitionen außerdem vorübergehend und gibt nur begrenzte lexikalische Merkmals- und Komplexitätsbänder aus. Definitionstext, Quellobjektnamen, Endpunkte, Providerzeichenfolgen, Principals, Geheimnisse, Schlüssel, Zertifikate, Paketnamen und Binärdateien werden niemals serialisiert.

Katalogrechte beeinflussen Aussagen über Abwesenheit. Prüfen Sie `visibility`, `inventory_complete`, `dependencies_complete`, `catalogs_unreadable` und `families_not_inventoried`; interpretieren Sie eine Nullanzahl bei offengelegter Lücke nicht als Beweis. `DBP1410W` kennzeichnet einen optionalen Artefaktkatalog, der nicht gelesen werden konnte.

Anonyme Abhängigkeitstopologie kann eine Anwendung dennoch identifizieren. Genehmigen Sie `graph` oder `analyzed` nur, wenn dieses Risiko akzeptabel ist. Siehe [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Tier 2: Komprimierungsmessung

Tier 2 wird ausschließlich durch das explizite Paar aktiviert:

```bash
--measure-compression --yes
```

Tier 2 liest zusätzlich begrenzte Zeilenstichproben in den Prozessspeicher. Die Stichprobenbytes werden in einen internen Row-Frame-Puffer codiert, lokal mit zstd auf Stufe 3 komprimiert, als gerundete Verhältnisse zusammengefasst und verworfen.

Die Stichprobenbytes werden:

- nicht in `blueprint.toml` geschrieben;
- nicht in das Auditprotokoll geschrieben;
- nicht in temporäre Dateien geschrieben;
- außer über die Datenbankverbindung über kein Netzwerk gesendet;
- nach der Zusammenfassung der Stichprobe nicht aufbewahrt.

Tier 2 ist wertvoll, weil DBWarp-Leistung und Egress-Kosten von komprimierten Bytes und nicht von rohen Tabellenbytes abhängen.

## RTT-Prüfung

Standardmäßig führt das Werkzeug nach dem Verbindungsaufbau fünf `SELECT 1`-Abfragen aus. Dadurch wird ein `[network]`-Block mit `connect_total_ms`, `query_rtt_ms_p50` und `query_rtt_ms_p95` ausgegeben.

Die Prüfung hilft Bedienern zu verstehen, wo das Blueprint-Werkzeug im Verhältnis zur Quelldatenbank ausgeführt wurde. Sie misst nicht die WAN-RTT der Migration.

Deaktivieren Sie sie mit:

```bash
--no-rtt-probe
```

## Gelesene Dateien

Zur Laufzeit liest das Werkzeug ausschließlich Dateien, die ausdrücklich in der Befehlszeile angegeben wurden, etwa Passwortdateien, Benutzerdateien, TLS-CA-/Zertifikat-/Schlüsseldateien, Entra-Tokendateien oder eine Eingabedatei für `--from-toml`.

Es liest bewusst keine üblichen impliziten Speicherorte für Anmeldedaten wie `~/.pgpass`, `~/.my.cnf`, Cloud-Anmeldedateien, SSH-Schlüssel, Shell-Verläufe oder standardmäßige Passwort-Umgebungsvariablen.

Die vollständige Liste finden Sie in [`../AUDIT.md`](AUDIT.md).

## Geschriebene Dateien

Das Werkzeug schreibt ausschließlich in Pfade, die vom aktiven Modus ausgewählt werden:

- Blueprint-TOML unter `--out` im Live-Modus;
- `--deck`, falls angefordert;
- `--audit-log`, falls angefordert;
- `--out-dir` im Batchmodus: `bundle.toml`, `blueprints/`, `audits/`, eine
  Eigentumsmarkierung und `errors.txt`, wenn ein Teilfehler gemeldet werden muss;
- bei jedem Lauf ein Auditprotokoll auf stderr.

Es verwendet kein implizites temporäres Verzeichnis des Betriebssystems. Die
atomare Batchveröffentlichung kann neben `--out-dir` ein benachbartes Staging-
oder Wiederherstellungsverzeichnis anlegen. Bei einem abgefangenen Fehler wird
dieses Verzeichnis entfernt oder das vorherige Bundle wiederhergestellt.

## Prüfliste für die Ausgabe

Prüfen Sie vor der Weitergabe von `blueprint.toml`:

- der Header ist der feste Header `dbwarp-blueprint v6`;
- Tabellen-IDs sehen wie `table-001` aus;
- Spalten-IDs sehen wie `col-1` aus;
- Schema-IDs sehen wie `schema-A` aus;
- es sind keine echten Tabellen-, Spalten-, Index-, Schema- oder Benutzernamen enthalten;
- keine Namen von Nicht-Tabellenobjekten, Definitionstexte, Endpunktzeichenfolgen, Anmeldedaten, Schlüssel-/Zertifikatsmaterial, Paketnamen oder Binärdateien vorhanden sind;
- es sind keine Zeilenwerte enthalten;
- numerische Werte sind wie in [`../FORMAT.md`](FORMAT.md) dokumentiert gerundet;
- optionale Komprimierungsabschnitte enthalten ausschließlich Verhältnisse und Stichprobenmetadaten.
- Artefakt-Vollständigkeitsfelder gefilterte Sichtbarkeit, unlesbare Kataloge und bekannte unmodellierte Familien offenlegen.

Die standardmäßige, ausgewogene MySQL-Ausgabe enthält exakte deklarierte Kapazitäten und Indexpräfixlängen sowie relativ gerundete Durchschnitts-/p95-Stichproben. Prüfen Sie die drei Treuemarkierungen ausdrücklich. Wenn `--length-fidelity exact --yes` verwendet wurde, genehmigen Sie auch die exakten Stichprobenstatistiken. Zeilenwerte und echte Objektnamen müssen weiterhin fehlen. Fehlende Treuemarkierungen bedeuten Legacy-/unbekannte Daten und dürfen nicht als benchmarkfähige Metadaten behandelt werden.

Die Markierung behauptet nicht, dass die Stichprobe jede Tabelle erfasst hat. Eine Benchmark-Übergabe muss im Estimator-Manifest außerdem null nichtleere, variabel breite indizierte Spalten ohne Stichprobe ausweisen; erhöhen Sie `--max-wall-secs` und erfassen Sie den Blueprint erneut, wenn diese Prüfung fehlschlägt.

## Betriebssicherheit

Empfohlener erster Lauf:

```bash
--sample-rows 500 --max-wall-secs 120
```

Empfohlener produktionsnaher Lauf nach der Genehmigung:

```bash
--sample-rows 1000 --max-wall-secs 300
```

Führen Sie das Werkzeug auf einer Lesereplik aus, wenn die Produktionsrichtlinie Stichproben auf dem Primärsystem verbietet.
