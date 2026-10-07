<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../.github/assets/dbwarp-logo-dark.png">
    <img src="../../.github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

# dbwarp-blueprint

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../README.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle. Consultez la [politique de traduction de la documentation](../TRANSLATIONS.md).

[`MACHINE_TRANSLATIONS.md`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/MACHINE_TRANSLATIONS.md).

**Langues :** [English](../../README.md) | [Deutsch](../de/README.md) | **Français** | [Español](../es/README.md) | [Polski](../pl/README.md) | [日本語](../ja/README.md) | [简体中文](../zh/README.md)

## Présentation

DBWarp Blueprint est un collecteur de Blueprint de base de données axé sur la confiance. Vous l’exécutez dans votre propre environnement sur PostgreSQL, MySQL ou SQL Server. Il lit les métadonnées du catalogue et, si vous demandez une mesure de compression, un échantillon borné de lignes. Il écrit ensuite un Blueprint structurel anonymisé de votre base de données : tailles des tables, nombres de lignes, familles de types, ainsi que structure des index et des clés étrangères.

Les identifiants sont remplacés par des étiquettes anonymes identifiées par une clé, et aucune valeur de ligne n'est écrite dans le Blueprint. Une clé locale à chaque processus empêche par défaut les vérifications de dictionnaire hors ligne ; `--anonymization-key-file` vous permet de conserver les étiquettes entre les exécutions de comparaison approuvées. Lisez [`SECURITY.md`](SECURITY.md) avant de partager tout résultat : il décrit précisément ce que chaque mode révèle, et quelles options élargissent cette information.

La sortie est un fichier en texte brut. Vous pouvez en lire chaque ligne avant de décider de la partager ou non.

DBWarp Blueprint est gratuit et open source, et s’exécute entièrement dans votre environnement. Il vous permet de nous communiquer des faits sur votre base de données sans nous communiquer votre base de données elle-même.

## Pourquoi l’exécuter

Partagez votre sortie Blueprint avec nous et nous pourrons vous indiquer dans quelle mesure DBWarp accélérerait le déplacement de vos données, ainsi que l’incidence sur les calendriers de migration, de données de test CI/CD et d’analyse.

La distance est le facteur déterminant. Plus vos données doivent voyager loin, plus l’amélioration que DBWarp peut vous montrer est importante.

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; Zurich, Suisse

---

Exécutez `dbwarp-blueprint` dans votre propre environnement pour produire un fichier `blueprint.toml` anonymisé, consultable et de taille limitée, que DBWarp peut utiliser pour dimensionner et planifier la migration, sans avoir accès à la base de données, aux fichiers de sauvegarde, aux noms de schémas ou aux données des lignes.

Il se connecte à PostgreSQL, MySQL ou SQL Server, lit les métadonnées du catalogue, mesure éventuellement la compression locale à partir d'un échantillon de lignes de taille limitée et écrit du TOML en texte brut. Il peut aussi dériver un Blueprint depuis des fichiers Parquet ou Avro locaux en mode hors ligne lorsque l'entrée est déjà un fichier de données structurées plutôt qu'une base de données active. Vous pouvez ouvrir la sortie, en vérifier chaque ligne et décider de la partager ou non.

Facultativement, `--deck blueprint.pptx` écrit également une synthèse PowerPoint du même Blueprint anonymisé. La présentation peut être écrite pendant une exécution sur une base de données active ou ultérieurement à partir d'un fichier TOML vérifié avec `--from-toml blueprint.toml --deck blueprint.pptx`. Le module de création de présentations est intégré au binaire Rust et n'établit aucune connexion réseau.

## Utilité de l'outil

DBWarp a besoin d'informations structurelles suffisantes pour estimer et planifier un transfert :

- nombre de tables ;
- nombre approximatif de lignes ;
- taille des tables et des index ;
- familles de types des colonnes, capacités structurelles/préfixes d'index exacts et,
  par défaut, largeurs observées arrondies pour préserver la confidentialité ;
