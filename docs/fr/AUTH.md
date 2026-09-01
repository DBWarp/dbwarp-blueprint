# Authentification

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../AUTH.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../AUTH.md) | [Deutsch](../de/AUTH.md) | **Français** | [Español](../es/AUTH.md) | [Polski](../pl/AUTH.md) | [日本語](../ja/AUTH.md) | [简体中文](../zh/AUTH.md)

`dbwarp-blueprint` prend en charge les modes d'authentification les plus couramment nécessaires pour collecter les Blueprints de PostgreSQL, MySQL et SQL Server.

## Nom d'utilisateur

Vous pouvez fournir le nom d'utilisateur dans l'URI ou séparément :

```bash
--connect postgresql://app@db.internal/payments
```

ou :

```bash
--connect postgresql://db.internal/payments --user app
```

Pour les noms d'utilisateur difficiles à encoder dans une URI, utilisez :

```bash
--user-file /path/to/user.txt
--user-env DB_USER
```

## Mot de passe

Recommandé :

```bash
--password-file /path/to/password.txt
```

Alternative :

```bash
--password-env DB_PASSWORD
```

Si aucune source de mot de passe n'est fournie, l'outil le demande de manière interactive lorsque cela est possible.

Les mots de passe intégrés dans l'URI de connexion sont refusés.

## Jeton Microsoft Entra ID pour SQL Server

Pour Azure SQL Database ou Managed Instance avec Microsoft Entra ID, générez le jeton à l'aide de vos outils habituels et transmettez-le à `dbwarp-blueprint` en tant que secret.

Fichier de jeton :

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-file /secure/path/token.txt \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out blueprint.toml
```

Variable d'environnement nommée :

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-env AZURE_SQL_TOKEN \
  --tls-mode verify-full \
  --out blueprint.toml
```

L'outil n'appelle pas Azure CLI, ne renouvelle pas les jetons et n'écrit pas le jeton sur disque.

## Authentification intégrée SQL Server

L'authentification intégrée utilise les informations d'identification du système d'exploitation déjà présentes sur l'hôte.

Kerberos / GSSAPI sous Linux :

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

SSPI sous Windows :

```powershell
.\dbwarp-blueprint.exe `
  --connect sqlserver://db.internal,1433/payments `
  --auth-mode integrated `
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' `
  --tls-mode verify-full `
  --out blueprint.toml
```

En mode intégré, `dbwarp-blueprint` ne lit aucun mot de passe. Le système d'exploitation fournit le jeton d'authentification au pilote SQL Server.

L'authentification intégrée est disponible uniquement pour SQL Server. PostgreSQL et MySQL refusent `--auth-mode integrated` avec `DBP1005E`.

