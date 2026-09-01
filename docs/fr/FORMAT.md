# Format de fichier dbwarp-blueprint v6

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../FORMAT.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../FORMAT.md) | [Deutsch](../de/FORMAT.md) | **Français** | [Español](../es/FORMAT.md) | [Polski](../pl/FORMAT.md) | [日本語](../ja/FORMAT.md) | [简体中文](../zh/FORMAT.md)

Lisible par un humain. Facile à comparer. Vérifiable à des fins forensiques.

> **Ce format réduit le risque de canal caché et de divulgation directe grâce à
> un schéma borné, des identifiants déterministes et une précision numérique
> documentée. La structure anonyme du graphe et les champs exacts facultatifs
> peuvent encore caractériser une charge de travail ; vérifiez donc le fichier
> selon votre propre politique de classification des données.**

## En-tête du fichier

À l'identique, octet pour octet :

```
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

La ligne vide fait partie du contrat. L'outil émet exactement cet en-tête et aucun autre commentaire. Cela facilite la détection de tout contenu de commentaire inattendu ; ce n'est pas une affirmation selon laquelle les autres champs structurés ne peuvent pas identifier un schéma ou un graphe de dépendances distinctif.

## Champs de premier niveau

| Champ | Type | Description |
|---|---|---|
| `schema_version` | int | Version du format. Actuellement `6` ; les versions 1 à 5 restent lisibles. |
| `generated_at` | ISO-8601 string | Horodatage UTC, à la seconde, sans fraction. **Peut être figé** au moyen de l'option CLI `--generated-at "2026-04-26T00:00:00Z"` pour les exécutions reproductibles et identiques octet pour octet. Le journal d'audit enregistre `generated_at_pin: ...` chaque fois que l'option est définie, afin que ce choix soit visible lors d'une analyse forensique. Cette option est le seul moyen de figer cette valeur : aucune variable d'environnement n'est jamais lue, conformément au contrat de confiance « no env vars read by default » du README. |
| `engine` | string | `"postgresql"`, `"mysql"` ou `"sqlserver"`. |
| `engine_version` | string | Chaîne de version renvoyée par le moteur de base de données. |
| `source_kind` | string | L'une des valeurs `"production"`, `"staging"`, `"scrubbed-replica"`, `"synthetic"`. Déclarée par le client. |
| `length_metadata` | string | Marqueur de compatibilité historique : `"hybrid-v2"`, `"exact"`, `"rounded"` ou `"not-captured"`. Les nouveaux consommateurs doivent utiliser les trois champs ci-dessous. |
| `declared_length_fidelity` | string | `"exact"` pour les capacités déclarées en caractères de PostgreSQL et pour les modes MySQL équilibré par défaut/exact ; `"coarse-rounded-v1"` pour la confidentialité MySQL stricte ; `"not-captured"` lorsque l'information n'est pas disponible. |
| `index_length_fidelity` | string | `"exact"` pour les préfixes d'index MySQL équilibrés par défaut ou exacts ; `"rounded-down-v1"` pour la confidentialité stricte ; `"not-captured"` lorsque l'information n'est pas disponible. |
| `observed_length_fidelity` | string | `"relative-rounded-v2"` par défaut lorsque l'échantillonnage a eu lieu, `"exact"` en mode exact, `"coarse-rounded-v1"` en mode strict ou `"not-sampled"`. La couverture de l'échantillonnage reste une exigence distincte pour chaque colonne. |
| `[totals]` | inline table | Nombres agrégés (voir ci-dessous). |
| `[network]` | table | Preuve facultative de connexion client-base et de RTT de requête. |
| `[database_topology]` | table | Obligatoire pour les sources de base en schéma v6. Déploiement, rôle local, visibilité et preuves de catalogue respectueux de la confidentialité. Absent pour les fichiers structurés. |
| `[dataset_scope]` | table | Obligatoire pour chaque Blueprint en schéma v6. Déclare ce que couvrent les totaux et si la couverture des tables, lignes et octets est complète. |
| `[tables.X]` | tables | Une entrée par table, avec identifiant anonymisé. |
| `[fk_edges]` | inline table | Graphe des clés étrangères entre tables anonymisées. Facultatif. |
| `[artifact_inventory]` | table | Comptages respectueux de la confidentialité des objets hors tables, graphe de dépendances anonyme facultatif, prérequis externes et recensement borné facultatif du langage. Sources de bases de données uniquement. |

## `[totals]`

| Champ | Type | Précision |
|---|---|---|
| `table_count` | int | exacte |
| `row_count` | int | somme des valeurs `rows` arrondies par table |
| `table_bytes` | int | somme des valeurs `table_bytes` arrondies par table |
| `index_bytes` | int | somme des valeurs `index_bytes` arrondies par table |

Ces nombres ne sont pas automatiquement les totaux du cluster entier. Ils
doivent toujours être interprétés avec `[dataset_scope]`. Une passerelle ou un
coordinateur shardé peut exposer un catalogue apparemment complet sans détenir
les shards sous-jacents. Le schéma v6 représente explicitement cette
incertitude au lieu de présenter silencieusement les statistiques locales
comme une vérité globale.

## `[database_topology]` (sources de base en schéma v6)

Ce bloc conserve uniquement des faits bornés visibles par le point de
connexion à la base. Il ne stocke jamais de noms de nœuds ou d'hôtes,
d'adresses IP, de noms de cluster ou de canal de réplication, d'identifiants de
serveur ni de points de terminaison.

| Champ | Valeurs / règle |
|---|---|
| `contract` | Toujours `dbwarp-blueprint-topology/v1`. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed` ou `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member` ou `unknown`. |
| `visibility` | `full`, `partial` ou `unknown` ; décrit les preuves de topologie, pas la justesse des données. |
| `member_count` | Nombre de membres visibles par des requêtes de preuve réussies. `0` signifie inconnu, jamais zéro membre. |
| `identifiers_redacted` | Doit valoir `true`. |
| `role_counts` | Comptages facultatifs par jeton de rôle fermé. Une visibilité complète exige que leur somme égale `member_count`. |
| `features` | Jetons fermés triés tels que `citus`, `mysql-group-replication`, `mysql-galera`, `mysql-ndb`, `postgresql-streaming-replication`, `sqlserver-availability-group` ou `vitess`. |
| `catalogs_read` | Libellés fermés triés des catalogues de topologie lus avec succès. |
| `catalogs_unreadable` | Libellés fermés triés des catalogues de topologie illisibles. Toute entrée empêche d'affirmer une visibilité complète. |

