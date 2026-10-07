# Inventar von Nicht-Tabellenartefakten

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../ARTIFACT_INVENTORY.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../ARTIFACT_INVENTORY.md) | **Deutsch** |
[Français](../fr/ARTIFACT_INVENTORY.md) | [Español](../es/ARTIFACT_INVENTORY.md) |
[Polski](../pl/ARTIFACT_INVENTORY.md) | [日本語](../ja/ARTIFACT_INVENTORY.md) |
[简体中文](../zh/ARTIFACT_INVENTORY.md)

Blueprints können nicht-tabellarische Datenbankobjekte und Bereitstellungsanforderungen beschreiben, ohne deren Quellnamen, Definitionen, Endpunkt-Strings, Geheimnisse, Zertifikate, Schlüssel oder Binärdateien zu veröffentlichen. Dieses Inventar hilft DBWarp, die Komplexität der Migration abzuschätzen und Aufgaben zu identifizieren, für die Pakete, Infrastruktur, Sicherheitsfreigaben oder eine unterstützte Konvertierung erforderlich sind.

Ein Inventar ist keine Aussage über eine Fähigkeit. Die Tatsache, dass ein Objekt erfasst wird, bedeutet nicht, dass DBWarp es automatisch neu erstellen oder übersetzen kann. Bitte überprüfen Sie mit DBWarp, welche Objekttypen unterstützt werden.

## Detailstufen

Mit `--artifact-detail` wählen Sie den Kompromiss zwischen Datenschutz und
Planung:

| Wert | Datenbankzugriffe | Ausgabe in der Blueprint-Datei | Zustimmung |
|---|---|---|---|
| `none` | Keine Artefaktinventarkataloge oder Definitionen (die reine Topologie-Zählabfrage wird weiterhin ausgeführt) | Explizites, nicht angefordertes v7-Inventar; keine Zahlen und kein Graph | Keine zusätzliche Zustimmung |
| `summary` | Artefaktkataloge, aber keine Definitionen | Zahlen je Art und Klasse externer Voraussetzung | Standard; keine zusätzliche Zustimmung |
| `graph` | Artefaktkataloge und Abhängigkeitsmetadaten, aber keine Definitionen | Zahlen sowie stabile anonyme Objektdatensätze und Kanten | Erfordert `--yes` |
| `analyzed` | Artefaktkataloge, Abhängigkeiten und verfügbare Definitionen | Graph sowie begrenzte Sprachmerkmale und Komplexitätsklassen | Erfordert `--yes` |

Standard ist `summary`. Verwenden Sie `none`, wenn die Richtlinie die
Tabellenstruktur erlaubt, aber Nicht-Tabellenkataloge verbietet. Verwenden Sie
`graph` für eine abhängigkeitssensitive Planung ohne Definitionszugriff und
`analyzed` nur nach Freigabe des vorübergehenden Definitionszugriffs.

```bash
./dbwarp-blueprint \
  --connect postgresql://blueprint_user@db.internal/appdb \
  --password-file /etc/dbwarp/blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --artifact-detail analyzed \
  --out appdb.blueprint.toml \
  --audit-log appdb.blueprint.audit.txt \
  --yes
```

## Datenschutzvertrag

Die Artefaktausgabe enthält nur begrenzte Metadaten aus geschlossenem Vokabular:

- innerhalb eines Laufs konsistente anonyme IDs wie `view-001`, `function-002`
  und `schema-A`; laufübergreifende Stabilität erfordert dieselbe geschützte
  `--anonymization-key-file`;
- geschlossene Tokens für Objektart, Unterart, Ebene, Sichtbarkeit und Sicherheitsmodus;
- typisierte Beziehungen ausschließlich über anonyme Artefakt- oder Tabellen-IDs, mit geschlossenen Evidenz- und Auflösungsgrund-Tokens;
- begrenzte, Engine-spezifische Funktionsanforderungen für die Migrationsplanung;
- Zahlen und begrenzte Klassen statt frei formuliertem Text;
- Standardkatalogbezeichnungen wie `pg_proc`, `information_schema.views` oder `sys.objects`;
- Klassen externer Voraussetzungen, niemals deren Namen oder Material.

Nicht enthalten sind Quellobjektnamen, SQL- oder Prozedurquelltext,
Schemanamen, Principals, Endpunktzeichenfolgen, Providerzeichenfolgen,
Anmeldedaten, Schlüsselmaterial, Zertifikatskörper, Assembly-Dateien,
Erweiterungspaketnamen oder Namen ladbarer Bibliotheken.