- structure des index et des clés étrangères ;
- comptages bornés et sans noms des artefacts hors tables et prérequis de déploiement externes ;
- synthèses facultatives de compression par table et colonne à partir d'un petit échantillon local ;
- mesure optionnelle du temps de trajet aller-retour du collecteur vers votre base de données.

Ces informations suffisent pour estimer la taille du transfert et planifier un transfert. Les noms de sources et les valeurs des lignes sont omis, mais la structure et les statistiques distinctives peuvent toujours permettre d'identifier une charge de travail ; l'anonymisation est une réduction des risques, et non une garantie d'irréversibilité.

## Ce que l'outil ne fait pas

`dbwarp-blueprint` ne fait pas ce qui suit :

- envoyer de la télémétrie ;
- appeler des serveurs DBWarp ;
- téléverser le fichier Blueprint ;
- lire `~/.pgpass`, `~/.my.cnf`, des informations d'identification cloud ou des clés SSH ;
- lire les variables d'environnement de mot de passe par défaut telles que `PGPASSWORD` ou `MYSQL_PWD` ;
- utiliser des répertoires système implicites de fichiers temporaires, cache ou configuration ; l'application écrit les fichiers de sortie explicitement sélectionnés, tandis que le mode batch utilise aussi un répertoire voisin de préparation ou de récupération à côté de `--out-dir` pour la publication atomique ;
- inclure dans la sortie les noms réels de tables, colonnes, index ou schémas, les noms d'objets hors tables, les définitions SQL, les points de terminaison externes, les informations d'identification, les clés, les certificats, les binaires ou les valeurs de lignes.

Les exécutions Blueprint actives ouvrent une session de base de données vers le point de terminaison indiqué. La résolution DNS peut utiliser le résolveur configuré, et l'authentification Kerberos/SSPI intégrée peut contacter l'infrastructure d'identité. Le mode batch répète cette limite pour chaque source de base de données. Les opérations TOML, Parquet, Avro et bundle locales n'ouvrent aucune connexion réseau initiée par l'application.

## Télécharger ou compiler

| Méthode | Usage recommandé | Lien |
|---|---|---|
| Téléchargez un fichier binaire. | essai rapide, hôte de test isolé. | [`binaries/README.md`](BINARIES.md) |
| Compiler depuis un petit clone des sources | revue de sécurité, politique de production, contrôle de reproductibilité | [`BUILD.md`](BUILD.md) |
| Compiler depuis un bundle de sources contenant les dépendances | audit strict et hors ligne des dépendances | GitHub Releases |
| Vérifier et exécuter le chemin de repli SQL | la politique DBA refuse un binaire tiers | [`sql/blueprint.pg.sql`](../../sql/blueprint.pg.sql), [`sql/blueprint.mysql.sql`](../../sql/blueprint.mysql.sql), [`sql/blueprint.sqlserver.sql`](../../sql/blueprint.sqlserver.sql) et [`blueprint_format.py`](../../blueprint_format.py) |

### Limite du chemin de repli SQL

Le chemin de repli SQL est un socle de catalogue vérifiable, pas un remplacement fonctionnellement équivalent du collecteur Rust. Chaque script SQL écrit un document JSON intermédiaire contenant les vrais noms de schémas, tables, colonnes et index ; avec MySQL, `COLUMN_TYPE` peut aussi contenir les membres enum/set déclarés. Traitez ce JSON comme du matériel de schéma sensible, conservez-le dans l'environnement source, normalisez-le localement avec `blueprint_format.py` et ne partagez que la sortie TOML vérifiée.

Le chemin de repli n'a pas de sélecteur `--schema` : PostgreSQL couvre les tables ordinaires de tous les schémas non système de la base connectée, tandis que MySQL et SQL Server couvrent les tables utilisateur locales ordinaires de la base sélectionnée. Ne l'utilisez pas si seule une partie est approuvée. Son TOML contient le sous-ensemble des tables ordinaires avec la structure table/colonne/index/FK et un dimensionnement local approximatif, mais n'inventorie pas tous les types de tables de v7. Il marque toutes les familles structurelles comme incomplètes et exclut les tables MySQL FEDERATED et les tables externes SQL Server au lieu de présenter à tort leurs données distantes comme locales. Il ne contient pas non plus d'échantillonnage de lignes, de preuve RTT, d'inventaire d'artefacts hors tables ni de sonde de topologie active ; la topologie et la complétude du jeu de données sont explicitement `unknown`.

