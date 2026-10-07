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
- keine DDL-Berechtigungen, außer die Prüfung genehmigt ausdrücklich die
  erweiterte MySQL-Erfassung, deren Metadatenberechtigungen `TRIGGER` und
  `EVENT` DDL-fähig sind;
- keine Superuser-/Administratorrolle;
- Lesezugriff ist auf die zu prüfende Datenbank begrenzt;
- Passwort oder Token wird per Datei oder Eingabeaufforderung bereitgestellt und nicht in die URI eingebettet.

Die genauen Berechtigungen variieren je nach Datenbank-Engine und Ihrer Richtlinie. Wenn das Konto einige Katalogansichten nicht lesen oder einige Tabellen nicht abfragen kann, schlägt das Tool deutlich fehl oder erzeugt eine reduzierte Blueprint; behalten Sie das Audit-Protokoll.

Verwenden Sie die versionsabhängigen Skripte und Hinweise in
[`../../sql/grants/README.md`](../../sql/grants/README.md). Entfernen Sie das
dedizierte Erfassungskonto nach der genehmigten Erfassung mit dem passenden
Skript unter `sql/revoke/`; prüfen Sie vor der Ausführung genau die Ziele für
Datenbank, Hostmuster, Rolle und Login.

## Tier 1: Nur Metadaten (keine Zeilenstichprobe)

Tier 1 ist die Standardeinstellung, wenn `--measure-compression` fehlt.

Es liest:

- Engine-Version;
- Tabellenliste und anonymisierte Eingaben für die Sortierung;
- ungefähre Zeilenzahlen;
- Tabellen- und Indexgrößen;
- Spaltentypfamilien, NULL-Zulässigkeit und gerundete Längenstatistiken, soweit verfügbar;
- Indextyp, Eindeutigkeit und anonymisierte Spaltenordnungsnummern;
- Struktur des Fremdschlüsselgraphen, soweit verfügbar;
- bestmögliche grobe Quellkapazitätsbänder, die der Datenbankendpunkt zurückgibt;
- begrenzte Anzahlen von Nicht-Tabellenobjekten und externen Voraussetzungen
  aus Objektkatalogen unter der Voreinstellung `--artifact-detail summary`
  (keine Definitionen);
- optionaler RTT-Test, es sei denn, `--no-rtt-probe` ist aktiviert.

Es liest keine Zeilenwerte.

## Quellumgebung

Der schema-v7-Block `[source_environment]` wird ausschließlich aus Werten
abgeleitet, die über die ausgewählte Datenbankverbindung zurückgegeben werden.
Der Collector untersucht niemals seinen eigenen Host oder stellt diese
Arbeitsstation als Datenbankserver dar.

PostgreSQL und MySQL legen eine Datenbank-Puffer-Einstellung offen, die unterhalb der normalen Mindestberechtigungen liegt, sodass der Speicher nur teilweise ein Beweis ist, basierend auf `database-buffer-cache`, und die CPU-Auslastung bleibt unbekannt.

SQL Server fordert nur die Ressourcen der Quellumgebung mit `--artifact-detail graph` oder `analyzed`, den erweiterten Modi, an. Die Basis- und Standardmodi führen keine Abfrage der Betriebssystemressourcen durch und protokollieren die Ressourcenbereiche als `not-requested`.

Das verbesserte Skript gewährt die erforderliche serverweite `VIEW SERVER STATE` (2019) oder `VIEW SERVER PERFORMANCE STATE` (2022/2025) in einem separaten Batch, den ein Datenbankadministrator entfernen kann. Wenn eine verbesserte Erfassung die DMV nicht lesen kann, wird die Erfassung fortgesetzt und der Katalog als nicht lesbar protokolliert, anstatt lokale Maschinenwerte zu verwenden oder Kapazitäten zu erfinden.

Dieser Erfassungspfad kontaktiert keine Cloud-, Kubernetes-, Hypervisor- oder
Betriebssystem-API.

## Inventar der Nicht-Tabellenartefakte

Blueprints inventarisieren nicht-tabellarische Objekte unabhängig von der Zeilenabfrage. Der Standardmodus `--artifact-detail summary` liest Objektkataloge, aber nicht Definitionen, und gibt nur begrenzte Zählungen und externe Abhängigkeitsklassen aus.

`--artifact-detail graph --yes` fügt anonyme Objekt-IDs und Abhängigkeitskanten hinzu. `--artifact-detail analyzed --yes` liest verfügbare Definitionen außerdem vorübergehend und gibt nur begrenzte lexikalische Merkmals- und Komplexitätsbänder aus. Definitionstext, Quellobjektnamen, Endpunkte, Providerzeichenfolgen, Principals, Geheimnisse, Schlüssel, Zertifikate, Paketnamen und Binärdateien werden niemals serialisiert.

Katalogrechte beeinflussen Aussagen über Abwesenheit. Prüfen Sie `visibility`,
`inventory_complete`, `dependencies_complete`, `requirements_complete`,
`catalogs_unreadable` und `families_not_inventoried`; interpretieren Sie eine
Nullanzahl oder eine leere Anforderungsliste bei offengelegter Lücke nicht als
Beweis. Prüfen Sie bei der Detailstufe graph/analyzed außerdem den
`requirement_status` jedes Objekts: Nur `complete` macht eine leere Liste zum
Beleg für null Anforderungen an dieses Objekt. `partial` erhält bekannte Fakten,
ohne vollständige Abdeckung zu behaupten; `unavailable` bedeutet, dass keine
nutzbare Abdeckung festgestellt wurde. In beiden Fällen bleibt die aus den
Anforderungen abgeleitete Kopplungsbewertung des Objekts unbekannt. `DBP1410W`
kennzeichnet einen optionalen Artefaktkatalog, der nicht gelesen werden konnte.

