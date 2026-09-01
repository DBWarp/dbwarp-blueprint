# TLS und Zertifikate

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../TLS.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../TLS.md) | **Deutsch** | [Français](../fr/TLS.md) | [Español](../es/TLS.md) | [Polski](../pl/TLS.md) | [日本語](../ja/TLS.md) | [中文](../zh/TLS.md)

Verwenden Sie TLS immer dann, wenn die Datenbankverbindung eine Netzwerkgrenze überschreitet.
`verify-full` ist der Standard: Zertifikatskette und Serverhostname werden geprüft, sofern der Bediener keinen anderen Modus auswählt.

## Allgemeine Optionen

```bash
--tls-mode disable|prefer|require|verify-ca|verify-full
--tls-ca /path/to/ca-bundle.pem
--tls-cert /path/to/client-cert.pem
--tls-key /path/to/client-key.pem
```

Empfohlene Produktionseinstellung:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

## Interne CA

Wenn Ihr Datenbankzertifikat von einer internen CA signiert ist:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml
```

## Nicht übereinstimmender Hostname

Verwenden Sie bei `--tls-mode verify-full` einen Hostnamen für `--connect`, der mit dem Zertifikat übereinstimmt. Dieses Release unterstützt keine Überschreibung des TLS-Servernamens; die Übergabe von `--tls-server-name` schlägt deutlich fehl, statt die Prüfung stillschweigend abzuschwächen. Wenn Ihre Richtlinie die CA-Prüfung ohne Hostnamenprüfung zulässt, verwenden Sie `--tls-mode verify-ca`.

Die standardmäßigen Vertrauensquellen sind Engine-spezifisch:

- PostgreSQL und MySQL verwenden die in die Binärdatei kompilierten
  Mozilla-Stammzertifikate, wenn `--tls-ca` nicht angegeben ist. Ein
  bereitgestelltes PEM-Bundle ersetzt diese Stammzertifikate.
- SQL Server verwendet den Trust Store des Betriebssystems, wenn `--tls-ca`
  nicht angegeben ist. Eine bereitgestellte `.pem`- oder `.crt`-Datei muss
  genau ein CA-Zertifikat enthalten und ersetzt die Stammzertifikate des
  Betriebssystems.

Der SQL-Server-Treiber prüft den Verbindungshostnamen sowohl bei `verify-ca`
als auch bei `verify-full`; für diese Engine ist `verify-ca` bewusst nicht
schwächer als `verify-full`.

## Klartext- und Kompatibilitätsmodi

`prefer` ist nur für Loopback-Ziele zulässig. PostgreSQL kann dort auf lokalen Klartext zurückfallen und gibt `DBP1404W` aus; andere Engines versuchen weiterhin TLS. Die entfernten Modi `disable` und `require` benötigen `--i-know-what-im-doing`, weil sie Klartext erlauben beziehungsweise den Server nicht authentifizieren. Diese Bestätigung macht sie nicht produktionstauglich.

## mTLS

PostgreSQL und MySQL unterstützen die Authentifizierung mit Clientzertifikat.
Wenn eine dieser Datenbanken ein Clientzertifikat verlangt:

```bash
--tls-cert /etc/dbwarp/client.crt \
--tls-key /etc/dbwarp/client.key
```

Private Schlüsseldateien dürfen auf Unix-ähnlichen Systemen nicht für Gruppe/Alle lesbar sein.
SQL Server unterstützt keine Authentifizierung mit Clientzertifikat; die
Angabe von `--tls-cert`/`--tls-key` für diese Engine schlägt mit `DBP1015E`
fehl, statt die Dateien stillschweigend zu ignorieren.

## Prüfung überspringen

`--tls-skip-verify` ist ausschließlich für Diagnosen vorgesehen. Verwenden Sie diese Option nicht zur Blueprint-Erfassung produktiver Datenbanken, sofern Ihr Sicherheitsteam sie nicht ausdrücklich genehmigt hat.

## Auditprotokoll

Das Auditprotokoll zeichnet den angeforderten TLS-Modus, CA-Pfad, Clientzertifikatpfad und die Information auf, ob die Prüfung übersprungen wurde. Nach erfolgreicher Verbindung wird die TLS-Aushandlung erfasst; da die aktuellen Treiber keine verlässliche Protokollversion liefern, wird diese als nicht verfügbar ausgewiesen. Private Schlüssel werden nicht protokolliert.