La méthode privilégiant la confiance consiste à compiler depuis les sources. Le dépôt normal reste petit et utilise `Cargo.lock` pour figer les versions des dépendances. Pour les audits hors ligne plus stricts, chaque version publie également un bundle de sources contenant le code source de toutes les dépendances. Des binaires de version sont fournis par commodité, accompagnés de sommes de contrôle SHA256.

## Démarrage rapide

Avant toute exécution réelle, demandez à un DBA de provisionner un compte de
collecte dédié avec le minimum de privilèges, au moyen du script correspondant
au moteur, à la version et au niveau sous
[`sql/grants/`](../../sql/grants/), puis faites approuver la portée exacte des
schémas. Ne commencez jamais avec un compte propriétaire de l'application ou
administrateur. Passez chaque schéma approuvé avec `--schema`, puis supprimez
le compte dédié après la capture avec le script correspondant sous
[`sql/revoke/`](../../sql/revoke/). La procédure initiale complète figure dans
[`docs/QUICKSTART.md`](QUICKSTART.md).

Choisissez une langue de présentation lorsque cela est utile. L'anglais est la
langue par défaut ; des catalogues complets sont intégrés pour l'allemand, le
français, l'espagnol, le polonais, le japonais et le chinois simplifié :

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --schema app --dry-run
```

Seuls l'aide destinée aux humains, les demandes, les diagnostics, la progression
et les libellés des présentations PowerPoint sont traduits. Les noms de commandes
et d'options, les valeurs acceptées, les schémas d'URI, les noms de variables
d'environnement, les sélecteurs, les codes DBP, les clés d'audit et le TOML
généré restent des jetons anglais canoniques. Les procédures d'automatisation et
de support sont donc identiques dans toutes les langues. Consultez
[`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

Avant de vous connecter à une base de données, consultez les exemples disponibles sous [`samples/`](../../samples/). Ils sont des fichiers TOML Blueprint ordinaires et ne nécessitent aucun paramétrage pour être consultés. Une fois que vous avez obtenu un fichier binaire, une première exécution hors ligne peut générer un rapport sans nécessiter d'accès à une base de données ou à un réseau :

```bash
./dbwarp-blueprint --from-toml samples/sqlserver-v6-analyzed.toml --deck sample.pptx
```

Commencez par les petits exemples de Blueprint décrits dans [`samples/README.md`](../../samples/README.md) : catalogue PostgreSQL uniquement, échantillonnage MySQL et échantillonnage SQL Server avec artefacts analysés. Ce sont des exemples créés manuellement, et non de véritables captures. Les exemples plus importants de schema-v1 montrent le format plus ancien, qui est toujours lisible. Utilisez [`FORMAT.md`](FORMAT.md) pour examiner l'ensemble de la sortie avant d'approuver une capture ; aucun exemple ne couvre tous les champs optionnels.
Commencez par une simulation. Elle affiche le plan sans établir de connexion :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --dry-run
```

Exécution recommandée de type production avec TLS, journal d'audit et mesure de compression :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Avec `--measure-compression --yes`, la sortie inclut les taux zstd au niveau des tables et les estimations de compression par colonne. Les blocs par colonne sont calculés à partir du même échantillon limité que le taux au niveau de la table ; ils affinent l'estimation de transfert et n'écrivent pas les valeurs échantillonnées sur le disque. Un échantillon binaire dominant, avec des signatures de conteneur compressés standard reconnues, reçoit uniquement l'étiquette générale `style = "precompressed"` ; aucun type de fichier, signature ou valeur échantillonnée n'est sérialisé. Les schémas v3 et versions ultérieures génèrent également des agrégats cardinality/skew par colonne limités et sans nom, ainsi que des résumés index-prefix/relationship inférés. Les hachages temporaires par valeur sont limités en mémoire et supprimés ; les valeurs échantillonnées et les hachages par valeur n'apparaissent jamais dans le fichier TOML du Blueprint, contrairement aux agrégats documentés.

Depuis le schéma v4, les Blueprints inventorient également les objets hors tables. Par défaut,
`--artifact-detail summary` conserve des comptages bornés par classe d'objet et
de prérequis externe sans lire les définitions. `graph` ajoute une topologie de
dépendances anonyme et `analyzed` des bandes bornées de caractéristiques du
langage et de complexité ; tous deux exigent `--yes`, car même un graphe anonyme
peut identifier une application :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```


