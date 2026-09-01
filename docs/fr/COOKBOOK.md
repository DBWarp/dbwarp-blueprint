# Livre de recettes

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../COOKBOOK.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../COOKBOOK.md) | [Deutsch](../de/COOKBOOK.md) | **Français** | [Español](../es/COOKBOOK.md) | [Polski](../pl/COOKBOOK.md) | [日本語](../ja/COOKBOOK.md) | [简体中文](../zh/COOKBOOK.md)

Recettes orientées tâches pour les flux de travail courants de `dbwarp-blueprint`.

## Recette : session opérateur localisée

Sélectionnez l'un des catalogues de langue complets intégrés, tout en conservant
les commandes, valeurs, identifiants et schémas de sortie canoniques :

```bash
./dbwarp-blueprint --lang de --help
./dbwarp-blueprint --lang ja \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

Pour les exécutions sans surveillance, définissez `DBWARP_BLUEPRINT_LANG=fr` ou des
paramètres régionaux de processus standard. Un `--lang` explicite est toujours
prioritaire. Les codes DBP et les détails de bas niveau du fournisseur restent
canoniques, afin qu'un échec localisé puisse être recherché et transmis au
support.

## Recette : PostgreSQL avec une autorité de certification interne

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out pg-appdb.blueprint.toml \
  --audit-log pg-appdb.audit.txt
```

Utilisez cette recette pour une revue normale de PostgreSQL en production. Si la vérification du nom d'hôte échoue, corrigez le certificat du serveur ou utilisez le bon nom DNS ; n'utilisez pas `--tls-skip-verify`, sauf pour les tests en boucle locale.

## Recette : MySQL avec un fichier de nom d'utilisateur

Utile lorsque le nom d'utilisateur contient des caractères difficiles à encoder dans une URI.

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

Pour une reconstruction synthétique représentative des performances, utilisez
la politique équilibrée par défaut : métadonnées de déclaration/index MySQL
exactes et largeurs échantillonnées arrondies avec précision :

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

Vérifiez `declared_length_fidelity = "exact"`,
`index_length_fidelity = "exact"` et
`observed_length_fidelity = "relative-rounded-v2"`. N'utilisez
`--length-fidelity exact --yes` qu'après que le client a approuvé le partage
des statistiques exactes de longueur échantillonnée. Les noms et les valeurs
restent exclus.

Pour les parcs contenant des milliers de tables, augmentez si nécessaire
`--max-wall-secs` au-delà de sa valeur par défaut de 300 secondes. Les marqueurs
de fidélité certifient la politique, tandis que l'estimateur en aval exige
séparément les longueurs moyenne/p95 observées pour chaque colonne indexée non
vide et de largeur variable avant de déclarer une fixture prête pour un
benchmark.

## Recette : authentification SQL de SQL Server

```bash
./dbwarp-blueprint \
  --connect sqlserver://sql-blueprint@sql-primary.internal,1433/appdb \
  --password-file /etc/dbwarp/sql-blueprint.pass \
  --auth-mode sql-auth \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-appdb.blueprint.toml \
  --audit-log mssql-appdb.audit.txt
```

Les modes TLS de SQL Server qui vérifient les certificats utilisent le magasin
de confiance du système d'exploitation lorsque `--tls-ca` est omis. Un fichier
`.pem` ou `.crt` fourni doit contenir exactement un certificat d'autorité de
certification et remplace ces certificats racines. `verify-ca` et `verify-full`
valident tous deux le nom d'hôte de la connexion.

## Recette : jeton Entra ID de SQL Server

Générez le jeton en dehors de l'outil, puis fournissez-le par fichier :

```bash
install -d -m 700 "$HOME/.cache/dbwarp-blueprint"
TOKEN_FILE="$HOME/.cache/dbwarp-blueprint/sql-token"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"
chmod 600 "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

## Recette : revue de sécurité du catalogue uniquement

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

Il s'agit du mode de revue le plus simple. Il évite l'échantillonnage des lignes, mais produit en aval des estimations de compression et de sortie réseau moins précises.

## Évaluer la complexité de migration hors tables

Commencez par le résumé par défaut afin de recueillir les comptages et prérequis externes sans lire les définitions :

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail summary \
  --out appdb-summary.blueprint.toml \
  --audit-log appdb-summary.audit.txt \
  --yes
```


Après approbation de sécurité, recueillez les dépendances anonymes et les preuves bornées de complexité du langage :

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail analyzed \
  --out appdb-analyzed.blueprint.toml \
  --audit-log appdb-analyzed.audit.txt \
  --yes