Im Modus `analyzed` bleiben Definitionen nur so lange im Speicher, wie
Kommentare und Literale entfernt und begrenzte lexikalische Aggregate erzeugt
werden. Sie liegen in einem bei Freigabe überschriebenen Besitzer und werden
weder serialisiert noch protokolliert noch an einen anderen Dienst gesendet.
Dies minimiert Prozessspeicher, behauptet aber nicht, dass Betriebssystem-Paging
oder ein privilegierter Debugger unmöglich ist.

Auch anonyme Graphen können eine Anwendung über Anzahl und Topologie
wiedererkennbar machen. Deshalb schlagen `graph` und `analyzed` mit `DBP1014E`
fehl, wenn der Operator nicht `--yes` angibt.

## Vollständigkeitsnachweise

Der Block `[artifact_inventory]` ist bewusst selbstprüfend:

| Feld | Bedeutung |
|---|---|
| `contract` | Unabhängig versionierter Vertrag; v7 verwendet `dbwarp-blueprint-artifacts/v2`, ältere Blueprint-Schemas behalten v1 |
| `detail` | Angeforderte Detailstufe |
| `scope` | v7-Katalogumfang: `all-visible-schemas`, `selected-schemas`, `structured-source` oder `unknown` |
| `visibility` | `full`, `privilege_filtered` oder `unknown` |
| `inventory_complete` | Nur bei voller Sichtbarkeit, ohne unlesbare Kataloge und ohne deklarierte unmodellierte Familien wahr |
| `dependencies_complete` | Nur wahr, wenn Abhängigkeitsquellen lesbar waren und die modellierten Familien erfasst werden können |
| `requirements_complete` | V7-Aggregat: nur nach Prüfung von Engine-Version und Edition, vollständiger Abdeckung der Bewertungspopulation im ausgewählten Umfang und `requirement_status = complete | not_applicable` für jedes ausgegebene Artefakt wahr; Weglassen bedeutet falsch |
| `analysis_complete` | Nur bei `analyzed` und vollständiger Analyse aller verfügbaren Definitionen wahr |
| `catalogs_read` | Erfolgreich gelesene Standardkatalogfamilien |
| `catalogs_unreadable` | Fehlgeschlagene oder nicht verfügbare Katalogfamilien; betroffene Vollständigkeitsangaben werden herabgestuft, ohne unabhängige Anforderungsnachweise pro Objekt zu löschen |
| `catalogs_not_applicable` | Nachweislich nicht anwendbare Katalogfamilien; überschneidungsfrei mit den lesbaren und unlesbaren Mengen |
| `families_not_inventoried` | Bekannte Objektfamilien, die in dieser Version nicht inventarisiert werden |

Ein optionaler Katalogfehler entfernt Objekte nicht stillschweigend. Der Lauf
meldet `DBP1410W`, zeichnet den betroffenen Katalog auf und setzt die passenden
Vollständigkeitsangaben auf falsch. Ein Konto mit geringen Rechten kann daher
ein nützliches Teilinventar erzeugen, ohne Abwesenheit als Beweis darzustellen.

`object_count` zählt ausgegebene Artefaktdatensätze, nicht Zeilen eines einzelnen
nativen Katalogs. Oracle-Pakete und Objekttypen werden als Spezifikation,
Hauptteil und Mitgliedsdatensätze modelliert; der Hauptteil besitzt die
kombinierte Sprachanalyse. Ein in mehreren Katalogen beobachtetes Objekt wird
nur einmal gezählt. Trigger-Metadaten und -Quelltext werden beispielsweise vor
der Anonymisierung anhand der nativen Objektidentität verbunden.

## Vertrag für aggregierte Komplexität

Schema v7 definiert für `graph` und `analyzed` einen rein aggregierten Datensatz
`[artifact_inventory.complexity]`. Er verursacht weder zusätzliche
Datenbankzugriffe noch zusätzliche Berechtigungen, sondern wird aus dem bereits
genehmigten anonymen Graphen und Sprachzensus abgeleitet. Für `graph` und
`analyzed` ist er erforderlich, bei `none` und `summary` fehlt er.

Die Bewertung umfasst sieben abgeschlossene Dimensionen: Volumen, Kontrollfluss, Funktionsumfang, Verflechtung, Umgebungsabhängigkeit, Undurchsichtigkeit und Dialektkopplung. Die Ergebnisse werden in Form von Bereichen angegeben, nicht als numerische Werte. `overall_score` ist reserviert und wird nicht befüllt, da ein Wert zwischen 0 und 100 eine nicht unterstützte Genauigkeit implizieren würde.

