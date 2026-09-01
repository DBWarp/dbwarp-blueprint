# Schnellstart

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../QUICKSTART.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../QUICKSTART.md) | **Deutsch** | [Français](../fr/QUICKSTART.md) | [Español](../es/QUICKSTART.md) | [Polski](../pl/QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | [中文](../zh/QUICKSTART.md)

Dieser Schnellstart richtet sich an Sales Engineers, DBAs und Sicherheitsprüfer, die eine weitergabefähige DBWarp-Blueprint-Datei erstellen müssen, ohne Kundendaten offenzulegen.

## 1. Ausführungsweg auswählen

Verwenden Sie einen der folgenden Wege:

- Laden Sie eine Release-Binärdatei herunter und prüfen Sie ihre Prüfsumme.
- Erstellen Sie das Werkzeug mit `./build.sh` aus dem Quellcode.
- Erstellen Sie es aus dem gebündelten Release-Paket, wenn eine strenge Offline-Prüfung der Abhängigkeiten erforderlich ist.

Siehe [`../BUILD.md`](BUILD.md) und [`../binaries/README.md`](BINARIES.md).

Wählen Sie bei Bedarf ausdrücklich eine Anzeigesprache:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --dry-run
```

Unterstützte Werte sind `en`, `de`, `fr`, `es`, `pl`, `ja` und `zh`. Die Anzeigesprache ändert Hilfetexte, Eingabeaufforderungen, Diagnosen, Fortschrittstexte und Präsentationsprosa. Optionsnamen, zulässige Werte, URI-Schemata, Selektoren, DBP-Codes, Audit-Schlüssel oder Blueprint-TOML werden niemals geändert. Siehe [`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Anmeldedaten sicher vorbereiten

Fügen Sie keine Passwörter in die Verbindungs-URI ein. Das Werkzeug lehnt in URIs eingebettete Passwörter ab, um Lecks über Prozesslisten und Shell-Verläufe zu vermeiden.

Bevorzugtes Muster für Passwortdateien (das Geheimnis wird ohne Anzeige eingegeben und erscheint nicht im Shell-Verlauf):

```bash
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

Wenn der Benutzername umständlich URI-codiert werden müsste, legen Sie ihn ebenfalls in einer Datei ab:

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\\migration_user' > /etc/dbwarp/db.user
```

Verwenden Sie danach `--user-file /etc/dbwarp/db.user`.

## 3. Zuerst einen Probelauf durchführen

Ein Probelauf validiert die Argumente und zeigt die geplante Aktion an, ohne eine Verbindung herzustellen:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

Im Präsentationsmodus `--from-toml` ist der Probelauf eine lokale Vorabprüfung und liest die Datenbank nicht.

Führen Sie bei mehreren Kundenquellen stattdessen einen Probelauf mit dem Batch-Manifest durch:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 4. Reinen Katalogmodus ausführen

Der reine Katalogmodus liest Metadaten und Statistiken, aber keine Zeilenstichproben:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

Verwenden Sie diesen Modus, wenn eine Richtlinie Zeilenstichproben verbietet oder wenn Sie einen ersten Durchlauf für die Sicherheitsprüfung benötigen.

## 5. Details zu Nicht-Tabellenartefakten auswählen

Die Voreinstellung `--artifact-detail summary` liest Kataloge für Nicht-Tabellenobjekte, aber keine Objektdefinitionen. Sie gibt begrenzte Anzahlen und Klassen externer Voraussetzungen aus. Verwenden Sie `--artifact-detail none`, wenn eine Richtlinie diese Kataloge verbietet.

Für anonyme Abhängigkeitstopologie verwenden Sie `graph`; für begrenzte Sprachmerkmal- und Komplexitätsbänder `analyzed`. Beide erfordern ausdrückliche Zustimmung:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


Die Ausgabe enthält niemals Objektnamen, Definitionstext, Endpunkte, Geheimnisse, Schlüssel, Zertifikate oder Binärdateien. Lesen Sie [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md), bevor Sie den Modus graph oder analyzed genehmigen.

## 6. Tier-2-Komprimierungsmessung ausführen

Tier 2 liest begrenzte Zeilenstichproben in den Speicher, komprimiert sie lokal, schreibt nur zusammenfassende Verhältnisse und verwirft die Stichprobenbytes:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

Verwenden Sie Tier 2, wann immer möglich. Dadurch kann DBWarp Übertragungsbytes, Egress-Kosten und die Erzeugung synthetischer Text-/Binärdaten besser schätzen.

## 7. Präsentation erzeugen

Während des Live-Laufs:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

## 8. Vor der Weitergabe prüfen

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
- optional nur Komprimierungsverhältnisse, keine Stichprobenbytes.

## 9. Übergabe an DBWarp

Minimale Übergabe:

```text
blueprint.toml
```

Erstellen und prüfen Sie für eine Kundenprüfung mit mehreren Quellen ein gepacktes Bundle, statt das Arbeitsverzeichnis zu übergeben:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Die Bundle-Metadaten enthalten die im Batch-Manifest gewählten Quell-IDs, Tags und Datensatzgruppen-IDs. Verwenden Sie anonyme Werte und prüfen Sie sie vor der Übertragung.

Verwenden Sie `docs/BATCH_AND_BUNDLES.md`, wenn der Kunde mehrere Datenbanken, mehrere Parquet- oder Avro-Datensätze besitzt oder nur ausgewählte Quellen/Tabellen für die Benchmark-Erzeugung freigeben möchte.

Bewahren Sie diese Artefakte standardmäßig als zugriffsgeschützte lokale Nachweise auf:

```text
blueprint.audit.txt
blueprint.pptx
command-used.txt
```

Auditprotokolle und gespeicherte Befehle können Datenbankendpunkte, authentifizierte Identitäten, lokale Pfade, Zeitangaben und Manifest-Quell-IDs enthalten. Senden Sie sie nur für einen konkreten Supportbedarf über einen genehmigten sicheren Kanal. Senden Sie keine Passwortdateien, privaten CA-Schlüssel, Kundendumps oder Datenbankprotokolle.
