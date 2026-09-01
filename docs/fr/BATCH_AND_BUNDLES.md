# Collecte par lots et bundles Blueprint

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../BATCH_AND_BUNDLES.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../BATCH_AND_BUNDLES.md) | [Deutsch](../de/BATCH_AND_BUNDLES.md) | **Français** | [Español](../es/BATCH_AND_BUNDLES.md) | [Polski](../pl/BATCH_AND_BUNDLES.md) | [日本語](../ja/BATCH_AND_BUNDLES.md) | [简体中文](../zh/BATCH_AND_BUNDLES.md)

`dbwarp-blueprint` prend en charge à la fois les fichiers Blueprint à source unique et les répertoires de bundles multisources.

Utilisez un seul fichier `blueprint.toml` lorsque le client partage une base de données, un sous-ensemble de tables, un fichier Parquet ou un fichier Avro. Utilisez un bundle lorsque le client possède plusieurs bases de données, plusieurs jeux de données de fichiers structurés ou souhaite disposer d'un seul paquet de revue pour l'ensemble de son parc.

## Organisation du bundle

Une exécution par lot écrit un répertoire :

```text
customer-blueprint-bundle/
  bundle.toml
  blueprints/
    erp_pg.blueprint.toml
    billing_mysql.blueprint.toml
    orders_parquet.blueprint.toml
  audits/
    erp_pg.audit.txt
    billing_mysql.audit.txt
    orders_parquet.audit.txt
```

`bundle.toml` contient les métadonnées au niveau des sources et les chemins relatifs vers les fichiers Blueprint enfants. Il s'agit de la forme de travail privilégiée, car chaque source reste vérifiable, auditable et réexécutable indépendamment.

Pour une transmission vérifiée séparément, regroupez le répertoire dans un seul fichier TOML intégré :

```bash
dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

La forme empaquetée intègre chaque Blueprint enfant sous l'entrée de sa source. Elle conserve les identifiants de source, les tags, les identifiants de groupe de jeux de données et les métadonnées de chemin d'audit fournis par l'opérateur ; utilisez donc des valeurs de manifeste anonymes et inspectez le fichier empaqueté avant le transfert. Le répertoire de travail est plus facile à examiner, mais contient aussi des audits détaillés et tout fichier `errors.txt` ; ne le transférez pas intégralement par défaut.

## Contrat du bundle

Les bundles actuels utilisent `schema_version = 3` et
`kind = "dbwarp-blueprint-bundle"`. Un bundle sous forme de répertoire référence
chaque Blueprint enfant avec `blueprint_path` ; un bundle empaqueté l'intègre
sous `blueprint`. Les outils d'écriture n'émettent que ces identifiants
canoniques.

Les lecteurs acceptent également les schémas de bundle v1 et v2. Ces contrats
servent uniquement à la compatibilité en entrée : tout ancien bundle accepté
est normalisé en v3 et n'est jamais réémis avec les anciens identifiants. Comme
les anciens bundles ne précisent pas si les sources sont indépendantes,
répliquées ou shardées, leur relation devient `unknown` et les totaux entre
sources sont supprimés. Les chemins enfants doivent rester relatifs et confinés
au répertoire du bundle après canonicalisation.

Le bundle v3 distingue les sources physiques des jeux de données logiques.
Chaque source possède `dataset_relationship`, `dataset_group` et
`dataset_scope_completeness`. La table supérieure `dataset_groups` enregistre
la relation, les membres et l'exhaustivité de l'ensemble déclaré.

L'agrégation échoue de manière sûre :

- `independent` : exactement une source dans le groupe, ajoutée une fois.
- `replica` : les copies concordantes comptent une fois. En cas de divergence,
  un représentant déterministe est conservé, sans moyenne, et le résultat est
  incomplet.
- `shard` : les membres ne sont additionnés que si
  `members_complete = true` et si tous ont réussi. Un groupe incomplet ne
  contribue à aucun total.
- `unknown` : tous les totaux de tables, lignes et octets entre sources sont
  supprimés.
- Une source dont `[dataset_scope]` est incomplet ou inconnu rend les preuves
  agrégées incomplètes même si sa relation est connue.

Les totaux par source sont toujours conservés. Seul l'agrégat entre sources est
supprimé, ce qui évite de multiplier un jeu de réplicas ou de présenter un
sous-ensemble de shards comme le jeu complet.

## Manifeste de lot

Créez un manifeste appartenant au client :

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
dataset_relationship = "independent"
tags = ["critical", "erp"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
dataset_relationship = "independent"
tags = ["billing"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
dataset_relationship = "independent"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
dataset_relationship = "independent"
tags = ["lake"]
```

Si la relation est omise, sa valeur par défaut est `unknown` ; l'exécution
réussit mais émet `DBP1414W` et `DBP1417W`, et supprime les totaux agrégés. Cela
est plus sûr que de supposer que deux endpoints sont deux jeux indépendants.

Déclarez les membres répliqués dans un même groupe :