Jede Dimension enthält ein eindimensionales exaktes Histogramm der geeigneten
Population. Umfang verwendet Größenklassen, die übrigen Dimensionen
Zählklassen. Beide besitzen `not_applicable`- und `unknown`-Eimer; jedes
Histogramm erfüllt `eligible = assessed + not_applicable + unknown`. Es gibt
weder zusammengesetzte Bewertungen pro Objekt noch Kreuztabellen nach Objektart,
Merkmal oder Schema. Positiv erkannte Engine-generierte und sekundäre Objekte
werden von der Bewertung ausgeschlossen, bleiben aber im Inventar; temporäre
Objekte und Objekte mit fehlenden Flags bleiben geeignet. Externe Binärartefakte
wie am Standort installierte Plug-ins, CLR-Assemblies, Java und Bibliotheken bleiben echte
Migrationsarbeit. Nur ein ausdrücklich gesetztes Engine-generiert-Flag schließt
sie aus.

`assessment_population_complete` gibt an, ob jedes geeignete Objekt bekannt ist.
Es ist unabhängig von `inventory_complete`; Weglassen bedeutet falsch. Eine
unvollständige Population erzwingt eine `unknown`-Gesamtklasse, sofern die
bekannte Untergrenze nicht bereits `very-high` ist.

Die Abdeckung wird je Dimension erfasst. `not_applicable` ist eine abgeschlossene
Bewertung. Teilweise bewertet bedeutet, dass mindestens eine anwendbare
Dimension bekannt und eine weitere unbekannt ist; unbewertet bedeutet, dass
keine anwendbare Dimension bekannt ist. Unbekannte Evidenz gilt niemals als
geringe Komplexität. Eine teilweise Dimension ist `unknown`, außer ihre bekannte
Untergrenze ist bereits `very-high`; die Gesamtklasse wird nur ausgegeben, wenn
Unter- und Obergrenze übereinstimmen. `graph` gibt für eine nicht leere
Population nie ein Gesamturteil aus, weil Definitionen nicht gelesen wurden.
Eine vollständige leere Population ist `not-applicable`.

Verpackte Objekte tragen zum Histogramm der Undurchsichtigkeit zum Bucket `unknown` bei, auch wenn keine teilweise Sprachstatistik erstellt werden kann. Lesen Sie die Undurchsichtigkeitsbande zusammen mit ihrer Abdeckung, sodass eine kleine beobachtete Bande nicht ohne ihre unbekannte Population gelesen wird. Wenn die Bewertung selbst fehlschlägt, wird das vollständige Inventar mit einem kanonischen, vollständig unbekannten Aggregat beibehalten. Verpackte oder zurückgehaltene Definitionen, unvollständige Graphen, unvollständige Nachweise für Anforderungen, Grenzen mit ausgewähltem Umfang sowie nicht unterstützte Sprachen oder Dialekte bleiben explizite Einschränkungen. Die Einschränkungsbehauptungen werden, wenn möglich, aus den Artefakten und den statistischen Daten abgeleitet. `unsupported-dialect` bleibt eindeutig, da die Statistik einen Dialekt nennen und `unavailable` melden kann, aber keinen `unsupported`-Status hat; das bedeutet, dass die Definition gelesen wurde, aber der angegebene Analysator diesen Dialekt nicht unterstützt, nicht dass die Quelle zurückgehalten oder verpackt wurde.

Der Datensatz enthält die einzelne Analyzer-Version sowie sortierte Mengen von Analysebereichen, Dialekten und Grammatikprofilen, die in der zulässigen Stichprobe vorhanden sind. Zwei Erfassungen sind nur dann vergleichbar, wenn diese Mengen, der Vertrag, der Gutachter, der Umfang und die Stichprobenrichtlinie übereinstimmen. Ein Bundle behält die Komplexität pro Quellsystem bei und aggregiert sie niemals über Engines oder Analyzer hinweg.

Die Komplexitäts-Vertrags- und Bewertungsversionen sind unabhängig voneinander. Für genaue Felder und Invarianten, siehe die [Formatreferenz](FORMAT.md).

## Engine-Abdeckung

Der aktuelle Collector modelliert folgende Familien:

