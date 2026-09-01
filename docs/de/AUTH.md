# Authentifizierung

> **Hinweis zur Übersetzung:** Dies ist eine maschinell unterstützte Übersetzung, die noch einer muttersprachlichen technischen Prüfung bedarf. Die [kanonische englische Fassung](../../AUTH.md) ist maßgeblich und diese Übersetzung ist nicht als Vertragsgrundlage geeignet.

**Sprachen:** [English](../../AUTH.md) | **Deutsch** | [Français](../fr/AUTH.md) | [Español](../es/AUTH.md) | [Polski](../pl/AUTH.md) | [日本語](../ja/AUTH.md) | [中文](../zh/AUTH.md)

`dbwarp-blueprint` unterstützt die Authentifizierungsmodi, die am häufigsten für die Blueprint-Erfassung bei PostgreSQL, MySQL und SQL Server benötigt werden.

## Benutzername

Sie können den Benutzernamen in der URI oder separat angeben:

```bash
--connect postgresql://app@db.internal/payments
```

oder:

```bash
--connect postgresql://db.internal/payments --user app
```

Verwenden Sie für Benutzernamen, die sich nur umständlich URI-codieren lassen:

```bash
--user-file /path/to/user.txt
--user-env DB_USER
```

## Passwort

Empfohlen:

```bash
--password-file /path/to/password.txt
```

Alternative:

```bash
--password-env DB_PASSWORD
```

Wenn keine Passwortquelle angegeben ist, fordert das Werkzeug, soweit möglich, interaktiv zur Eingabe auf.

In die Verbindungs-URI eingebettete Passwörter werden abgelehnt.

## SQL Server Entra-ID-Token

Erzeugen Sie für Azure SQL Database oder Managed Instance mit Microsoft Entra ID das Token mit Ihren üblichen Werkzeugen und übergeben Sie es als Geheimnis an `dbwarp-blueprint`.

Tokendatei:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-file /secure/path/token.txt \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out blueprint.toml
```

Benannte Umgebungsvariable:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-env AZURE_SQL_TOKEN \
  --tls-mode verify-full \
  --out blueprint.toml
```

Das Werkzeug ruft weder Azure CLI auf noch aktualisiert es Tokens oder schreibt das Token auf die Festplatte.

## Integrierte SQL-Server-Authentifizierung

Die integrierte Authentifizierung verwendet die bereits auf dem Host vorhandenen Anmeldedaten des Betriebssystems.

Linux Kerberos/GSSAPI:

```bash
kinit user@EXAMPLE.COM
DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh
./target/release/dbwarp-blueprint \
  --connect sqlserver://db.internal,1433/payments \
  --auth-mode integrated \
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' \
  --tls-mode verify-full \
  --out blueprint.toml
```

Windows SSPI:

```powershell
.\dbwarp-blueprint.exe `
  --connect sqlserver://db.internal,1433/payments `
  --auth-mode integrated `
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' `
  --tls-mode verify-full `
  --out blueprint.toml
```

Im integrierten Modus liest `dbwarp-blueprint` kein Passwort. Das Betriebssystem stellt dem SQL-Server-Treiber das Authentifizierungstoken bereit.

Die integrierte Authentifizierung ist nur für SQL Server verfügbar. PostgreSQL und MySQL lehnen `--auth-mode integrated` mit `DBP1005E` ab.