```toml
[[source]]
id = "orders_primary"
kind = "postgresql"
connect_env = "ORDERS_PRIMARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true

[[source]]
id = "orders_secondary"
kind = "postgresql"
connect_env = "ORDERS_SECONDARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true
```

Pour un système shardé, listez chaque shard connu dans un groupe commun et ne
définissez `dataset_group_complete = true` que si le manifeste énumère le jeu
logique complet. Un membre en échec rend ce groupe incomplet pour l'exécution.

Commencez par une simulation :

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Exécutez le lot :

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Une exécution par lot qui n'est pas une simulation exige `--yes`, car elle peut se connecter à plusieurs bases de données ou décoder des échantillons de fichiers structurés. Chaque source enfant reçoit son propre fichier d'audit.

Avec `continue_on_error = true`, les autres sources sont traitées et le bundle de diagnostic, y compris `errors.txt`, est publié atomiquement. La commande se termine néanmoins en erreur : `DBP1115E` si toutes les sources ont échoué et `DBP1116E` si l'échec est partiel. Un bundle partiel sert à la revue et à la relance ; ce n'est pas une collecte complète réussie.

La simulation comme l'exécution réelle valident le manifeste complet avant de
toucher à une source. Les champs inconnus, les identifiants dupliqués, les
identifiants qui entrent en collision après normalisation sûre du nom de fichier,
les champs incompatibles avec le type de source, les sources de connexion à la
base de données ambiguës, les modes de jeu de données non valides et les budgets
d'échantillonnage de compression nuls sont refusés. Chaque `source.id` doit être
unique, sans espaces de début ou de fin, et ne pas dépasser 120 octets ASCII
après normalisation.

## Modes de jeux de données de fichiers structurés

Pour les sources Parquet et Avro :

- `single_file` exige exactement un fichier résolu et le conserve sous la forme d'une table logique unique.
- `one_table_per_file` associe chaque fichier à une table assainie distincte dans un fichier Blueprint enfant unique.
- `merge_same_schema` fusionne de nombreux fichiers en une table logique lorsque le nombre de colonnes correspond.
- `partitioned_dataset` utilise actuellement le même comportement de fusion que `merge_same_schema` ; il réserve la distinction sémantique à la découverte des partitions de style Hive.

Le contrôle de fusion est volontairement prudent. Il exige une disposition de
colonnes anonymisée identique, les mêmes types canoniques et natifs, la même
nullabilité, les mêmes largeurs déclarées, précision et échelle, les mêmes
sémantiques non signées et `BIT(n)`, la même précision d'horodatage, le même jeu
de caractères et classement, ainsi que les mêmes sémantiques de source
structurée. Pour la planification de lacs de données à enjeux élevés, regroupez
les jeux de données selon un schéma connu même lorsque ce contrôle structurel
réussit.

## Opérations sur les bundles

Lister les sources :

```bash
dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

Les premières lignes indiquent `aggregation`, les `sources` physiques,
`logical_datasets`, les totaux agrégés et les `limitations`. Les lignes de
groupe indiquent `relationship`, `members_complete` et les identifiants source.
Les lignes source indiquent `dataset_relationship`, `dataset_group` et
`dataset_scope`. Traitez `aggregation=suppressed` comme une instruction de
contrôler ou corriger le manifeste, jamais comme un parc de taille nulle.

Lister un sous-ensemble de sources portant une balise :

```bash
dbwarp-blueprint \
  --bundle-list customer-blueprint-bundle/bundle.toml \
  --select tag=erp
```

Extraire une source :

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Extraire une table d'une source :

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Les clés de sélecteur prises en charge sont :

- `source=ID`
- `table=ID`
- `engine=postgresql|mysql|sqlserver|parquet|avro`
- `tag=NAME`

Les sélecteurs peuvent être transmis sous la forme d'une chaîne unique séparée par des virgules ou au moyen d'options `--select` répétées. Les valeurs contradictoires pour une même clé sont refusées.

## Transmission en aval

Un bundle est une entrée Blueprint portable et vérifiable. Avant de l'accepter, un consommateur en aval doit valider le contrat du bundle et les versions de schéma, appliquer les sélecteurs enregistrés et conserver les identifiants de source lorsqu'il combine plusieurs enfants afin d'éviter toute collision entre identifiants de table. Les commandes et règles de compatibilité des autres produits DBWarp relèvent de leur documentation examinée séparément et ne sont volontairement pas reproduites ici.

## Limite de confidentialité et de revue

Un bundle n'assouplit pas le modèle de confidentialité :

- les sources de bases de données actives continuent d'émettre des identifiants assainis de table, colonne et index ;
- les valeurs des fichiers structurés ne sont décodées que lorsque `--measure-compression --yes` est activé ;
- les échantillons décodés restent en mémoire ;
- les métadonnées du bundle utilisent les identifiants de source et balises choisis par le client ;
- aucune commande de bundle n'envoie de télémétrie ni ne téléverse de fichier.

Le client peut supprimer toute Blueprint enfant ou toute entrée de source avant de partager le bundle.