| Engine | Modellierte Objektfamilien |
|---|---|
| PostgreSQL | Views, materialisierte Views, Sequenzen, Routinen, Aggregate, Enum-/Domain-/Composite-/Range-Typen, Trigger, Defaults, Checks, Policies, Regeln, Event-Trigger, Erweiterungen, Fremdtabellen/-server, Publikationen, Subskriptionen, Tablespaces und native Funktionen |
| MySQL | Views, gespeicherte Funktionen und Prozeduren, Trigger, geplante Events, View-Abhängigkeiten, FEDERATED-Tabellen und registrierte ladbare UDFs |
| SQL Server | Views, gespeicherte Prozeduren, skalare/Tabellenfunktionen, CLR-Module, Trigger, Defaults, Checks, Regeln, Synonyme, Sequenzen, benutzerdefinierte Typen, CLR-Assemblies, externe Datenobjekte, Volltextkataloge, Partitionierungsobjekte, Nicht-PRIMARY-Dateigruppen, Zertifikate, Schlüssel, datenbankbezogene Anmeldedaten, Linked Server und SQL-Server-Agent-Jobs |

Jede Blueprint-Datei nennt bekannte unmodellierte Familien. Aus einer leeren Zahl
darf nur dann auf Abwesenheit geschlossen werden, wenn `visibility`, die
Vollständigkeitsfelder und die Liste unmodellierter Familien dies stützen.

## Anforderungsnachweise

Anforderungen an Artefakte sind Engine-spezifische Informationen, die aus begrenzten Katalogspalten oder dedizierten, engine-sensitiven Syntaxprüfungen stammen. Eine generische lexikalische Analyse erzeugt keine engine-spezifischen Anforderungen. Wenn ein analysiertes Sprachmerkmal denselben Fakt widerspiegelt, hat die Anforderung Vorrang und das Merkmal bleibt eine lexikalische Beobachtung. Eine fehlende Anforderung ist kein Beweis dafür, dass jedes Anforderungselement überprüft wurde.

Jedes v7 graph/analyzed-Objekt speichert `requirement_status` als `complete`, `partial`, `unavailable` oder `not_applicable`. Nur `complete` macht eine leere Liste zu einem Beweis für null Anforderungen für dieses Objekt. `partial` protokolliert, dass ein bestimmtes Faktum oder ein bestimmter Produzent erfolgreich war, ohne vollständige Abdeckung; `unavailable` protokolliert, dass kein Produzent eine brauchbare Abdeckung hergestellt hat und daher keine bekannten Anforderungen oder externen Voraussetzungen belegen kann. Solche Beweise erfordern `partial`. Beide tragen zu unbekannten, anforderungen-bedingten Komplexitätsbeobachtungen bei, während vollständige Objekte weiterhin bewertet werden können. `not_applicable` verbietet die Aufzeichnung von Anforderungen und externen Voraussetzungen. Der inventarbezogene Wert `requirements_complete` ist nur dann wahr, nachdem die Engine-Version und -Edition geprüft wurden, eine vollständige Bewertung durchgeführt wurde und jedes ausgegebene Objekt vollständig oder nicht anwendbar ist. PostgreSQL, MySQL und SQL Server setzen das Gesamtergebnis erst, nachdem jedes anwendbare Artefakt-Katalog versucht wurde und die ausgewählte Population als vollständig bewiesen wurde. Ein verweigerter oder unlesbarer Katalog oder eine Auswahlgrenze, deren Population nicht bewiesen werden kann, lässt das Gesamtergebnis falsch, ohne vollständige, objektspezifische Beweise aus den gelesenen Katalogen zu löschen. Artefaktanforderungen werden für Oracle nicht gemeldet.

## Externe Voraussetzungen

Objekte, die mehr als portables Tabellen-DDL benötigen, erhalten eine anonyme
Klasse externer Voraussetzungen:

| Klasse | Vom Operator zu klären |
|---|---|
| `postgresql_extension` | Kompatibles Erweiterungspaket und Zielversion |
| `postgresql_native_function` | Native Bibliothek und ABI-Kompatibilität |
| `mysql_loadable_udf` | Ladbare UDF-Binärdatei und ABI-Annahmen des Quellservers |
| `sqlserver_clr_assembly` | CLR-Aktivierung, Assembly, Runtime und Vertrauensrichtlinie |
| `foreign_endpoint` | Netzwerk, Provider, entfernte Datenbank und Authentifizierung |
| `replication_topology` | Publikations-/Subskriptionstopologie und Zielrichtlinie |
| `physical_storage` | Dateigruppen- oder Platzierungsdesign |
| `server_feature` | Verfügbarkeit einer Server- oder Managed-Service-Funktion |
| `certificate_material` | Zertifikatsausstellung oder -import nach Zielrichtlinie |
| `encryption_or_credential_material` | Schlüssel, Anmeldedaten, externer Schlüsselspeicher und Geheimnisverwaltung |
| `sqlserver_agent` | Agent-Verfügbarkeit, Betriebsumgebung und Job-Governance |