La présence d'un artefact constitue une preuve de planification, pas une
promesse que DBWarp peut le recréer ou le traduire automatiquement. Consultez
[`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

### Fidélité des longueurs MySQL

La politique `balanced` par défaut préserve exactement les capacités déclarées
en caractères/octet et les longueurs de préfixe d'index. Les longueurs
moyenne/p95 des valeurs échantillonnées utilisent des intervalles à erreur
relative (environ 3,2 % d'erreur maximale, avec conservation exacte des valeurs
jusqu'à 32 octets). Ainsi, une clé `VARCHAR(3000)` dont les valeurs contiennent
normalement 9 caractères reste proche de 9 caractères, de sorte que le
dimensionnement reflète la largeur réelle des valeurs tout en conservant des
limites DDL/d'index source valides :

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

N'utilisez les statistiques échantillonnées exactes que lorsque la politique autorise cette précision supplémentaire :

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

Utilisez `--length-fidelity strict` pour appliquer un regroupement grossier de confidentialité pour les longueurs déclarées, observées et de préfixe. Le mode strict réduit la précision de l'estimation résultante. L'orthographe `--preserve-exact-lengths --yes` reste un alias pour `--length-fidelity exact --yes`.

Les nouveaux modèles enregistrent des champs distincts `declared_length_fidelity`, `index_length_fidelity` et `observed_length_fidelity`. Le champ `length_metadata` est également écrit afin que les outils qui lisent le format précédent continuent de fonctionner. Les capacités de caractères de PostgreSQL sont des valeurs exactes du catalogue ; les limites de bytes dépendantes de l'encodage et les longueurs de préfixe d'index restent indisponibles.

Pour obtenir l'estimation la plus précise, exécutez avec `--measure-compression` : cela enregistre les longueurs moyennes observées et la valeur p95, de sorte qu'une colonne déclarée beaucoup plus large que ses valeurs réelles ne sera pas surestimée. Le budget d'échantillonnage par défaut est de 300 secondes ; augmentez `--max-wall-secs` pour les schémas très volumineux.

Vérifiez ensuite les fichiers :

```bash
less blueprint.toml
less audit.txt
```

Suivez les [étapes de révision et de partage](QUICKSTART.md#review-and-share) avant d'envoyer tout artefact. Partagez uniquement le contenu du Blueprint approuvé et, si celui-ci a été examiné et approuvé séparément, une présentation ; conservez par défaut les preuves opérationnelles localement.

## Mode fichier structuré

Si la source est déjà un fichier structuré local, générez le TOML Blueprint sans informations d'identification de base de données :

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

```bash
./dbwarp-blueprint \
  --from-avro /data/sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Le mode Parquet lit le pied de fichier et les métadonnées des groupes de lignes. Les conteneurs d'objets Avro ne possèdent pas de nombre de lignes équivalent dans un pied de fichier ; le mode Avro parcourt donc le conteneur pour compter les enregistrements et utilise le schéma d'écriture pour déterminer la structure des colonnes. Aucun de ces modes ne se connecte à une base de données ni ne lit d'options d'informations d'identification.

Si votre politique permet l'échantillonnage décodé, le mode fichier peut aussi
mesurer une compressibilité locale bornée pour la planification du transfert :

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Les mêmes options fonctionnent avec `--from-avro`. Les valeurs échantillonnées
sont encodées en mémoire sous la forme `blueprint-compression-probe-v2` ; le
Blueprint conserve les mesures agrégées de compression, densité de valeurs NULL,
cardinalité/fréquence, longueur et style, jamais les valeurs échantillonnées.

## Mode lot et bundle

Pour plusieurs bases de données, plusieurs tables/datasets, ou pour une analyse de l'ensemble d'un système, utilisez un manifeste par lots et créez un répertoire de regroupements :

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Le répertoire de travail contient `bundle.toml`, des fichiers Blueprint enfants
par source et des journaux d'audit par source à accès contrôlé. Ne transférez
pas l'ensemble du répertoire de travail par défaut. Vous pouvez en afficher le
contenu, en extraire des éléments ou créer un bundle Blueprint empaqueté et vérifié séparément :

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
./dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 --out table-042.blueprint.toml
./dbwarp-blueprint --bundle-pack customer-blueprint-bundle --out customer-blueprint-bundle.packed.toml
```

Consultez [`docs/BATCH_AND_BUNDLES.md`](BATCH_AND_BUNDLES.md) pour la syntaxe
du manifeste, les modes de jeu de données de fichiers structurés et les règles
des sélecteurs.

## Commandes courantes par base de données

PostgreSQL :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

MySQL :

```bash
./dbwarp-blueprint \
  --connect mysql://app@db.internal/payments \
  --schema payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

SQL Server :

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@db.internal,1433/payments \
  --schema dbo \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

Pour des exemples Kerberos, SSPI et Entra ID, consultez [`AUTH.md`](AUTH.md). Pour les autorités de certification internes, mTLS et la vérification du nom d'hôte, consultez [`TLS.md`](TLS.md).

## Mode catalogue uniquement

Si la politique autorise uniquement les catalogues de tables, colonnes, index et clés étrangères, omettez `--measure-compression` et désactivez explicitement la synthèse par défaut des objets hors tables :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

Ce mode, qui ne concerne que le catalogue, lit les métadonnées des tables, les statistiques et une analyse topologique qui ne compte que le nombre d'éléments, mais ne lit aucune valeur de ligne ni aucun inventaire d'objets autres que les tables. DBWarp peut toujours estimer à partir de la taille des tables, du nombre de lignes, des familles de types et de la forme index/FK, mais les estimations de compression sont moins précises car l'entropie text/binary doit être déduite. Sans `--artifact-detail none`, le résumé par défaut lit également les catalogues d'objets autres que les tables, mais pas leurs définitions.

## Aperçu de la sortie

```toml
# dbwarp-blueprint v7
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 7
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 1
row_count = 12500000
table_bytes = 4194304000
index_bytes = 1048576000

[database_topology]
contract = "dbwarp-blueprint-topology/v2"
deployment = "unknown"
local_role = "unknown"
visibility = "unknown"
member_count = 0
member_count_scope = "unknown"
identifiers_redacted = true

[dataset_scope]
contract = "dbwarp-blueprint-dataset-scope/v1"
layout = "unknown"
table_inventory_completeness = "unknown"
row_count_completeness = "unknown"
size_completeness = "unknown"
row_count_method = "postgres-planner-estimate"
size_method = "postgres-local-relation-size"
limitations = ["topology-unobserved", "topology-visibility-unknown"]

[structure_scope]
contract = "dbwarp-blueprint-structure-scope/v1"
visibility = "unknown"
table_inventory_completeness = "unknown"
column_inventory_completeness = "unknown"
index_inventory_completeness = "unknown"
relationship_inventory_completeness = "unknown"
limitations = ["metadata-visibility-unknown"]

[source_environment]
contract = "dbwarp-blueprint-source-environment/v1"
evidence_origin = "database-endpoint"
hosting_model = "unknown"
infrastructure_location = "unknown"
capacity_scope = "connected-instance"
capacity_visibility = "partial"
cpu_capacity_band = "unknown"
cpu_capacity_basis = "unknown"
memory_capacity_band = "under-2-gib"
memory_capacity_basis = "database-buffer-cache"
collector_machine_excluded = true
catalogs_read = ["pg-capacity-settings"]

[statistics_evidence]
contract = "dbwarp-blueprint-statistics-evidence/v1"
visibility = "unknown"
table_count = 1
counts_by_statistics_state = { unknown = 1 }
counts_by_row_count_quality = { unknown = 1 }
counts_by_size_quality = { unknown = 1 }
limitations = ["statistics-provenance-unclassified"]

[artifact_inventory]
contract = "dbwarp-blueprint-artifacts/v2"
detail = "none"
scope = "all-visible-schemas"
visibility = "unknown"
inventory_complete = false
dependencies_complete = false
requirements_complete = false
analysis_complete = false
families_not_inventoried = ["non_table_objects"]

[tables.table-001]
rows = 12500000
table_bytes = 4194304000
index_bytes = 1048576000
schema = "schema-A"
has_clustered_index = false
object_kind = "ordinary-table"
storage_organization = "unknown"
partitioning = "none"
segment_state = "unknown"

[tables.table-001.statistics]
row_count_method = "postgres-planner-estimate"
row_count_quality = "unknown"
statistics_state = "unknown"
refresh_age_band = "unknown"
modification_ratio_band = "unknown"
sample_fraction_band = "unknown"
statistics_scope = "unknown"
size_method = "postgres-local-relation-size"
size_quality = "unknown"
size_scope = "unknown"
size_accounting = "unknown"
size_visibility = "unknown"

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false
numeric_model = "integer"
numeric_precision_radix = "decimal"

[tables.table-001.idxs.idx-1]
type = "btree"
primary = true
unique = true
cols = [1]
```

Le contrat complet du fichier est documenté dans [`FORMAT.md`](FORMAT.md). Le journal d'audit est documenté dans [`AUDIT.md`](AUDIT.md).

## Présentation visuelle récapitulative

Générez une présentation pendant l'exécution active :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

Ou générez-la ultérieurement depuis un fichier Blueprint vérifié, sans connexion à la base de données :

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx
```

La présentation s'adapte à la taille du schéma : détails par table pour les petits schémas, diapositives de caractérisation pour les grands schémas, synthèse de compression lorsque des données Tier 2 sont présentes et diapositive sur le modèle de confiance. Consultez [`DECK.md`](DECK.md).

## Documentation

Pour commencer :

- [`docs/QUICKSTART.md`](QUICKSTART.md) : premier lancement sécurisé et informations à partager.
- [`docs/COOKBOOK.md`](COOKBOOK.md) : recettes pratiques pour PostgreSQL, MySQL, SQL Server, TLS, les présentations et les flux de travail sans échantillonnage.
- [`docs/DBA_REVIEW_GUIDE.md`](DBA_REVIEW_GUIDE.md) : ce qu'un DBA ou responsable de la sécurité doit savoir avant d'exécuter l'outil.
- [`sql/grants/README.md`](../../sql/grants/README.md) : scripts d’octroi de privilèges minimaux tenant compte des versions et suppression du compte après la capture.
- [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md) : échecs courants et corrections.
- [`docs/MESSAGES.md`](MESSAGES.md) : codes de message opérateur stables `DBPnnnnS`.
- [`docs/COMPRESSION_MEASUREMENT.md`](COMPRESSION_MEASUREMENT.md) : fonctionnement de l'échantillonnage de compression Tier 2.
- [`docs/INDEX.md`](INDEX.md) : carte complète de la documentation.

Points de départ de la revue de sécurité :

- [`SECURITY.md`](SECURITY.md) : modèle de sécurité et gestion des informations d'identification.
- [`AUDIT.md`](AUDIT.md) : éléments lus, écrits, interrogés et journalisés.
- [`FORMAT.md`](FORMAT.md) : champs de sortie et règles d'arrondi.
- [`TLS.md`](TLS.md) : comportement TLS et mTLS.
- [`AUTH.md`](AUTH.md) : modes d'authentification pris en charge.
- [`BUILD.md`](BUILD.md) : compilation depuis les sources et vérification des versions.
- [`DECK.md`](DECK.md) : présentation PowerPoint facultative de synthèse.

## Licence

Apache-2.0 OR MIT.