Die obigen Beispiele setzen voraus, dass der Windows-Principal bereits als SQL-Server-Anmeldung vorhanden ist. Die Tier-Skripte unter `sql/grants/` erstellen eine SQL-Anmeldung mit Passwort, was für diesen Modus ungeeignet ist. Erstellen Sie daher zuerst die Anmeldung mit `FROM WINDOWS` und wenden Sie anschließend die Tier-Berechtigungen unverändert an. Nur die Anmeldungs-DDL unterscheidet sich. Die Anweisungen sowie die Fälle für Gruppen, verwaltete Dienstkonten und Computerkonten finden Sie unter [Windows- und Domänen-Principals für die integrierte Authentifizierung](../../sql/grants/DATABASE_PERMISSIONS.md#windows-and-domain-principals-for-integrated-authentication).

Im Vergleich zu `sql-auth` sind zwei betriebliche Punkte in diesem Modus besonders wichtig. SQL Server sieht das Konto, unter dem der Collector-Prozess ausgeführt wird. Wird der Collector von einem Administrator auf einem Host gestartet, auf dem `BUILTIN\Administrators` zu `sysadmin` gehört, authentifiziert sich die Sitzung als `sysadmin` und umgeht sämtliche `DENY`-Regeln im Grant-Skript, obwohl die Erfassung erfolgreich ist. Mit `--expect-server-principal` wird dies vor jedem Katalogzugriff als `DBP1606E` abgebrochen. Ein dediziertes Dienstkonto erbt außerdem keine Dateizugriffe von dem Benutzer, der es gestartet hat. Es benötigt daher Leserechte auf seine eigene Anmeldedatendatei, sofern eine verwendet wird, sowie Schreibrechte auf die Pfade für `--out` und `--audit-log`.

Bei jeder SQL-Server-Verbindung werden `ORIGINAL_LOGIN()`, `SUSER_SNAME()` und
`USER_NAME()` im lokalen Audit aufgezeichnet. `--expect-server-principal` ist
optional und funktioniert auch mit SQL-Authentifizierung. SQL Server vergleicht
dabei `ORIGINAL_LOGIN()` auf der bestehenden Sitzung mit dem erwarteten
Principal. Bei einer Abweichung oder nicht verfügbarer Identität schlägt der
Lauf vor jeder Katalogerfassung mit `DBP1606E` fehl. Die exakten Identitäten
bleiben lokale Auditnachweise und werden nicht in Blueprint, Präsentation oder
Publikationsartefakte aufgenommen.

## Authentifizierung bei Cloud-verwalteten Datenbanken

Ein verwalteter Endpunkt ändert für sich allein nicht die Datenbankberechtigungen, die `dbwarp-blueprint` benötigt. Ein nativer Datenbankbenutzername mit Passwort verwendet `sql-auth` und benötigt keine Cloud-Control-Plane-Rolle, nachdem Netzwerk und Datenbankkonto bereitgestellt wurden.

`dbwarp-blueprint` ruft weder Cloud-CLIs, Metadatendienste, Secret Manager noch APIs zur Tokenaktualisierung auf. Ein Wrapper muss jedes kurzlebige Token erzeugen oder abrufen und über genau eine geschützte Geheimnisquelle bereitstellen.

### Cloud-Token für PostgreSQL und MySQL

Verwenden Sie `cloud-token` für ein direktes, von AWS, Azure oder Google Cloud erzeugtes PostgreSQL- oder MySQL-Token eines verwalteten Dienstes. Geben Sie genau eine der Optionen `--password-file` oder `--password-env` an. Der Modus erfordert `verify-full`; fügen Sie das CA-Bundle des Providers oder der Instanz hinzu, wenn es nicht in der im Binärprogramm kompilierten Vertrauensbasis verankert ist.

PostgreSQL-Beispiel:

```bash
./dbwarp-blueprint \
  --connect postgresql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

MySQL-Beispiel:

```bash
./dbwarp-blueprint \
  --connect mysql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

Bei MySQL aktiviert `cloud-token` den Austausch über `mysql_clear_password` ausschließlich innerhalb dieser verifizierten TLS-Verbindung. Bei normalen `sql-auth`-Verbindungen bleibt das Plugin deaktiviert. PostgreSQL verwendet sein normales Passwortprotokoll unter derselben Anforderung an verifiziertes TLS.

### Cloud-seitige Laufzeitberechtigungen

Diese Berechtigungen autorisieren die Anmeldung oder einen Verbindungstunnel; sie ersetzen niemals den Datenbankprinzipal und dessen Berechtigungen:

| Verwalteter Pfad | Binärmodus | Laufzeitberechtigung außerhalb der Datenbank |
|---|---|---|
| RDS/Aurora PostgreSQL- oder MySQL-IAM-Anmeldung | `cloud-token` | `rds-db:connect` für den exakten ARN des Datenbankbenutzers |
| Entra-Anmeldung bei Azure Database for PostgreSQL/MySQL | `cloud-token` | Keine Azure-Ressourcen-RBAC-Rolle für Datenzugriff; die Identität muss in der Datenbank zugeordnet sein |
| Direkte Cloud SQL PostgreSQL/MySQL-IAM-Anmeldung | `cloud-token` | Exakte Berechtigung `cloudsql.instances.login`; `roles/cloudsql.instanceUser` ist die breitere vordefinierte Alternative |
| Cloud SQL Auth Proxy oder Connector | Üblicherweise `sql-auth`; der Proxy kann automatische IAM-Authentifizierung durchführen | Die Proxy-Identität benötigt `roles/cloudsql.client`; automatische IAM-Authentifizierung benötigt zusätzlich die Anmeldeberechtigung |
| Entra-Anmeldung bei Azure SQL Database oder Managed Instance | `entra-token` | Keine Azure-Ressourcen-RBAC-Rolle für Datenzugriff; verwenden Sie die oben dokumentierten SQL-Server-Tokenoptionen |
| Jede unterstützte verwaltete Datenbank mit nativen Datenbankanmeldedaten | `sql-auth` | Keine |

Die Berechtigungsprüfung der Bereitstellung sollte die versionsabhängigen Datenbankberechtigungen, exakten Cloud-Richtlinien, Alternativen mit integrierten Rollen und Einschränkungen des Geltungsbereichs festhalten. Providerkonfiguration, Prinzipalerstellung, Netzwerkzugriff, Tokenerzeugung und optionaler Geheimnisabruf sind Aufgaben der Bereitstellung oder des Wrappers – keine Berechtigungen, die allein wegen eines verwalteten Endpunkts an den Collector angehängt werden sollten.