Les exemples ci-dessus supposent que le principal Windows existe déjà en tant que connexion SQL Server. Les scripts de niveau dans `sql/grants/` créent une connexion SQL avec un mot de passe, ce qui ne convient pas à ce mode. Créez donc d'abord la connexion avec `FROM WINDOWS`, puis appliquez les autorisations du niveau sans les modifier. Seul le DDL de connexion diffère. Consultez [Principaux Windows et de domaine pour l'authentification intégrée](../../sql/grants/DATABASE_PERMISSIONS.md#windows-and-domain-principals-for-integrated-authentication) pour les instructions ainsi que les cas des groupes, comptes de service administrés et comptes d'ordinateur.

Deux points opérationnels sont plus importants dans ce mode qu'avec `sql-auth`. Le compte qui exécute le processus du collecteur est l'identité vue par SQL Server. Si le collecteur est lancé par un administrateur sur un hôte où `BUILTIN\Administrators` appartient à `sysadmin`, la session est `sysadmin` et contourne chaque règle `DENY` du script d'autorisations, alors même que la capture réussit. `--expect-server-principal` transforme ce cas en échec `DBP1606E` avant toute lecture du catalogue. Par ailleurs, un compte de service dédié n'hérite d'aucun accès aux fichiers de la personne qui l'a lancé. Il lui faut un droit de lecture sur son propre fichier d'informations d'identification lorsqu'un tel fichier est utilisé, ainsi qu'un droit d'écriture sur les chemins `--out` et `--audit-log`.

Chaque connexion SQL Server consigne `ORIGINAL_LOGIN()`, `SUSER_SNAME()` et
`USER_NAME()` dans l'audit local. `--expect-server-principal` est facultatif et
fonctionne aussi avec l'authentification SQL. SQL Server compare alors
`ORIGINAL_LOGIN()` au principal attendu sur la session établie. Une différence
ou une identité indisponible provoque `DBP1606E` avant toute capture du
catalogue. Les identités exactes restent des preuves d'audit locales et ne sont
pas incluses dans le Blueprint, la présentation ou les artefacts publiés.

## Authentification des bases de données gérées dans le cloud

Un point de terminaison géré ne modifie pas à lui seul les droits de base de données requis par `dbwarp-blueprint`. Un nom d’utilisateur et un mot de passe natifs utilisent `sql-auth` et n’exigent aucun rôle de plan de contrôle cloud une fois le réseau et le compte de base de données provisionnés.

`dbwarp-blueprint` n’appelle ni CLI cloud, ni service de métadonnées, ni gestionnaire de secrets, ni API de renouvellement de jeton. Un wrapper doit générer ou récupérer chaque jeton de courte durée et le fournir par une seule source de secret protégée.

### Jetons cloud PostgreSQL et MySQL

Utilisez `cloud-token` pour un jeton direct de service géré PostgreSQL ou MySQL généré par AWS, Azure ou Google Cloud. Fournissez exactement une des options `--password-file` ou `--password-env`. Le mode exige `verify-full`; ajoutez le bundle d’AC du fournisseur ou de l’instance s’il n’est pas ancré dans l’ensemble de confiance compilé du binaire.

Exemple PostgreSQL :

```bash
./dbwarp-blueprint \
  --connect postgresql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

Exemple MySQL :

```bash
./dbwarp-blueprint \
  --connect mysql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

Pour MySQL, `cloud-token` active l’échange `mysql_clear_password` uniquement dans cette connexion TLS vérifiée. Le plug-in reste désactivé pour les connexions `sql-auth` normales. PostgreSQL utilise son protocole de mot de passe normal avec la même exigence de TLS vérifié.

### Autorisations d’exécution côté cloud

Ces autorisations permettent la connexion ou un tunnel; elles ne remplacent jamais le principal et les droits de la base de données :

| Chemin géré | Mode du binaire | Autorisation d’exécution hors base de données |
|---|---|---|
| Connexion IAM RDS/Aurora PostgreSQL ou MySQL | `cloud-token` | `rds-db:connect` sur l’ARN exact de l’utilisateur de base de données |
| Connexion Entra Azure Database for PostgreSQL/MySQL | `cloud-token` | Aucun rôle RBAC de ressource Azure pour l’accès aux données; l’identité doit être mappée dans la base |
| Connexion IAM directe Cloud SQL PostgreSQL/MySQL | `cloud-token` | Autorisation exacte `cloudsql.instances.login`; `roles/cloudsql.instanceUser` est l’alternative prédéfinie plus large |
| Cloud SQL Auth Proxy ou connecteur | Généralement `sql-auth`; le proxy peut effectuer l’authentification IAM automatique | L’identité du proxy exige `roles/cloudsql.client`; l’authentification IAM automatique exige aussi l’autorisation de connexion |
| Connexion Entra Azure SQL Database ou Managed Instance | `entra-token` | Aucun rôle RBAC de ressource Azure pour l’accès aux données; utilisez les options de jeton SQL Server documentées ci-dessus |
| Toute base gérée prise en charge avec des identifiants natifs | `sql-auth` | Aucune |

La revue des autorisations de déploiement doit consigner les droits de base de données dépendant de la version, les politiques cloud exactes, les alternatives de rôles intégrés et leurs limites de portée. La configuration du fournisseur, la création des principaux, l’accès réseau, la génération des jetons et la récupération facultative des secrets relèvent du provisionnement ou du wrapper; ces droits ne doivent pas être attribués au collecteur simplement parce que le point de terminaison est géré.