Un point de connexion ordinaire peut légitimement signaler
`deployment = "unknown"` tout en fournissant des statistiques locales
complètes d'une copie intégrale. Blueprint ne déduit pas qu'un serveur sans
caractéristique particulière est single-node simplement parce qu'aucune
fonction de cluster n'était visible.

## `[dataset_scope]` (schéma v6)

Ce bloc qualifie indépendamment chaque total de dimensionnement. Les
consommateurs doivent refuser tout calcul non qualifié sur le jeu de données
entier lorsqu'une dimension requise est `incomplete` ou `unknown`.

| Champ | Valeurs / règle |
|---|---|
| `contract` | Toujours `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset` ou `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete` ou `unknown`. |
| `row_count_completeness` | `complete`, `incomplete` ou `unknown`. |
| `size_completeness` | `complete`, `incomplete` ou `unknown`. |
| `row_count_method` | Jeton de provenance fermé tel que `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter` ou `distributed-aggregate`. |
| `size_method` | Jeton de provenance fermé tel que `postgres-local-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `citus-distributed-relation-size` ou `distributed-aggregate`. |
| `limitations` | Raisons fermées et triées d'une couverture incomplète ou inconnue. Au moins une est obligatoire sauf si toutes les dimensions sont complètes. |

`selection-limited` signifie que les totaux et les déclarations de complétude couvrent exactement les schémas demandés au moyen du sélecteur actif répétable `--schema` ; ils ne prétendent pas couvrir toute la base de données connectée. Sans `--schema`, la capture de tous les schémas visibles est conservée.

Les collecteurs natifs PostgreSQL, MySQL et SQL Server interrogent les
catalogues de topologie pris en charge avant de décider si les statistiques
locales représentent le jeu logique. Les passerelles distribuées connues
suppriment les totaux dangereux lorsqu'aucun agrégat fiable n'est disponible.
Le formateur SQL de repli ne possède aucune sonde de topologie : il émet donc
ses estimations locales utiles avec toutes les dimensions marquées `unknown`
et les limitations `topology-unobserved` et
`topology-visibility-unknown`.

Les Blueprints Parquet et Avro structurés omettent `[database_topology]` et
utilisent `layout = "structured-dataset"` avec une provenance de
footer/conteneur.

Blueprint n'exécute aucun test de vitesse du stockage pendant une collecte
ordinaire et ne déduit pas le matériel du serveur de base depuis la machine qui
exécute le client. Les totaux d'octets décrivent le volume stocké selon la
méthode de catalogue indiquée ; ils ne prétendent pas connaître le type de
disque, les IOPS, le débit, le CPU, la RAM ni les performances de migration
cible.

## `[network]` (facultatif)

Statistiques de temps d'aller-retour réseau observées côté client entre l'outil Blueprint et la base de données source. Il ne s'agit **PAS** du RTT entre la source et la cible de la migration : ces données indiquent seulement à quelle distance l'outil Blueprint se trouvait de la base de données source du client au moment de l'exécution. L'estimateur en aval les utilise uniquement pour vérifier la plausibilité du RTT de migration fourni par l'opérateur (par exemple, un opérateur qui annonce un RTT de migration de 200 ms est peu crédible si la sonde locale du client mesure 0,4 ms : l'outil Blueprint s'exécutait probablement sur la base source elle-même).

La sonde s'exécute après l'établissement de la connexion et avant les requêtes de catalogue, afin que les mesures ne soient pas faussées par le préchauffage du cache de requêtes. Elle exécute **5× `SELECT 1`** et émet la latence médiane. Chaque `SELECT 1` renvoie l'entier constant 1 : cette sonde ne lit jamais de données de ligne.

Le bloc est absent lorsque le client a indiqué `--no-rtt-probe` ou lorsque la sonde elle-même a échoué en cours d'exécution (l'échec est consigné comme avertissement non fatal dans stderr et dans le journal d'audit ; le fichier Blueprint est tout de même émis sans ce bloc).

| Champ | Type | Précision |
|---|---|---|
| `sample_count` | int | exacte (toujours 5 dans v1) |
| `connect_total_ms` | int | temps total écoulé entre le début de la connexion TCP et la disponibilité de la session authentifiée, en millisecondes. Comprend la négociation TCP, la négociation TLS le cas échéant et le défi/réponse d'authentification. Arrondi à la milliseconde la plus proche. Généralement 3 à 6 fois `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | latence médiane d'un aller-retour parmi les 5 échantillons `SELECT 1`, en millisecondes. Arrondie à la milliseconde la plus proche. Le bruit réseau naturel (≥ 1 ms en pratique) est supérieur à la granularité d'arrondi ; celui-ci élimine donc tout canal caché dans les bits de poids faible sans perdre de précision utile. Les valeurs LAN inférieures à la milliseconde deviennent 0 ou 1. |
| `query_rtt_ms_p95` | int | 95e centile des 5 échantillons calculé selon la méthode du rang le plus proche (l'observation la plus lente), en millisecondes. Arrondi à la milliseconde la plus proche. Utilisez-le avec p50 pour repérer de brefs pics de latence ; cinq échantillons servent uniquement de repère et ne constituent pas un benchmark de charge de travail. |

Les 5 requêtes de sonde apparaissent dans le journal d'audit sous la forme d'une **entrée récapitulative unique** (et non de 5 lignes distinctes) libellée « 5x SELECT 1 (RTT probe; constant integer 1, no row data) », conformément au principe de confiance selon lequel aucun contenu de ligne n'est lu.

## `[tables.<id>]`

L'identifiant prend la forme `table-NNN`, où `NNN` est l'ordinal indexé à partir
de 1 dans un ordre HMAC-SHA256 avec séparation de domaine du nom de schéma et
du nom de table. Par défaut, la clé est générée à nouveau pour le processus et
n’est jamais émise. Le même `--anonymization-key-file` conservé par le client
préserve cet ordre entre les exécutions de comparaison approuvées.

| Champ | Type | Précision / valeurs |
|---|---|---|
| `rows` | int | arrondi : à la centaine la plus proche (≤10k), au millier (≤1M), à la dizaine de milliers (>1M) |
| `table_bytes` | int | arrondi : au 1KiB, 1MiB ou 100MiB le plus proche selon l'ordre de grandeur |
| `index_bytes` | int | arrondi : identique à `table_bytes` |
| `schema` | string | identifiant anonymisé `schema-A`, `schema-B`, ..., `schema-AA` |
| `kind` | string | Jeton fermé facultatif du schéma v6 : `partitioned`, `materialized-view`, `temporal-current`, `temporal-history`, `memory-optimized`, `external`, `graph-node` ou `graph-edge`. Omis pour une table ordinaire ou une preuve inconnue. |
| `unlogged` | bool | Observation facultative du catalogue PostgreSQL dans le schéma v6. Omise si elle n'est pas capturée ; `false` explicite signifie que le catalogue a confirmé une table journalisée. |
| `partition_strategy` | string | Jeton facultatif du schéma v6 pour `partitioned` : `range`, `list`, `hash`, `key` ou `linear-hash`. |
| `partition_count` | int | Nombre exact et positif de partitions feuilles dans le schéma v6, requis avec `kind = "partitioned"`. |
| `partition_key_cols` | array of int | Ordinaux des colonnes d'une clé de partitionnement simple dans le schéma v6. Omis pour une clé d'expression ou lorsque la preuve du catalogue est indisponible ; aucune expression de clé n'est sérialisée. |
| `partition_rows_max` | int | Estimation arrondie facultative du nombre de lignes de la plus grande partition feuille dans le schéma v6. |
| `temporal_history` | string | Identifiant de la table `temporal-history` associée dans le schéma v6, requis pour `temporal-current`. |
| `counted_in_totals` | bool | Schéma v6. L'absence signifie l'inclusion dans tous les totaux agrégés. `external` exige `false` explicite, ce qui exclut la table de `table_count`, `row_count`, `table_bytes` et `index_bytes` ; aucune autre valeur explicite n'est canonique. |
| `check_count` | int | Nombre structurel exact facultatif de contraintes CHECK dans le schéma v6. L'absence signifie inconnu ; `0` signifie que le catalogue concerné n'en a trouvé aucune. |
| `has_clustered_index` | bool | toujours `false` pour PostgreSQL |
| `stats_freshness` | string | `"fresh"` / `"stale"` / `"never_analyzed"` (PG), vide en cas de repli SQL |
| `[tables.<id>.cols.<cid>]` | sub-tables | une par colonne |
| `[tables.<id>.idxs.<iid>]` | sub-tables | une par index |
| `[tables.<id>.compression]` | sub-table | uniquement avec Tier 2 |

## `[tables.<id>.cols.<cid>]`

L'identifiant prend la forme `col-N`, où `N` correspond à l'ordre naturel des attributs de la colonne (indexé à partir de 1 et conservant l'ordinal sur disque). Il reste stable d'une exécution à l'autre.

| Champ | Type | Notes |
|---|---|---|
| `ordinal` | int | le même N que dans l'identifiant |
| `type` | string | famille de types normalisée, par exemple `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"` ou `"user-defined"`. Les noms réels de domaines, d'énumérations, d'alias, de composites et de types définis par l'utilisateur ne sont pas émis. |
| `nullable` | bool | |
| `value_source` | string | Jeton fermé facultatif du schéma v6 : `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time` ou `rowversion`. Omis pour une valeur fournie ordinaire ou une preuve inconnue. |
| `has_default` | bool | Observation facultative du catalogue dans le schéma v6. L'absence signifie inconnu ; `false` explicite signifie que le catalogue a confirmé l'absence de valeur par défaut. |
| `default_kind` | string | Classification facultative `constant`, `function` ou `expression` dans le schéma v6, valide uniquement avec `has_default = true`. Le texte et les littéraux de la valeur par défaut ne sont jamais sérialisés. |
| `type_kind` | string | Jeton fermé facultatif du schéma v6 : `enum`, `set`, `domain`, `composite`, `array`, `range` ou `alias`. Omis pour un type de base ou une preuve inconnue. |
| `member_count` | int | Nombre structurel exact et positif de membres dans le schéma v6, requis uniquement pour `enum` et `set`. Les noms des membres ne sont jamais sérialisés. |
| `domain_has_check` | bool | Observation facultative du CHECK d'un domaine dans le schéma v6, valide uniquement avec `type_kind = "domain"`. |
| `hidden`, `masked`, `encrypted`, `sparse` | bool | Observations facultatives du catalogue dans le schéma v6. L'absence signifie inconnu ; `false` explicite signifie que le catalogue a confirmé l'absence de la propriété. |
| `has_check` | bool | Observation facultative d'un CHECK sur une seule colonne dans le schéma v6. Chaque `true` explicite est couvert par le `check_count` de la table. |
| `null_fraction` | float | Fraction de valeurs NULL observée facultative, comprise entre `0.0` et `1.0`. Seul l'agrégat arrondi est conservé ; aucun bitmap de valeurs NULL n'est conservé. |
| `native_type` | string | Type de base facultatif et assaini du moteur, par exemple `varchar` ou `longtext` ; aucun identifiant, membre d'énumération, valeur par défaut ou expression. Actuellement émis par la capture MySQL corrigée. |
| `declared_max_chars` | int | Capacité déclarée facultative en caractères. Exacte pour les valeurs de catalogue PostgreSQL `character`/`character varying` et dans les modes MySQL équilibré par défaut/exact ; arrondie grossièrement uniquement avec MySQL `--length-fidelity strict`. |
| `declared_max_bytes` | int | Capacité déclarée facultative en octets. Exacte dans les modes MySQL équilibré par défaut et exact ; arrondie grossièrement uniquement avec `--length-fidelity strict`. |
| `numeric_precision`, `numeric_scale`, `datetime_precision` | int | Précision scalaire facultative déclarée par le moteur. |
| `charset`, `collation` | string | Métadonnées de caractères MySQL facultatives et assainies. Il s'agit de noms de catalogue, jamais d'identifiants ni de valeurs du client. |
| `len_avg` | int | Moyenne échantillonnée des octets pour les valeurs de longueur variable. Les classes relatives par défaut ont une erreur maximale d'environ 3,2 % et conservent exactement les valeurs jusqu'à 32 octets ; valeur exacte avec `--length-fidelity exact --yes` ; arrondi grossier à la dizaine uniquement en mode strict. 0 = longueur fixe ou non mesurée. |
| `len_p95` | int | 95e centile échantillonné avec les mêmes classes relatives par défaut ; valeur exacte avec `--length-fidelity exact --yes` ; arrondi grossier à la centaine uniquement en mode strict. 0 = non mesuré. |
| `style` | string | Tier 2 uniquement. L'une des valeurs `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"` ; vide si aucune classification n'est disponible. |
| `magnitude_min`, `magnitude_max` | int | Exposants décimaux signés facultatifs du schéma v6 délimitant l'ordre de grandeur des nombres non NULL échantillonnés. Ils sont émis avec `has_negative` ; les valeurs exactes ne sont jamais sérialisées. |
| `has_negative` | bool | Observation facultative du signe dans le schéma v6, émise uniquement avec les deux limites d'ordre de grandeur. |
| `time_span` | string | Plage date/heure échantillonnée facultative du schéma v6 : `intraday`, `days`, `weeks`, `months`, `years` ou `decades`. |
| `time_recent_decade` | int | Décennie contenant la date/heure échantillonnée la plus récente dans le schéma v6, émise uniquement avec `time_span` et toujours divisible par 10. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Tier 2 uniquement. Présente pour les colonnes candidates de texte ou de données binaires échantillonnées. Même disposition des champs que la compression au niveau de la table, mais limitée à une colonne anonymisée. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Synthèse de la distribution des valeurs échantillonnées du schéma v3. Contient uniquement des comptages et fréquences bornés ou arrondis. |

### `[tables.<id>.cols.<cid>.cardinality]` (schéma v3)

Lorsque l'échantillonnage des lignes est activé, le collecteur conserve au plus
8 192 empreintes temporaires de 64 bits par colonne en mémoire, calcule des
statistiques agrégées de NDV et d'asymétrie, puis supprime les empreintes. Ni les
valeurs ni les empreintes ne sont sérialisées. Le bloc contient `measured`,
`sample_rows`, `non_null_rows`, `observed_distinct_count`,
`estimated_distinct_count`, `top_value_fraction`, `frequency_p50`,
`frequency_p95`, `frequency_p99`, `frequency_max`, `sample_method`,
`sampled_with_bias` et `bias_reason`.

Les comptages et les fractions sont arrondis lorsque la confidentialité
l'exige. Ces statistiques servent à reproduire la densité des doublons,
l'asymétrie des valeurs fréquentes et les domaines finis dans les fixtures
synthétiques ; elles ne permettent pas de reconstruire les valeurs source ni
leur signification métier.

### `[tables.<id>.cols.<cid>.compression]` (Tier 2 uniquement)

La compression par colonne est émise uniquement pour les candidats texte/binaires bornés lorsque `--measure-compression --yes` est utilisé. Elle permet aux outils en aval de générer des données synthétiques textuelles/binaires dont l'entropie est plus réaliste qu'avec les seuls ratios au niveau de la table.

Le bloc contient les mêmes champs que `[tables.<id>.compression]` : `measured`, `sample_rows`, `sample_bytes`, `sample_method`, `sampled_with_bias`, `bias_reason`, `ratio_zstd_3`, `ratio_zstd_19`, `ratio_stddev` et `sample_encoding`.

Exemple :

```toml
[tables.table-001.cols.col-2]
ordinal = 2
type = "json"
nullable = false
len_avg = 430
len_p95 = 0
style = "json"

[tables.table-001.cols.col-2.compression]
measured = true
sample_rows = 1000
sample_bytes = 65536
sample_method = "column TABLESAMPLE SYSTEM(0.1) LIMIT N (text format)"
sampled_with_bias = false
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

Aucune valeur de colonne échantillonnée n'est écrite dans le fichier Blueprint.

## `[tables.<id>.idxs.<iid>]`

L'identifiant prend la forme `idx-N`, où `N` est l'ordinal indexé à partir de 1 de l'index dans la table, trié selon un HMAC-SHA256 avec séparation de domaine du nom d’index.

| Champ | Type | Valeurs |
|---|---|---|
| `type` | string | Famille de méthodes d'index normalisée, par exemple `"btree"`, `"hash"`, `"gin"`, `"gist"`, `"brin"`, `"spgist"`, `"fulltext"`, `"spatial"`, `"clustered"`, `"nonclustered"`, `"clustered columnstore"`, `"nonclustered columnstore"` ou `"other"`. Les noms de méthodes d'extension ou personnalisées ne sont pas émis. |
| `primary` | bool | Facultatif ; émis avec la valeur `true` pour les index de clé primaire. Omis ou faux dans les autres cas. |
| `unique` | bool | |
| `cols` | array of int | ordinaux des colonnes participantes, dans l'ordre des colonnes de l'index |
| `prefix_lengths` | array of int | Longueurs facultatives des préfixes d'index MySQL alignées sur `cols` ; zéro signifie la colonne entière. Exactes par défaut ; arrondies vers le bas uniquement avec `--length-fidelity strict`. |
| `include_cols` | array of int | Facultatif ; ordinaux des colonnes INCLUDE hors clé lorsque le moteur source les expose. |
| `expression` | bool | Facultatif ; vrai lorsqu'un élément de clé basé sur une expression ou une fonction existe et ne peut pas être représenté par de simples ordinaux de colonnes. |
| `filtered` | bool | Facultatif ; vrai pour les index filtrés ou partiels. |
| `descending` | bool | Facultatif ; vrai lorsqu'au moins une colonne de clé est explicitement descendante. |
| `prefix_distinct_counts` | array of int | Nombre estimé par le schéma v3 de tuples distincts pour chaque préfixe de clé, d'une à N colonnes. Zéro signifie que la valeur n'est pas disponible pour ce préfixe. |
| `cardinality_sample_method` | string | Provenance bornée de `prefix_distinct_counts` ; les produits inférés sont explicitement étiquetés et ne sont pas présentés comme des échantillons directs de tuples. |

## `[tables.<id>.compression]` et `[tables.<id>.cols.<cid>.compression]` (Tier 2 uniquement)

Présents uniquement lorsque le fichier a été généré avec `--measure-compression --yes`. Le bloc au niveau de la table mesure le flux complet des lignes échantillonnées et reste le ratio faisant autorité pour les estimations de transfert de la table entière. Les blocs au niveau des colonnes sont projetés à partir des mêmes lignes échantillonnées, colonne par colonne, et servent à aider les générateurs de fixtures synthétiques en aval à ajuster l'entropie de chaque colonne sans voir les valeurs du client. Ils ne déclenchent aucune lecture supplémentaire de la base de données.

| Champ | Type | Précision |
|---|---|---|
| `measured` | bool | toujours `true` si le bloc est présent |
| `sample_rows` | int | exacte |
| `sample_bytes` | int | taille du tampon d'échantillons en mémoire, **regroupée par paliers** : au multiple de **64 KiB** le plus proche pour une valeur inférieure à 1 MiB, au multiple de **1 MiB** le plus proche pour une valeur inférieure à 1 GiB, au multiple de **100 MiB** le plus proche au-delà. Les octets ne sont jamais écrits sur disque. Cette discrétisation élimine le canal caché dans les bits de poids faible par table qu'un `buf.len()` exact exposerait autrement. |
| `sample_method` | string | description bornée de l'échantillonnage propre au moteur, par exemple `"TABLESAMPLE SYSTEM(0.1) LIMIT N"`, `"LIMIT N (fallback after empty TABLESAMPLE)"` ou `"SELECT TOP N"` |
| `sampled_with_bias` | bool | vrai si l'échantillon n'est pas uniforme, par exemple en cas de repli sur LIMIT uniquement |
| `bias_reason` | string | vide si `sampled_with_bias = false`, sinon balise telle que `"unordered_limit_after_empty_TABLESAMPLE"` |
| `ratio_zstd_3` | float | arrondi au **0,05** le plus proche, zstd niveau 3 (valeur de production par défaut). Mesuré sur des octets encodés selon `sample_encoding`. |
| `ratio_zstd_19` | float | ratio hérité zstd niveau 19 accepté des captures plus anciennes ; l'outil ne le mesure plus et ne l'émet plus |
| `ratio_stddev` | float | arrondi au **0,05** le plus proche, écart-type des ratios de niveau 3 sur des tronçons de 64 KiB alignés sur les lignes de l’échantillon. Les blocs de projection au niveau des colonnes émettent actuellement `0.0`, car il s'agit d'indications consultatives sur l'entropie et non d'un modèle de variance. |
| `sample_encoding` | string | identifiant de l'encodage au niveau des octets auquel zstd a été appliqué. Valeur actuelle : `"dbwarp-blueprint-rowframe-v1"`. L'estimateur dbwarp DOIT valider cette chaîne avant d'utiliser le ratio : des encodages différents produisent des ratios différents pour les mêmes données logiques et ne sont PAS interchangeables. Les anciens fichiers Blueprint peuvent ne pas contenir ce champ ; les estimateurs ne doivent utiliser les ratios mesurés que lorsque la balise d'encodage est présente et reconnue. |

L'estimateur dbwarp doit privilégier les blocs de compression par colonne reconnus lors de la création de fixtures synthétiques, puis se rabattre sur la compression au niveau de la table et enfin sur les valeurs par défaut du type/style.

### Encodage au niveau des octets `dbwarp-blueprint-rowframe-v1`

L'échantillonneur Tier 2 concatène les lignes ou les valeurs de colonnes échantillonnées dans un tampon en mémoire au format suivant, puis applique zstd au niveau 3. Le tampon est supprimé ; seuls les ratios arrondis obtenus sont émis dans le fichier Blueprint.

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

Les balises de type font partie du contrat d'encodage et ne seront pas renumérotées sans incrément du suffixe `-v2`.

| Balise | Nom | Utilisé pour |
|---|---|---|
| 0x00 | Null | SQL NULL (aucune longueur, aucune charge utile) |
| 0x01 | TextUtf8 | Texte UTF-8 |
| 0x02 | TextUtf16Le | Octets UTF-16LE, principalement SQL Server `nvarchar`/`nchar`/`ntext` |
| 0x03 | TextOther | Octets dans un autre jeu de caractères |
| 0x04 | NumberText | Représentation textuelle décimale des valeurs numériques |
| 0x05 | BoolText | Booléen sous forme de texte |
| 0x06 | TimestampText | Horodatage au format texte ISO-8601 |
| 0x07 | DateText | Date au format texte ISO-8601 |
| 0x08 | TimeText | Texte `HH:MM:SS[.fff]` |
| 0x09 | UuidText | UUID canonique de 36 caractères sous forme de texte |
| 0x0F | JsonText | JSON UTF-8 |
| 0x10 | BinaryRaw | Octets `bytea`, `varbinary`, `image` ou blob |
| 0xFE | UnknownText | Représentation textuelle de repli fournie par la base de données |

### Limites de précision

`ratio_zstd_3` décrit le `sample_encoding` nommé ; il ne mesure pas les octets du protocole de base de données ni ceux d'un transfert de migration. La suite automatisée publique valide l'encodage déterministe, l'échantillonnage borné et la sérialisation, mais ne revendique pas une erreur en pourcentage universelle pour tous les moteurs et chemins d'extraction.

Avant d'utiliser ce ratio pour une décision de capacité importante, qualifiez le binaire et la version du moteur actuels avec des données source représentatives et le mécanisme d'extraction prévu. Enregistrez avec le plan obtenu la méthode de comparaison, la taille de l'échantillon, le hachage du binaire, la version du moteur et l'erreur observée. La relation primitive est `compressed_bytes ≈ sample_bytes / ratio_zstd_3` selon la distribution d'octets produite par le sample_encoding enregistré.

## `[fk_edges]`

Facultatif. Table en ligne dont chaque clé est un identifiant `table-NNN`
associé à une liste d'arêtes. Le schéma v3 conserve les ordinaux parents, les
actions référentielles, le mode de correspondance, le caractère différable,
l'état de validation/confiance et une synthèse relationnelle facultative
respectueuse de la confidentialité. Les arêtes sont triées par destination,
puis par liste de colonnes.

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

Le bloc facultatif `statistics` enregistre les valeurs échantillonnées ou
inférées de `non_null_rows`, `distinct_parent_values`,
`parent_coverage_fraction`, fanout p50/p95/p99/max et `orphan_rows`, ainsi que
les champs de provenance et de biais. Les contraintes source validées impliquent
l'absence d'orphelins. Les estimations composites dérivées d'échantillons par
colonne sont explicitement marquées comme inférées. Les générateurs utilisent
ces agrégats pour reproduire la couverture NULL et le fanout, en associant
chaque clé enfant composite à un tuple parent synthétique cohérent.

## `[artifact_inventory]` (depuis le schéma v4, sources de bases de données)

Le contrat indépendant et versionné `dbwarp-blueprint-artifacts/v1` décrit les
objets hors tables sans sérialiser les noms source ni les définitions. Il est
absent pour les fichiers structurés et lorsque `--artifact-detail none` est choisi.

Par défaut, `--artifact-detail summary` émet `object_count`,
`external_prerequisite_count`, `counts_by_kind` et
`counts_by_external_class`. `graph` ajoute un enregistrement d'objet anonyme
par artefact et les arêtes de dépendance. `analyzed` ajoute des enregistrements
bornés `dbwarp-language-feature-census/v1`, dérivés transitoirement des
définitions disponibles. `graph` et `analyzed` exigent explicitement `--yes`,
car la topologie du graphe peut identifier une application.

Les preuves au niveau de l'inventaire comprennent :

| Champ | Valeurs / règle |
|---|---|
| `detail` | `none`, `summary`, `graph` ou `analyzed` |
| `visibility` | `full`, `privilege_filtered` ou `unknown` |
| `inventory_complete` | Ne peut être vrai qu'avec une visibilité complète, aucun catalogue illisible et aucune famille non modélisée déclarée |
| `dependencies_complete` | Ne peut être vrai que si les catalogues de dépendances modélisés étaient lisibles |
| `analysis_complete` | Ne peut être vrai qu'au niveau analyzed et seulement si chaque analyse émise est complète |
| `catalogs_read` | Libellés fermés et standard des catalogues moteur inspectés avec succès |
| `catalogs_unreadable` | Libellés des catalogues en échec ; toute entrée interdit une affirmation de complétude |
| `families_not_inventoried` | Familles d'objets connues hors du contrat actuel du collecteur |

Les identifiants d'objet ont la forme `<kind>-NNN`, par exemple `view-001` ou
`function-002`. L'enregistrement ne contient que des jetons fermés de kind,
subkind et tier, des identifiants anonymes de schéma/parent, des dépendances
anonymes, un nombre de dépendances non résolues, une visibilité des définitions
et un mode de sécurité bornés, un prérequis externe facultatif et un recensement
du langage facultatif. Noms d'objets source, texte SQL, principaux, points de
terminaison, informations d'identification, clés, certificats et binaires ne
sont pas des champs du contrat.

Les prérequis externes enregistrent une `class` fermée, la portée du déploiement,
le besoin d'éléments binaires/secrets/points de terminaison non capturés et une
catégorie de compatibilité bornée. Leur nombre est une preuve de planification
de migration, pas une affirmation que DBWarp peut les provisionner ou les
traduire automatiquement.

Les recensements du langage utilisent `analyzer_version = "lexical-v1"` et
`status = "partial"`. Les valeurs de nombre, taille, imbrication, complexité et
régions opaques sont des bandes, non des empreintes source exactes. Les
caractéristiques proviennent d'un vocabulaire fermé. L'analyseur retire
commentaires, littéraux et identifiants délimités ; ce n'est ni un analyseur
syntaxique, ni un lieur sémantique, ni une garantie de traduction réussie.

Consultez l'[Inventaire des artefacts hors tables](ARTIFACT_INVENTORY.md) pour
les instructions opérationnelles et la couverture des moteurs.

## Défenses contre la stéganographie, par vecteur

| Vecteur | Défense |
|---|---|
| Ordre des identifiants | HMAC-SHA256 avec séparation de domaines et clé secrète locale au processus empêche la vérification hors ligne de noms candidats. Ne réutilisez une clé conservée par le client que si des libellés stables entre les exécutions sont nécessaires. |
| Bits de poids faible des nombres | Les statistiques sont arrondies par défaut selon la précision documentée. Le mode de longueurs exactes est explicite, soumis au consentement, enregistré dans le journal d'audit et doit être traité comme une métadonnée plus sensible. |
| Horodatage inférieur à la seconde | Un seul horodatage UTC au début, à la seconde uniquement |
| Formatage TOML | Canonique : clés alphabétiques, indentation fixe, aucun commentaire inséré |
| Aléa de l'échantillonnage | L’échantillonnage utilise des graines fixes (`TABLESAMPLE SYSTEM` déterministe de PG). Indépendamment, l’anonymisation des identifiants obtient volontairement une clé secrète auprès du CSPRNG du système d’exploitation, sauf si le client en fournit une. |
| Champs inutilisés | Chaque champ est documenté ci-dessus ; aucun champ « metadata »/« comment »/« reserved » susceptible de transporter des données de taille illimitée |
| Texte source des artefacts et éléments externes | Les définitions sont transitoires et effacées après l'analyse bornée ; noms, texte SQL, points de terminaison, chaînes de fournisseur, informations d'identification, clés, certificats, noms de paquets et binaires n'ont aucun champ sérialisé |

## Compatibilité des versions de schéma

Les producteurs actuels émettent la version 6 du schéma. Les versions 1 à 5
restent lisibles pour la rétrocompatibilité. Un fichier v1/v2 ne contient aucun
bloc de distribution ; les générateurs utilisent donc des valeurs de repli
déterministes pour le type, la largeur et les relations uniformes, et signalent
cette perte de fidélité. Un fichier v3 contient les métadonnées de distribution,
mais aucun inventaire d'artefacts. Un fichier v4 peut contenir un inventaire
d'artefacts, mais il est antérieur aux identifiants actuels du contrat Blueprint.
Les lecteurs normalisent les anciens identifiants v4 en entrée et réémettent le
document avec les identifiants Blueprint canoniques. Un fichier v5 est antérieur
aux qualifications de topologie et de portée du jeu de données ajoutées en v6. Les consommateurs
doivent refuser toute version future inconnue avec un message clair demandant
une mise à niveau, plutôt que d'ignorer silencieusement des champs.

## Pourquoi TOML plutôt que JSON

- TOML sépare plus lisiblement les sections structurelles des données terminales (`[tables.table-001.cols.col-2]` au lieu d'un JSON imbriqué).
- Les comparaisons sont plus simples (une clé par ligne ; les sous-tables identifiées restent contiguës).
- Le client peut effectuer des modifications manuelles s'il souhaite masquer un champ précis avant le partage.

JSON est utilisé comme **format intermédiaire** dans le chemin de repli SQL (`sql/blueprint.pg.sql` produit du JSON ; `blueprint_format.py` le normalise en TOML). Le fichier final partagé avec dbwarp est toujours au format TOML.

## Extensions de provenance des fichiers structurés

La version 3 du schéma et les versions ultérieures peuvent émettre les champs bornés suivants.

Les Blueprints de fichiers structurés utilisent les mêmes identifiants anonymisés
que les Blueprints de bases de données : `table-NNN` dans l'ordre déterministe des
entrées et `col-N` dans l'ordre ordinal du schéma. Les noms de fichiers, les
chemins Parquet, les noms de champs Avro et la valeur `logical_table` du
manifeste ne deviennent jamais des identifiants de table ou de colonne.

Lorsque `engine` ou `source_kind` vaut `"parquet"` ou `"avro"`, `table_bytes`
est l'estimation logique utilisée pour dimensionner le transfert, tandis que
`storage_bytes` est la taille réelle de l'objet source. Sans échantillonnage
décodé, Parquet utilise les octets non compressés des segments de colonne pour
`table_bytes` ; l'échantillonnage décodé facultatif les remplace par une
projection des octets `dbwarp-blueprint-rowframe-v1`. Avro dérive la valeur de son
parcours intégral décodé. `source_partitions`, `row_group_count` et
`source_codec` décrivent l'organisation et la provenance de planification. Les
jeux multifichiers agrègent ces valeurs. `row_group_count` est propre à Parquet
et `source_partitions` vaut `1` pour un objet d'entrée unique.

Au niveau colonne, `null_fraction` est une observation comprise entre `0.0` et
`1.0`. `length_sample_rows` et `length_sample_method` indiquent l'origine de
`len_avg` et `len_p95`. `source_semantics` conserve des faits bornés comme
`"repeated-leaf"`, `"nested-json"` ou `"multi-type-union"`. La précision
décimale, la précision et la sémantique UTC/locale des horodatages, les UUID et
la taille binaire fixe utilisent les champs scalaires existants et
`native_type`.

Au niveau table, `ratio_storage` compare `table_bytes` aux octets réels de
l'objet source. Au niveau d'une colonne Parquet, il compare les octets non
compressés et compressés du segment de colonne dans le footer. Ce sont des
signaux de planification de fichier, pas des estimations de transfert DBWarp.
`ratio_zstd_3` et `ratio_zstd_19` ne constituent des entrées valides pour
l’étalonnage du transfert que lorsque
`sample_encoding` vaut `"dbwarp-blueprint-rowframe-v1"`. Les ratios de footer
Parquet ou de conteneur Avro ne doivent jamais être copiés dans ces champs zstd.