Anonyme Abhängigkeitstopologie kann eine Anwendung dennoch identifizieren. Genehmigen Sie `graph` oder `analyzed` nur, wenn dieses Risiko akzeptabel ist. Siehe [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Tier 2: Komprimierungsmessung

Tier 2 wird ausschließlich durch das explizite Paar aktiviert:

```bash
--measure-compression --yes
```

Stufe 2 liest zusätzlich begrenzte Zeilenmuster in den Arbeitsspeicher. Die abgetasteten Bytes werden in einen In-Memory-Puffer codiert und verwendet, um aggregierte Kompressions-, Null-Dichte-, cardinality/frequency-Werte, Längen- und Stilmessungen abzuleiten, bevor die Werte und temporären Fingerabdrücke verworfen werden.

Die Stichprobenbytes werden:

- nicht in `blueprint.toml` geschrieben;
- nicht in das Auditprotokoll geschrieben;
- nicht in temporäre Dateien geschrieben;
- außer über die Datenbankverbindung über kein Netzwerk gesendet;
- nach der Zusammenfassung der Stichprobe nicht aufbewahrt.

Stufe 2 ist wertvoll, weil die Übertragungszeit und die Ausgabekosten von den komprimierten Bytes und nicht von den rohen Tabellengrößen abhängen.

## RTT-Prüfung

Standardmäßig führt das Werkzeug nach dem Verbindungsaufbau fünf `SELECT 1`-Abfragen aus. Dadurch wird ein `[network]`-Block mit `connect_total_ms`, `query_rtt_ms_p50` und `query_rtt_ms_p95` ausgegeben.

Die Prüfung hilft Bedienern zu verstehen, wo das Blueprint-Werkzeug im Verhältnis zur Quelldatenbank ausgeführt wurde. Sie misst nicht die WAN-RTT der Migration.

Deaktivieren Sie sie mit:

```bash
--no-rtt-probe
```

## Gelesene Dateien

Zur Laufzeit liest das Werkzeug ausschließlich Dateien, die ausdrücklich in
der Befehlszeile ausgewählt oder von einem ausdrücklich ausgewählten
Batch-Manifest oder Bundle referenziert werden. Dazu können Passwort- und
Benutzerdateien, Anonymisierungsschlüsseldateien, TLS-CA-/Zertifikat-/
Schlüsseldateien, Entra-Tokendateien, Eingaben strukturierter Dateien sowie
Blueprint- oder Bundle-Eingaben gehören.

Es liest bewusst keine üblichen impliziten Speicherorte für Anmeldedaten wie `~/.pgpass`, `~/.my.cnf`, Cloud-Anmeldedateien, SSH-Schlüssel, Shell-Verläufe oder standardmäßige Passwort-Umgebungsvariablen.

Diese Aussage betrifft die von der Anwendung gesteuerte Suche nach
Anmeldedaten. Datenbank-, TLS-, DNS- und integrierte
Authentifizierungsbibliotheken können Vertrauensspeicher, Konfiguration und
Anmeldedatencaches des Betriebssystems verwenden. Prüfen oder verfolgen Sie
diese Plattformabhängigkeiten separat, wenn die Hostrichtlinie dies verlangt.

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

- der Header ist der feste Header `dbwarp-blueprint v7`;
- Tabellen-IDs sehen wie `table-001` aus;
- Spalten-IDs sehen wie `col-1` aus;
- Schema-IDs sehen wie `schema-A` aus;
- es sind keine echten Tabellen-, Spalten-, Index-, Schema- oder Benutzernamen enthalten;
- keine Namen von Nicht-Tabellenobjekten, Definitionstexte, Endpunktzeichenfolgen, Anmeldedaten, Schlüssel-/Zertifikatsmaterial, Paketnamen oder Binärdateien vorhanden sind;
- es sind keine Zeilenwerte enthalten;
- numerische Werte verwenden die in [`../FORMAT.md`](FORMAT.md) dokumentierte
  exakte oder gerundete Genauigkeit; exakte Opt-in-Felder sind als sensibler zu prüfen;
- optionale aus Stichproben abgeleitete Abschnitte enthalten aggregierte
  Komprimierungs-, NULL-Dichte-, Kardinalitäts-/Häufigkeits-, Längen-, Stil-
  und Stichprobenherkunftsmetadaten, niemals beprobte Werte.
- Artefakt-Vollständigkeitsfelder gefilterte Sichtbarkeit, unlesbare Kataloge und bekannte unmodellierte Familien offenlegen.

Die standardmäßige, ausgewogene MySQL-Ausgabe enthält die exakt deklarierten Kapazitäten und Indexpräfixlängen sowie relativ gerundete Durchschnitts- und p95-Werte. Überprüfen Sie die drei Genauigkeitsmarker explizit. Wenn `--length-fidelity exact --yes` verwendet wurde, genehmigen Sie auch die exakten Stichprobenstatistiken. Zeilenwerte und echte Objektnamen müssen weiterhin fehlen. Eine Blueprint-Datei ohne Genauigkeitsmarker wurde von einer älteren Version erstellt; erfassen Sie diese erneut.

Der Marker besagt nicht, dass die Stichproben alle Tabellen abgedeckt haben. Wenn `DBP1406W` gemeldet wird, erhöhen Sie `--max-wall-secs` und führen Sie die Stichproben erneut durch.

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