```


Examinez `visibility`, les trois indicateurs de complétude, `catalogs_unreadable`, `families_not_inventoried` et `counts_by_external_class`. Traitez chaque classe externe comme une tâche de migration explicite. Un objet inventorié ne prouve pas que DBWarp peut le recréer ou le traduire ; comparez-le à la matrice de capacité de migration. Consultez [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Recette : désactiver la sonde RTT

Par défaut, l'outil exécute cinq sondes `SELECT 1` après l'établissement de la connexion et émet un bloc `[network]`. Si un DBA interdit les requêtes hors catalogue, désactivez-la :

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --no-rtt-probe \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

La sonde RTT ne lit jamais de données de ligne ; chaque requête renvoie l'entier constant `1`.

## Recette : limiter dans le temps l'échantillonnage de compression

Pour les grands systèmes de production, conservez une première exécution prudente :

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal/appdb \
  --password-file /etc/dbwarp/mysql.pass \
  --measure-compression --yes \
  --sample-rows 500 \
  --max-wall-secs 120 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Si la sortie marque de nombreux échantillons comme biaisés ou manquants, recommencez depuis une réplique en lecture avec un budget temporel plus élevé.

## Recette : un client, plusieurs bases de données

Utilisez un manifeste de lot lorsqu'un client souhaite un dossier vérifié unique pour plusieurs bases de données.

`customer.batch.toml` :

```toml
[defaults]
measure_compression = true
sample_rows = 1000
max_wall_secs = 300
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["erp", "critical"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
tags = ["billing"]

[[source]]
id = "warehouse_sql"
kind = "sqlserver"
connect_env = "WAREHOUSE_SQL_URI"
password_file = "/etc/dbwarp/warehouse.pass"
auth_mode = "sql-auth"
tags = ["warehouse"]
```

Simulation :

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Exécution :

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Cette opération écrit `bundle.toml`, un Blueprint enfant par source et un audit par source.
Les Blueprints enfants restent vérifiables indépendamment.

## Recette : un client, des bases de données et des fichiers de lac de données mélangés

Utilisez des sources de fichiers structurés dans le même lot lorsque le client dispose d'extraits Parquet ou Avro à côté de bases de données actives.

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["database"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/mnt/customer/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/mnt/customer/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` fusionne actuellement les fichiers comme `merge_same_schema`, mais conserve l'intention du client visible dans le bundle. Conservez les schémas sans rapport dans des sources distinctes.

## Recette : extraire une seule source ou table d'un bundle

Après une exécution par lot, listez les sources :

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

Extrayez une source :

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Extrayez une table d'une source :

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Utilisez cette méthode lorsque le client n'approuve qu'une partie de son parc pour un benchmark, ou lorsque vous souhaitez générer une petite fixture ciblée à partir d'un grand bundle.

## Recette : empaqueter pour transmission un bundle examiné séparément

Le répertoire de bundle de travail contient les Blueprints enfants et les
audits à accès contrôlé. Ne le transférez pas dans son intégralité. Après avoir
examiné les valeurs du manifeste et les Blueprints enfants, créez une
transmission dans un seul fichier :

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

Le fichier empaqueté conserve les identifiants de source, les balises, les
identifiants de groupe de jeux de données et les métadonnées de chemin d'audit
fournis par l'opérateur. Utilisez des valeurs anonymes, inspectez le TOML
empaqueté et transférez-le uniquement par le canal approuvé.

## Recette : dossier de transmission par lot

Créez un répertoire de ce type :

```text
customer-blueprint-handoff/
  customer-blueprint-bundle.packed.toml
  customer.batch.toml.redacted
  reviewer-notes.md       # optional
```

Construisez ce répertoire séparé à partir de copies examinées. Conservez
localement et avec un accès contrôlé le fichier `bundle.toml` de travail, les
répertoires `blueprints/` et `audits/`, ainsi que tout fichier `errors.txt`.
`customer.batch.toml.redacted` doit présenter uniquement les identifiants,
types, balises et modes de jeu de données des sources approuvés. N'incluez pas
de secrets, noms d'hôte privés, fichiers de mot de passe, fichiers de jeton,
clés privées, journaux de base de données ou échantillons de lignes décodées.

## Recette : présentation hors ligne depuis un TOML vérifié

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

Ce mode lit uniquement le fichier TOML et écrit la présentation. Il refuse les options de base de données active au lieu de les ignorer silencieusement.

## Recette : reproductibilité à l'octet près

Figez l'horodatage :

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

Utilisez cette méthode pour une revue forensique, la comparaison d'instantanés ou la génération déterministe de présentations.

## Recette : dossier de transmission à DBWarp

Créez un répertoire de ce type :

```text
customer-blueprint-handoff/
  blueprint.toml
  blueprint.pptx              # optional
  command-used.redacted.txt
  reviewer-notes.md           # optional
```

`command-used.redacted.txt` peut consigner les options et les budgets
d'échantillonnage approuvés, mais doit supprimer les identifiants, les jetons,
les noms d'hôte privés et les chemins locaux. Conservez `audit.txt` localement
comme preuve opérationnelle à accès contrôlé. Ne l'incluez que pour un besoin
d'assistance identifié, par un canal sécurisé approuvé. N'incluez pas de
fichiers de mot de passe, fichiers de jeton, clés privées ou journaux de base de
données.
