# Inventar von Nicht-Tabellenartefakten

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../ARTIFACT_INVENTORY.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../ARTIFACT_INVENTORY.md) | **Deutsch** |
[Français](../fr/ARTIFACT_INVENTORY.md) | [Español](../es/ARTIFACT_INVENTORY.md) |
[Polski](../pl/ARTIFACT_INVENTORY.md) | [日本語](../ja/ARTIFACT_INVENTORY.md) |
[简体中文](../zh/ARTIFACT_INVENTORY.md)

Seit Schema v4 können Blueprints Datenbankobjekte außerhalb von Tabellen und
Voraussetzungen für die Bereitstellung beschreiben, ohne deren Quellnamen,
Definitionen, Endpunktzeichenfolgen, Geheimnisse, Zertifikate, Schlüssel oder
Binärdateien zu veröffentlichen. Das Inventar hilft DBWarp, die Migrationskomplexität zu
bewerten und Arbeiten zu erkennen, die Pakete, Infrastruktur,
Sicherheitsfreigaben oder unterstützte Konvertierung benötigen.

Ein Inventareintrag ist keine Fähigkeitszusage. Ein gemeldetes Objekt bedeutet
nicht, dass DBWarp es automatisch neu erstellen oder übersetzen kann. Die
Migrationsfähigkeit muss separat anhand der Routen- und Artefaktmatrix von
DBWarp geprüft werden.

## Detailstufen

Mit `--artifact-detail` wählen Sie den Kompromiss zwischen Datenschutz und
Planung:

| Wert | Datenbankzugriffe | Ausgabe in der Blueprint-Datei | Zustimmung |
|---|---|---|---|
| `none` | Keine Artefaktkataloge oder Definitionen | Keine Artefaktzahlen und kein Graph | Keine zusätzliche Zustimmung |
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

- stabile anonyme IDs wie `view-001`, `function-002` und `schema-A`;
- geschlossene Tokens für Objektart, Unterart, Ebene, Sichtbarkeit und Sicherheitsmodus;
- Abhängigkeiten ausschließlich über anonyme Artefakt- oder Tabellen-IDs;
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
| `contract` | Unabhängig versionierter Vertrag, derzeit `dbwarp-blueprint-artifacts/v1` |
| `detail` | Angeforderte Detailstufe |
| `visibility` | `full`, `privilege_filtered` oder `unknown` |
| `inventory_complete` | Nur bei voller Sichtbarkeit, ohne unlesbare Kataloge und ohne deklarierte unmodellierte Familien wahr |
| `dependencies_complete` | Nur wahr, wenn Abhängigkeitsquellen lesbar waren und die modellierten Familien erfasst werden können |
| `analysis_complete` | Nur bei `analyzed` und vollständiger Analyse aller verfügbaren Definitionen wahr |
| `catalogs_read` | Erfolgreich gelesene Standardkatalogfamilien |
| `catalogs_unreadable` | Fehlgeschlagene oder nicht verfügbare Katalogfamilien |
| `families_not_inventoried` | Bekannte Objektfamilien außerhalb des aktuellen Erfassungsvertrags |

Ein optionaler Katalogfehler entfernt Objekte nicht stillschweigend. Der Lauf
meldet `DBP1410W`, zeichnet den betroffenen Katalog auf und setzt die passenden
Vollständigkeitsangaben auf falsch. Ein Konto mit geringen Rechten kann daher
ein nützliches Teilinventar erzeugen, ohne Abwesenheit als Beweis darzustellen.

## Engine-Abdeckung

Der v1-Collector modelliert folgende Familien:

| Engine | Modellierte Objektfamilien |
|---|---|
| PostgreSQL | Views, materialisierte Views, Sequenzen, Routinen, Aggregate, Enum-/Domain-/Composite-/Range-Typen, Trigger, Defaults, Checks, Policies, Regeln, Event-Trigger, Erweiterungen, Fremdtabellen/-server, Publikationen, Subskriptionen, Tablespaces und native Funktionen |
| MySQL | Views, gespeicherte Funktionen und Prozeduren, Trigger, geplante Events, View-Abhängigkeiten, FEDERATED-Tabellen und registrierte ladbare UDFs |
| SQL Server | Views, gespeicherte Prozeduren, skalare/Tabellenfunktionen, CLR-Module, Trigger, Defaults, Checks, Regeln, Synonyme, Sequenzen, benutzerdefinierte Typen, CLR-Assemblies, externe Datenobjekte, Volltextkataloge, Partitionierungsobjekte, Nicht-PRIMARY-Dateigruppen, Zertifikate, Schlüssel, datenbankbezogene Anmeldedaten, Linked Server und SQL-Server-Agent-Jobs |

Jede Blueprint-Datei nennt bekannte unmodellierte Familien. Aus einer leeren Zahl
darf nur dann auf Abwesenheit geschlossen werden, wenn `visibility`, die
Vollständigkeitsfelder und die Liste unmodellierter Familien dies stützen.

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

`analyzed` ergänzt Blöcke nach `dbwarp-language-feature-census/v1` für
verfügbare SQL- und Prozedurdefinitionen. Der erste Analyzer ist `lexical-v1`
und meldet `status = "partial"`; er ist weder Parser noch Compiler, semantischer
Binder oder Erfolgsgarantie für eine Übersetzung.

Er speichert begrenzte Klassen für Definitionsgröße, Anweisungen, Tokens,
Verschachtelung, zyklomatische Komplexität und undurchsichtige/dynamische
Bereiche. Ein geschlossenes Vokabular beschreibt Kontrollfluss, Joins,
Unterabfragen, CTEs, Aggregate, Fenster, DML, DDL, temporäre Objekte,
dynamisches SQL, JSON, XML, räumliche und Vektortypen sowie Sicherheitsmodi.
Der Engine-Kontext enthält ein normalisiertes Grammatikprofil, MySQL-SQL-Modi
und bei SQL Server Kompatibilität, `ANSI_NULLS` und `QUOTED_IDENTIFIER`.

Der lexikalische Analyzer entfernt Kommentare, Literale und quotierte
Bezeichner. Kontextregeln behandeln Trigger-Ereignisse, PostgreSQL
`EXECUTE FUNCTION` und SQL-Server-Moduloptionen. Die Ergebnisse bleiben grobe
Planungsnachweise. Ein zukünftiger grammatikgestützter Analyzer kann eine neue
Analyzerversion verwenden, ohne den äußeren Artefaktvertrag zu ändern.

## Empfohlener Prüfablauf

1. `summary` zusammen mit der normalen Katalogprüfung ausführen.
2. Zahlen, externe Klassen, Sichtbarkeit, unlesbare Kataloge und unmodellierte Familien prüfen.
3. `graph` nur freigeben, wenn anonyme Abhängigkeitstopologie akzeptabel ist.
4. `analyzed` nur freigeben, wenn vorübergehende Definitionszugriffe akzeptabel sind.
5. Das Auditprotokoll lokal als zugriffsgeschützten Nachweis aufbewahren. Nur weitergeben, wenn ein namentlich benannter Empfänger die Endpunkt-, Identitäts-, Pfad- und Degradierungsdetails über einen genehmigten sicheren Kanal benötigt.
6. Inventar mit der DBWarp-Fähigkeitsmatrix vergleichen, bevor automatische Neuerstellung oder Übersetzung zugesagt wird.

Die exakten Felder beschreibt die [Formatreferenz](FORMAT.md). Laufzeitliche
Lese-/Schreibzugriffe, Warnungen und Vertrauensaussagen beschreibt die
[Auditreferenz](AUDIT.md).
