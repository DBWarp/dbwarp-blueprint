# Schnellstart

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../QUICKSTART.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../QUICKSTART.md) | **Deutsch** | [Français](../fr/QUICKSTART.md) | [Español](../es/QUICKSTART.md) | [Polski](../pl/QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | [中文](../zh/QUICKSTART.md)

Diese Schnellstartanleitung richtet sich an einen Datenbankadministrator oder einen Sicherheitsexperten, der eine gemeinsam nutzbare DBWarp Blueprint-Datei erstellen muss, ohne dabei Daten preiszugeben.

## 1. Ausführungsweg auswählen

Verwenden Sie einen der folgenden Wege:

- Laden Sie eine Release-Binärdatei herunter und prüfen Sie ihre Prüfsumme.
- Erstellen Sie das Werkzeug mit `./build.sh` aus dem Quellcode.
- Erstellen Sie es aus dem gebündelten Release-Paket, wenn eine strenge Offline-Prüfung der Abhängigkeiten erforderlich ist.

Siehe [`../BUILD.md`](BUILD.md) und [`../binaries/README.md`](BINARIES.md).

Wählen Sie bei Bedarf ausdrücklich eine Anzeigesprache:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --schema app --dry-run
```

Unterstützte Werte sind `en`, `de`, `fr`, `es`, `pl`, `ja` und `zh`. Die Anzeigesprache ändert Hilfetexte, Eingabeaufforderungen, Diagnosen, Fortschrittstexte und Präsentationsprosa. Optionsnamen, zulässige Werte, URI-Schemata, Selektoren, DBP-Codes, Audit-Schlüssel oder Blueprint-TOML werden niemals geändert. Siehe [`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Ein dediziertes Konto mit minimalen Rechten bereitstellen

Führen Sie diesen Schritt vor jeder Live-Verbindung aus, auch vor
`--dry-run`-Beispielen, die später als Erfassung ausgeführt werden. Beginnen
Sie nicht mit einem Anwendungseigentümer-, Administrator-, Superuser-,
`root`-, `sa`- oder `db_owner`-Konto.

1. Bestimmen Sie die genaue Engine-Version, Datenbank und die genehmigten
   Schemas.
2. Wählen Sie die Erfassungsebene: `basic` nur für Tabellenkataloge, `standard` zum Hinzufügen einer begrenzten Zeilenstichprobe oder `enhanced` für die Analyse von Nicht-Tabellen-Objekten.
3. Lassen Sie den DBA das passende Skript unter `sql/grants/<engine>/`
   kopieren, alle markierten Werte für Datenbank, Schema, Principal, Passwort
   und Rollenumschalter bearbeiten und es im normalen Änderungsverfahren
   ausführen.
4. Verwenden Sie das dafür erstellte dedizierte Konto und übergeben Sie bei
   jedem Live-Befehl denselben genehmigten Umfang mit einer Option
   `--schema NAME` pro Schema.
5. Nachdem die Erfassung überprüft wurde, soll der Datenbankadministrator das entsprechende Skript unter `sql/revoke/` überprüfen und ausführen, um das Konto und die Berechtigungen zu entfernen.

Die Skripte unterscheiden bewusst zwischen exakt begrenzten Grants und
bequemen integrierten Rollen und erklären, wo eine Rolle weiter reicht. Die
ausführbaren Skripte stehen in
[`../../sql/grants/README.md`](../../sql/grants/README.md), die
versionsabhängige Begründung für DBA und Sicherheitsprüfung in
[`../../sql/grants/DATABASE_PERMISSIONS.md`](../../sql/grants/DATABASE_PERMISSIONS.md).
Der Collector selbst erstellt, erweitert oder entfernt keine
Datenbank-Principals.

## 3. Anmeldedaten sicher vorbereiten

Fügen Sie keine Passwörter in die Verbindungs-URI ein. Das Werkzeug lehnt in URIs eingebettete Passwörter ab, um Lecks über Prozesslisten und Shell-Verläufe zu vermeiden.

Bevorzugtes Muster für Passwortdateien (das Geheimnis wird ohne Anzeige eingegeben und erscheint nicht im Shell-Verlauf):

```bash
sudo install -d -m 700 -o "$USER" -g "$(id -gn)" /etc/dbwarp
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

Wenn der Benutzername umständlich URI-codiert werden müsste, legen Sie ihn ebenfalls in einer Datei ab:

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\migration_user' > /etc/dbwarp/db.user
```

Verwenden Sie danach `--user-file /etc/dbwarp/db.user`.

## 4. Zuerst einen Probelauf durchführen

Ein Probelauf validiert die Argumente und zeigt die geplante Aktion an, ohne eine Verbindung herzustellen:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

Im Präsentationsmodus `--from-toml` ist der Probelauf eine lokale Vorabprüfung und liest die Datenbank nicht.

Für mehrere Quellen sollten Sie stattdessen das Batch-Manifest im Testmodus ausführen:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 5. Reinen Katalogmodus ausführen

Der reine Katalogmodus liest Metadaten und Statistiken, aber keine Zeilenstichproben:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

Verwenden Sie diesen Modus, wenn eine Richtlinie Zeilenstichproben verbietet oder wenn Sie einen ersten Durchlauf für die Sicherheitsprüfung benötigen.

## 6. Details zu Nicht-Tabellenartefakten auswählen

Die Voreinstellung `--artifact-detail summary` liest Kataloge für Nicht-Tabellenobjekte, aber keine Objektdefinitionen. Sie gibt begrenzte Anzahlen und Klassen externer Voraussetzungen aus. Verwenden Sie `--artifact-detail none`, wenn eine Richtlinie diese Kataloge verbietet. Die reine Zählprüfung der Topologie wird dennoch ausgeführt; siehe [Berechtigungsreferenz](../../sql/grants/README.md#topology-evidence).

Für anonyme Abhängigkeitstopologie verwenden Sie `graph`; für begrenzte Sprachmerkmal- und Komplexitätsbänder `analyzed`. Beide erfordern ausdrückliche Zustimmung:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


Die Ausgabe enthält niemals Objektnamen, Definitionstext, Endpunkte, Geheimnisse, Schlüssel, Zertifikate oder Binärdateien. Lesen Sie [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md), bevor Sie den Modus graph oder analyzed genehmigen.

## 7. Tier-2-Komprimierungsmessung ausführen

Tier 2 liest begrenzte Zeilenstichproben in den Speicher, berechnet aggregierte
Messwerte für Komprimierung, NULL-Dichte, Kardinalität/Häufigkeit, Länge und Stil
und verwirft die Stichprobenwerte:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log blueprint.audit.txt
```

Verwenden Sie nach Möglichkeit Stufe 2. Sie liefert genauere Schätzungen für die Datenübertragungsgröße und die ausgehenden Kosten.

## 8. Präsentation erzeugen

Während des Live-Laufs:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --audit-log blueprint.audit.txt \
  --yes
```

Oder nach der Prüfung, ohne Datenbankverbindung:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

## 9. Vor der Weitergabe prüfen

Prüfen Sie:

```bash
less blueprint.toml
less blueprint.audit.txt
unzip -l blueprint.pptx  # optional deck package inspection
```

Erwartete Eigenschaften:

- keine echten Tabellennamen;
- keine echten Spaltennamen;
- keine Zeilenwerte;
- keine Kommentare außer dem festen Header;
- gerundete Zeilenzahlen und Bytegrößen;
- anonymisierte IDs wie `table-001`, `col-1` und `schema-A`;
- begrenzte Artefaktanzahlen und, nach Genehmigung, anonyme Artefakt-IDs;
- ausdrückliche Nachweise zu unvollständigen oder unlesbaren Artefakten statt stiller Auslassung;
- optionale aggregierte Messwerte für Komprimierung, NULL-Dichte,
  Kardinalität/Häufigkeit, Länge und Stil, niemals Stichprobenwerte.

## 10. Teilen Sie es mit DBWarp.

Minimum zum Teilen:

```text
blueprint.toml
```

Für mehrere Quellen sollten Sie ein zusammengefasstes Bundle erstellen und prüfen, anstatt das Arbeitsverzeichnis zu teilen:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Die Bundle-Metadaten enthalten die im Batch-Manifest gewählten Quell-IDs, Tags und Datensatzgruppen-IDs. Verwenden Sie anonyme Werte und prüfen Sie sie vor der Übertragung.

Siehe [Batch-Sammlung und Blueprint-Pakete](BATCH_AND_BUNDLES.md), wenn Sie mehrere Datenbanken oder mehrere Parquet- oder Avro-Datensätze haben, oder wenn Sie nur ausgewählte Quellen oder Tabellen freigeben möchten.

### Prüfen und teilen

Teilen Sie standardmäßig nur die geprüfte `blueprint.toml` oder das gepackte Bundle. Eine Präsentation darf nur nach gesonderter Prüfung ihres Inhalts und ihrer Vertraulichkeitskennzeichnung sowie ausdrücklicher Freigabe gemäß Ihrer Organisationsrichtlinie beigefügt werden.

Bewahren Sie Protokolle, Befehlsaufzeichnungen und nicht genehmigte Präsentationen lokal und mit eingeschränktem Zugriff auf. Sie können Endpunkte, authentifizierte Benutzer, lokale Pfade, Zeitdaten und Manifest-Identifikatoren enthalten. Senden Sie diese nur für einen bestimmten Support-Zweck über einen genehmigten, sicheren Kanal. Fügen Sie niemals Passwortdateien, Token-Dateien, Anonymisierungs-Schlüssel, CA-Private-Schlüssel, Datenbank-Dumps oder Datenbank-Protokolle mit einem freigegebenen Blueprint hinzu.