Die Blueprint-Datei vermerkt, ob Binär-, Geheimnis- oder Endpunktmaterial benötigt,
aber nicht erfasst wird. Externe Objekte müssen explizite Migrationsaufgaben
werden und dürfen nicht stillschweigend ausgelassen werden.

## Zensus der Sprachmerkmale

`analyzed` Details fügen `dbwarp-language-feature-census/v1` Blöcke hinzu. Das Schema v7 erzeugt `lexical-v2`, das nur den ausführbaren oder deklarativen Teil analysiert und `analysis_span = "executable-body"` erfasst. Es schließt die äußere Erstellungsstruktur, die Identität, die Signatur, die Rückgabedeklaration und die Moduloptionen aus. Wenn der Teil nicht sicher isoliert werden kann, zeichnet der Sammler einen unbekannten Bereich und keine verfügbaren Beweise auf; Objekte ohne Definitionsdimension verwenden `not-applicable`. Ein weggelassener Bereich wird als unbekannt interpretiert, anstatt aus der Engine abgeleitet zu werden. Der Analysator meldet `status = "partial"` für unterstützte Definitionen, da er kein Parser, Compiler, semantischer Binder oder eine Garantie für eine erfolgreiche Übersetzung ist. Fehlende oder nicht unterstützte Definitionselemente sind `unavailable`, während eine bewiesenermaßen nicht anwendbare Analyse `not_applicable` ist.

Er speichert begrenzte Klassen für Definitionsgröße, Anweisungen, Tokens,
Verschachtelung, zyklomatische Komplexität und undurchsichtige/dynamische
Bereiche. Ein geschlossenes Vokabular beschreibt Kontrollfluss, Joins,
Unterabfragen, CTEs, Aggregate, Fenster, DML, DDL, temporäre Objekte,
dynamisches SQL, JSON, XML, räumliche und Vektortypen, ausgelöste Fehler,
Transaktionssteuerung, Ref-Cursor, verankerte Typen, Intervall-, Zeitzonen-,
Boolean- und LOB-Nutzung sowie Sicherheitsmodi.
Der Engine-Kontext enthält ein normalisiertes Grammatikprofil, MySQL-SQL-Modi
und bei SQL Server Kompatibilität, `ANSI_NULLS` und `QUOTED_IDENTIFIER`.

Der lexikalische Analysator entfernt Kommentare, in Anführungszeichen gesetzte Literale und in Anführungszeichen gesetzte Bezeichner, bevor er zählt. Er verfügt über Kontextregeln für Trigger-Ereignisdeklarationen, PostgreSQL `EXECUTE FUNCTION` und SQL Server-Moduloptionen. Dennoch bleiben alle Ergebnisse grobe Planungsnachweise. Wrapped PL/SQL wird abgelehnt; verschleierte Bytes werden niemals zu plausiblen Messwerten des Programmkörpers.

## Empfohlener Prüfablauf

1. Die standardmäßige Stufe `summary` mit einer Artefaktkatalogprüfung
   ausführen. Wenn die Richtlinie nur Tabellenkataloge erlaubt, stattdessen
   `--artifact-detail none` verwenden; v7 zeichnet diese Entscheidung explizit
   auf, statt den Inventarstatus auszulassen.
2. Zahlen, externe Klassen, Sichtbarkeit, unlesbare Kataloge und unmodellierte Familien prüfen.
3. `graph` nur freigeben, wenn anonyme Abhängigkeitstopologie akzeptabel ist.
4. `analyzed` nur freigeben, wenn vorübergehende Definitionszugriffe akzeptabel sind.
5. Das Auditprotokoll lokal als zugriffsgeschützten Nachweis aufbewahren. Nur weitergeben, wenn ein namentlich benannter Empfänger die Endpunkt-, Identitäts-, Pfad- und Degradierungsdetails über einen genehmigten sicheren Kanal benötigt.
6. Nehmen Sie nicht an, dass ein inventarisiertes Objekt automatisch neu erstellt oder übersetzt werden kann; bestätigen Sie dies mit DBWarp.

Die genauen serialisierten Felder finden Sie in der [Formatreferenz](FORMAT.md). Laufzeit-Lese- und Schreibvorgänge, Warnungen und Vertrauensaussagen sind in der [Audit-Referenz](AUDIT.md) beschrieben.
