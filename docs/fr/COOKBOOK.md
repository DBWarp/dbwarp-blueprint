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
  --artifact-detail none \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

Pour les exécutions sans surveillance, définissez `DBWARP_BLUEPRINT_LANG=fr` ou des
paramètres régionaux de processus standard. Un `--lang` explicite est toujours
prioritaire. Les codes DBP et les détails de bas niveau du pilote restent
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

La recette ci-dessus utilise déjà la politique équilibrée par défaut : métadonnées MySQL declaration/index exactes et largeurs échantillonnées arrondies avec précision.

Confirmez `declared_length_fidelity = "exact"`, `index_length_fidelity = "exact"` et `observed_length_fidelity = "relative-rounded-v2"`. Utilisez `--length-fidelity exact --yes` uniquement après que votre organisation ait approuvé le partage de statistiques de longueur d'échantillon exactes. Les noms et les valeurs restent exclus.

Dans les bases de données contenant des milliers de tables, augmentez `--max-wall-secs` au-dessus de sa valeur par défaut de 300 secondes si nécessaire. Les marqueurs de fidélité décrivent la politique ; ils n'indiquent pas que l'échantillonnage a atteint toutes les tables.

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
install -m 600 /dev/null "$TOKEN_FILE"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

Azure SQL présente un certificat émis par une autorité publique. Cette recette
laisse donc `--tls-ca` non défini et utilise le magasin de confiance du système
d’exploitation. Un fichier `--tls-ca` fourni remplace ce magasin par un seul
certificat ; consultez [TLS](TLS.md).

## Recette : revue de sécurité du catalogue uniquement

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

C'est le mode de révision avec le moins de contraintes. Il évite l'échantillonnage des lignes, mais produit des estimations de compression et de débit de sortie moins précises.

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


Vérifiez `visibility`, les trois indicateurs de complétude, `catalogs_unreadable`, `families_not_inventoried` et `counts_by_external_class`. Traitez chaque classe externe comme une tâche de migration explicite. Ne considérez pas un objet répertorié comme une preuve que DBWarp peut le recréer ou le traduire ; demandez à DBWarp quels types d'objets sont pris en charge pour votre migration. Consultez [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

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

## Recette : Plusieurs bases de données dans un seul package.

Utilisez un manifeste par lots lorsque vous souhaitez un seul paquet révisé pour plusieurs bases de données.

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

## Recette : Bases de données hétérogènes et fichiers de data lake.

Utilisez les sources de fichiers structurés dans le même lot lorsque vous avez des extractions Parquet ou Avro à proximité de bases de données actives.

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
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` fusionne les fichiers tels que `merge_same_schema` et enregistre le mode déclaré dans le groupe. Conservez les schémas non liés dans des sources distinctes.

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

Utilisez ceci lorsque seulement une partie d'un ensemble est approuvée pour être partagée.

## Recette : Préparer un ensemble validé pour le partage.

Le répertoire du paquet de travail contient des Blueprints enfants et des audits contrôlés par l'accès. Ne le transférez pas intégralement. Après avoir examiné les valeurs du manifeste et les Blueprints enfants, créez un seul fichier à partager :

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

Le fichier empaqueté conserve les identifiants de source, les balises, les
identifiants de groupe de jeux de données et les métadonnées de chemin d'audit
fournis par l'opérateur. Utilisez des valeurs anonymes, inspectez le TOML
empaqueté et transférez-le uniquement par le canal approuvé.

## Recette : Paquet groupé pour le partage.

Suivez les [instructions de révision et de partage](QUICKSTART.md#review-and-share). Conservez le manifeste de travail, les audits et les enregistrements de commandes localement ; créez ce répertoire distinct uniquement à partir du Blueprint empaqueté révisé.

```text
blueprint-share/
  customer-blueprint-bundle.packed.toml
```

## Recette : présentation hors ligne depuis un TOML vérifié

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

Ce mode lit uniquement le fichier TOML et écrit la présentation. Il refuse les options de base de données active au lieu de les ignorer silencieusement.

## Recette : reproductibilité à l'octet près

Figez l'horodatage et réutilisez la même clé d'anonymisation protégée que vous
détenez :

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --anonymization-key-file /etc/dbwarp/anonymization.key \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

Le fichier de clé doit contenir exactement 32 octets bruts ou 64 caractères hexadécimaux, ne doit pas être group/world-readable sous Unix, et ne doit jamais être partagé. Sans cette option, une nouvelle clé générée aléatoirement par le système d'exploitation modifie intentionnellement l'ordre des étiquettes anonymes à chaque exécution. Fixer uniquement `--generated-at` est insuffisant. Utilisez la recette complète pour les instantanés forensiques approuvés ; Une présentation générée deux fois à partir du même Blueprint examiné reste identique au niveau des octets lorsque son horodatage et sa langue ne changent pas.

## Recette : Package à partager avec DBWarp.

Suivez les [instructions de révision et de partage](QUICKSTART.md#review-and-share). Le paquet par défaut contient uniquement le Blueprint approuvé :

```text
blueprint-share/
  blueprint.toml
```

Ajoutez `blueprint.pptx` uniquement après une revue et une approbation distinctes. Conservez les audits, les enregistrements de commandes et le matériel credential/key en dehors du répertoire partagé ; envoyez les audits uniquement pour un besoin de support spécifique via un canal sécurisé approuvé.
