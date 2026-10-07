# Format de fichier DBWarp Blueprint, version 7.

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../FORMAT.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../FORMAT.md) | [Deutsch](../de/FORMAT.md) | **Français** | [Español](../es/FORMAT.md) | [Polski](../pl/FORMAT.md) | [日本語](../ja/FORMAT.md) | [简体中文](../zh/FORMAT.md)

Lisible par un humain. Facile à comparer. Vérifiable à des fins forensiques.

> **Ce format réduit le risque de canal caché et de divulgation directe grâce à
> un schéma borné, des identifiants fondés sur une clé secrète et une précision numérique
> documentée. La structure anonyme du graphe et les champs exacts facultatifs
> peuvent encore caractériser une charge de travail ; vérifiez donc le fichier
> selon votre propre politique de classification des données.**

## En-tête du fichier

À l'identique, octet pour octet :

```
# dbwarp-blueprint v7
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

La ligne vide fait partie de l'en-tête canonique. Le collecteur Rust émet exactement cet en-tête et aucun autre commentaire. Le normaliseur de repli SQL le conserve à l'identique, puis ajoute un commentaire fixe `Producer: blueprint_format.py SQL fallback` indiquant la source de la clé afin que les destinataires puissent distinguer le producteur. Ce n'est pas une affirmation selon laquelle les autres champs structurés ne peuvent pas identifier un schéma ou un graphe de dépendances distinctif.

## Champs de premier niveau

| Champ | Type | Description |
|---|---|---|
| `schema_version` | int | Version du format. Actuellement `7`. Les versions 1 à 6 restent lisibles. |
| `generated_at` | Chaîne ISO-8601. | Horodatage UTC, résolution en secondes, sans fraction. **Fixable** via l'option `--generated-at "2026-04-26T00:00:00Z"` de l'interface en ligne de commande. Les captures en direct identiques au niveau des octets nécessitent également la même protection `--anonymization-key-file`, l'état de la source, les options et la version du collecteur. Le journal d'audit enregistre `generated_at_pin: ...` chaque fois que l'option est définie, de sorte que la fixation est visible pour l'analyse forensique. Aucune variable d'environnement ne fixe cette valeur. |
| `engine` | chaîne de caractères | `"postgresql"`, `"mysql"`, `"sqlserver"`, `"oracle"`, `"parquet"`, ou `"avro"`. `oracle` n'apparaît que dans les résultats de l'aperçu Oracle. |
| `engine_version` | chaîne de caractères | Version numérique du produit de la base de données source ; vide pour les sources de fichiers structurés. Les bannières de distribution sont exclues. |
| `source_kind` | chaîne de caractères | Les sources de données utilisent les valeurs `"production"`, `"staging"`, `"scrubbed-replica"` ou `"synthetic"` déclarées par l'opérateur. Les sources structurées utilisent `"parquet"` ou `"avro"`. |
| `length_metadata` | chaîne de caractères | Résumé : Marqueur conservé pour les lecteurs précédents : `"hybrid-v2"`, `"exact"`, `"rounded"` ou `"not-captured"`. Les trois champs ci-dessous sont les plus fiables. |
| `declared_length_fidelity` | string | `"exact"` pour les capacités déclarées en caractères de PostgreSQL et pour les modes MySQL équilibré par défaut/exact ; `"coarse-rounded-v1"` pour la confidentialité MySQL stricte ; `"not-captured"` lorsque l'information n'est pas disponible. |
| `index_length_fidelity` | string | `"exact"` pour les préfixes d'index MySQL équilibrés par défaut ou exacts ; `"rounded-down-v1"` pour la confidentialité stricte ; `"not-captured"` lorsque l'information n'est pas disponible. |
| `observed_length_fidelity` | string | `"relative-rounded-v2"` par défaut lorsque l'échantillonnage a eu lieu, `"exact"` en mode exact, `"coarse-rounded-v1"` en mode strict ou `"not-sampled"`. La couverture de l'échantillonnage reste une exigence distincte pour chaque colonne. |
| `[totals]` | inline table | Nombres agrégés (voir ci-dessous). |
| `[network]` | table | Preuve facultative de connexion client-base et de RTT de requête. |
| `[database_topology]` | table | Nécessaire pour les sources de bases de données utilisant le schéma v6 et versions ultérieures. Le schéma v7 utilise le contrat de topologie v2 et enregistre la portée de chaque nombre de membres. Absent pour les fichiers structurés. |
| `[dataset_scope]` | table | Nécessaire pour chaque Blueprint schema-v6 et versions ultérieures. Indique ce que les totaux couvrent et si la couverture des tables, des lignes et des octets est complète. |
| `[structure_scope]` | table | Nécessaire dans la version 7. Qualifie séparément l'exhaustivité de l'inventaire des tables, des colonnes, des index et des relations. |
| `[source_environment]` | table | Requis dans les Blueprints de base de données v7 et interdit pour les fichiers structurés. Contient uniquement des preuves capacity/hosting approximatives observées via le point d'accès de la base de données ou un fournisseur explicite. |
| `[statistics_evidence]` | table | Nécessaire dans la version 7. Agrégation exacte des classifications "nombre de lignes par table", "statistiques de l'optimiseur" et "preuves de taille". |
| `[activity_snapshot]` | table | Non rédigé par DBWarp Blueprint 1.6. |
| `[tables.X]` | tables | Une entrée par table, avec identifiant anonymisé. |
| `[fk_edges]` | inline table | Graphe des clés étrangères entre tables anonymisées. Facultatif. |
| `[artifact_inventory]` | table | Nécessaire dans la version 7, de sorte que les termes « non demandé », « non applicable », « illisible » et un inventaire vérifié de zéro objet restent distincts. Contient des décomptes d'objets limités et sans nom, des relations anonymes typées facultatives, des exigences et un recensement limité de la langue. |

## `[totals]`

| Champ | Type | Précision |
|---|---|---|
| `table_count` | int | exacte |
| `row_count` | int | somme des données sérialisées par table `rows` ; les estimations du catalogue sont arrondies, tandis qu'une lecture complète et vérifiée est exacte. |
| `table_bytes` | int | somme des valeurs `table_bytes` arrondies par table |
| `index_bytes` | int | somme des valeurs `index_bytes` arrondies par table |

Ces chiffres ne représentent pas automatiquement les totaux de l'ensemble du cluster. Interprétez-les toujours en conjonction avec `[dataset_scope]`. Une passerelle ou un coordinateur partitionné peut exposer un catalogue qui semble complet, tout en ne contenant aucune des partitions sous-jacentes ; les versions 6 et 7 du schéma expriment explicitement cette incertitude, au lieu de traiter silencieusement les statistiques locales du catalogue comme une vérité globale.

`row_count` est une somme arithmétique des valeurs sérialisées par table, et non une deuxième mesure non arrondie. Un nombre positif connu inférieur au premier seuil de confidentialité est représenté par `100` avec `row_count_quality = "engine-estimate"` par table ; par conséquent, un ensemble contenant de nombreuses petites tables peut avoir un total élevé de manière conservatrice. Dans ce cas, `dataset_scope.limitations` contient également `row-counts-statistical`. Zéro reste réservé pour indiquer que la table source est vide.

## `[database_topology]` (sources de bases de données)

Ce bloc conserve uniquement des faits bornés visibles par le point de
connexion à la base. Il ne stocke jamais de noms de nœuds ou d'hôtes,
d'adresses IP, de noms de cluster ou de canal de réplication, d'identifiants de
serveur ni de points de terminaison.

| Champ | Valeurs / règle |
|---|---|
| `contract` | `dbwarp-blueprint-topology/v1` dans le schéma v6 ; `dbwarp-blueprint-topology/v2` dans v7. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed` ou `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member`, `physical-standby`, `logical-standby`, `snapshot-standby`, ou `unknown`. |
| `visibility` | `full`, `partial` ou `unknown` ; décrit les preuves de topologie, pas la justesse des données. |
| `member_count` | Nombre de membres visibles par des requêtes de preuve réussies. `0` signifie inconnu, jamais zéro membre. |
| `member_count_scope` | V7 uniquement : `deployment`, `visible-subset`, `connected-member` ou `unknown`. La visibilité complète de la topologie nécessite `deployment` ; `connected-member` nécessite un nombre égal à un. |
| `identifiers_redacted` | Doit valoir `true`. |
| `role_counts` | Comptages facultatifs par jeton de rôle fermé. Une visibilité complète exige que leur somme égale `member_count`. |
| `features` | Des jetons fermés triés tels que `citus`, les formulaires MySQL replication/cluster, `postgresql-streaming-replication`, `sqlserver-availability-group`, `oracle-non-cdb`, `oracle-cdb`, `oracle-pdb`, `oracle-rac`, `oracle-data-guard` ou `vitess`. |
| `catalogs_read` | Libellés fermés triés des catalogues de topologie lus avec succès. |
| `catalogs_unreadable` | Libellés fermés triés des catalogues de topologie illisibles. Toute entrée empêche d'affirmer une visibilité complète. |
| `catalogs_not_applicable` | Version 7 uniquement. Les étiquettes fermées triées se sont avérées inapplicables à cette source. Elle est distincte des ensembles lisibles et illisibles. |

Un point de connexion ordinaire peut légitimement signaler
`deployment = "unknown"` tout en fournissant des statistiques locales
complètes d'une copie intégrale. Blueprint ne déduit pas qu'un serveur sans
caractéristique particulière est single-node simplement parce qu'aucune
fonction de cluster n'était visible.

## `[dataset_scope]` (schéma v6 et versions ultérieures)

Ce bloc qualifie chaque total de dimensionnement indépendamment. Ne considérez pas les totaux comme des chiffres représentant l'ensemble des données lorsque toute dimension de complétude requise est `incomplete` ou `unknown`.

| Champ | Valeurs / règle |
|---|---|
| `contract` | Toujours `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset` ou `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete` ou `unknown`. |
| `row_count_completeness` | `complete`, `incomplete` ou `unknown`. |
| `size_completeness` | `complete`, `incomplete` ou `unknown`. |
| `row_count_method` | Un jeton de provenance fermé tel que `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter`, `oracle-table-statistics` ou `oracle-segment-statistics`. `bounded-complete-read` et `mixed-catalog-and-bounded-read` identifient les totaux récupérés à partir d'une lecture de niveau 2 complète et vérifiée, seuls ou en combinaison avec les décomptes du catalogue. Oracle utilise `not-applicable` conjointement avec la même méthode de taille uniquement lorsque l'inventaire n'est pas vide et qu'il ne contient pas de tables dans la population des totaux de copie. `distributed-aggregate` est accepté en entrée, mais n'est pas écrit par cette version. |
| `size_method` | Un jeton de provenance fermé tel que `postgres-local-relation-size`, `citus-distributed-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `oracle-segment-bytes`, `oracle-table-logical-estimate`, `mixed` ou `not-applicable`. Oracle utilise `mixed` lorsque les tables incluses combinent des compteurs de segments attribués avec des estimations logiques étiquetées. Il utilise `not-applicable` uniquement lorsque l'inventaire n'est pas vide et qu'il n'y a pas de tables dans la population totale copiée, afin qu'un total nul complet ne prétende pas faussement utiliser une méthode de mesure. `distributed-aggregate` est accepté en entrée, mais n'est pas écrit par cette version. |
| `limitations` | Raisons fermées et triées d'une couverture incomplète ou inconnue. Au moins une est obligatoire sauf si toutes les dimensions sont complètes. |

`selection-limited` signifie que les totaux et les déclarations de complétude couvrent exactement les schémas demandés au moyen du sélecteur actif répétable `--schema` ; ils ne prétendent pas couvrir toute la base de données connectée. Sans `--schema`, la capture de tous les schémas visibles est conservée.

Un schéma sélectionné et lisible peut légitimement ne contenir que des objets non tabulaires et est conservé dans les inventaires applicables. Cependant, lorsque la capture complète ne contient aucune table, le collecteur ne doit pas publier un ensemble de données vide définitif : l'exhaustivité des tables, des lignes et de la taille reste incomplète, et `table-inventory-visibility-unknown` enregistre la limite conservatrice.

`row-count-evidence-incomplete` et `size-evidence-incomplete` indiquent qu'au moins une table incluse ne contenait pas la valeur de catalogue correspondante. Le total numérique est alors la somme des contributions connues, et non une affirmation qu'une table indisponible contenait zéro ligne ou octet. Les statistiques par table indiquent quelles données sont indisponibles.

Pour Oracle, `oracle-segment-bytes` est la preuve exacte préférée de la taille allouée. Si l'espace de stockage pour une table ne peut pas être attribué à partir de `DBA_SEGMENTS`—par exemple, une table clusterisée ou une table organisée par index dont le catalogue d'index n'est pas disponible—le collecteur peut émettre `oracle-table-logical-estimate` en utilisant les valeurs `DBA_TABLES.NUM_ROWS * AVG_ROW_LEN` déjà disponibles. La preuve de la table contient alors `size_quality = "engine-estimate"`, `size_scope = "table-only"`, et `size_accounting = "logical-estimate"`, `size_visibility = "partial"`, et la complétude de la taille de l'ensemble de données est `incomplete` ; elle ne prétend pas connaître la taille des LOB ou des index. Un catalogue de raffinement manquant ne fait jamais disparaître les octets déjà attribués à cette table logique : la contribution mesurée reste `oracle-segment-bytes`, avec une visibilité partielle et une couverture agrégée incomplète. Le stockage partagé ou organisé par index qui ne peut pas être attribué n'est pas publié comme un zéro exact. Une table Oracle avec un index de domaine utilise également une visibilité partielle et une portée inconnue car les implémentations de domaine Text, Spatial et autres peuvent stocker des octets dans des objets secondaires en dehors de l'inventaire de la table utilisateur émise. La solution de repli logique est une preuve de dimensionnement par copie plutôt qu'un compteur d'octets alloués. Elle peut surestimer l'allocation actuelle lorsque les estimations de lignes de l'optimiseur restent obsolètes après la libération de l'espace de stockage (par exemple, après `TRUNCATE ... DROP STORAGE`), et son origine d'estimation doit être conservée. Aucune autorisation `DBA_TABLESPACES` n'est requise pour ce repli.

Pour le stockage des index Oracle, la lecture complète du catalogue des index constitue la limite logique. Une ligne `DBA_SEGMENTS` ultérieure sans identité d'index correspondante se trouve en dehors de cet ensemble et n'est pas attribuée à une table arbitraire. Un index qui était présent à la limite mais qui ne possède pas sa contribution de segment attendue ne permet pas de visualiser la taille complète de sa table.

`logical-partition-root-unmeasured` est une preuve spécifique à PostgreSQL qu'une partition logique racine incluse ne contribue délibérément ni lignes ni octets, car ces valeurs se trouvent sur ses feuilles physiques. Contrairement à une lacune remédiable `row-count-evidence-incomplete` dans les preuves statistiques, une lecture complète d'une autre table ne peut pas restaurer l'intégrité du jeu de données tant que cette racine reste dans l'inventaire sélectionné.

`table-inventory-visibility-unknown` indique que le collecteur n'a pas pu lire la classification propre au moteur, nécessaire pour séparer les objets utilisateur des objets de support. Des enregistrements visibles peuvent toujours être présents, mais l'intégrité des tables, des lignes et de la taille est compromise, plutôt que de considérer ce sous-ensemble comme l'ensemble complet.

`catalog-capture-truncated` signifie qu'une session de catalogue unique s'est arrêtée avant que toutes les familles ou tous les schémas prévus ou sélectionnés n'aient été lus. Les enregistrements dont l'intégrité a déjà été prouvée peuvent toujours être émis, mais aucune affirmation concernant l'intégrité d'un ensemble de données ou d'une structure ne peut s'étendre au reste non lu.

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

## `[structure_scope]` (schéma v7)

Ce bloc permet de distinguer un catalogue vide et vérifié d'un catalogue qui a été filtré, rendu illisible ou qui n'a pas été inspecté.

| Champ. | Valeurs / règle. |
|---|---|
| `contract` | Toujours `dbwarp-blueprint-structure-scope/v1`. |
| `visibility` | `full`, `privilege-filtered` ou `unknown`. La complétude est limitée aux schémas sélectionnés et à la portée des privilèges visibles ; elle ne constitue pas une affirmation de visibilité illimitée de la base de données. |
| `table_inventory_completeness`, `column_inventory_completeness`, `index_inventory_completeness`, `relationship_inventory_completeness` | Indépendamment de `complete`, `incomplete` ou `unknown`. Les familles dépendantes ne peuvent pas indiquer que leur statut est "complet" si leur famille parente requise est incomplète. |
| `catalogs_read`, `catalogs_unreadable`, `catalogs_not_applicable` | Étiquettes de catalogues fermées et disjointes, triées. `catalogs_read` indique une preuve de lecture positive ; dans le cas d'une capture multi-propriétaires, elle peut conserver un catalogue lorsqu'au moins un propriétaire prévu a été lu avec succès, même si la lecture a échoué pour un autre propriétaire. `catalogs_unreadable` signifie qu'aucune lecture positive n'a survécu. Une famille complète nécessite son catalogue spécifique à l'environnement `catalogs_read`, chaque requête de famille prévue doit être terminée, et il ne doit y avoir aucun écart affectant les objets individuels. |
| `limitations` | Les raisons de fermeture, telles que `selection-limited`, `metadata-visibility-privilege-filtered` ou `table-kinds-not-inventoried`, doivent être indiquées. Lorsqu'il existe des preuves partielles ou inconnues, une raison doit être fournie. |

Les sélecteurs de schéma font partie de la portée : `complete` signifie complet pour les schémas sélectionnés et résolus, et non nécessairement tous les schémas du service. Un sélecteur qui ne résout aucun schéma est une erreur et ne doit pas devenir un Blueprint vide et complet.

`catalog-capture-truncated` a la même signification dans le contexte de la vérification de la structure : les enregistrements de tables et de colonnes publiés constituent le préfixe ou le sous-ensemble de propriétaire vérifié, et non une affirmation que le reste du travail prévu pour le catalogue a été achevé. Un catalogue qui n'a pas été traité par cette étape n'apparaît dans aucun des trois ensembles de catalogues ; il ne doit pas être étiqueté comme illisible ou non applicable.

Pour une lecture multi-propriétaires, `index-inventory-unavailable` ou `relationship-inventory-unavailable` peuvent donc accompagner un catalogue dans `catalogs_read` : l'étiquette du catalogue conserve la preuve positive du propriétaire ayant réussi, tandis que le champ de complétude et l'enregistrement de limitation indiquent que l'ensemble de la population sélectionnée n'a pas été observé. Les limitations par table identifient les objets émis avec un écart de représentativité ; elles ne remplacent pas la preuve de l'état de la requête pour un propriétaire qui a été refusé ou pour lequel la tentative a échoué et qui n'a émis aucune table.

Les enregistrements Oracle `oracle-identity-columns` et `oracle-constraint-columns` sont stockés séparément de leurs catalogues de colonnes et de contraintes parents. Leur présence ou leur absence décrit la génération d'identité et les preuves de clé de relation facultatives ; cela ne doit pas être réduit à une affirmation selon laquelle le catalogue parent n'était pas accessible.

## `[source_environment]` (sources de bases de données du schéma v7)

Ce bloc ne décrit jamais la station de travail exécutant `dbwarp-blueprint`. `collector_machine_excluded` doit être `true`. Les preuves de capacité proviennent uniquement du point de terminaison de la base de données connecté ou d'un fournisseur, orchestrateur ou opérateur autorisé.

| Champ. | Valeurs / règle. |
|---|---|
| `contract` | Toujours `dbwarp-blueprint-source-environment/v1`. |
| `evidence_origin` | `database-endpoint`, `provider-api`, `orchestrator-api`, `operator-attested`, `mixed`, ou `none`. |
| `hosting_model` | `managed-service`, `self-managed`, `orchestrated` ou `unknown`. |
| `infrastructure_location` | `cloud`, `on-premises`, `hybrid` ou `unknown`. |
| `capacity_scope` | `connected-instance`, `database-resource`, `cluster-aggregate`, `member-subset`, ou `unknown`. |
| `capacity_visibility` | `capacity_visibility` peut être `full`, `partial`, `unknown` ou `not-requested`. `not-requested` nécessite des plages, des bases et une portée inconnus, et aucun catalogue de capacité classifié. Une classification autre que celle de la capacité, comme l'édition SQL Server, peut toujours être présente. |
| `cpu_capacity_band` | `1`, `2`, `3-4`, `5-8`, `9-16`, `17-32`, `33-64`, `65-128`, `129-plus`, ou `unknown`. |
| `cpu_capacity_basis` | `logical-cpu-limit`, `database-resource-limit`, `operating-system-visible`, `physical-host`, ou `unknown`. `operating-system-visible` n'affirme pas qu'une allocation de machine virtuelle, de conteneur ou de service géré correspond à l'hôte physique sous-jacent. |
| `memory_capacity_band` | Des plages larges allant de `under-2-gib` à `512-gib-plus`, ou `unknown`. |
| `memory_capacity_basis` | `database-buffer-cache`, `database-resource-limit`, `operating-system-visible`, `physical-host`, ou `unknown`. `operating-system-visible` est la base conservatrice pour un moteur DMV dont la valeur peut décrire un conteneur ou une instance plutôt que du matériel physique. Une bande `database-buffer-cache` est l'allocation de cache configurée et représente donc uniquement une limite inférieure de la mémoire totale de la source ; elle ne doit jamais être interprétée comme la capacité de l'hôte sans sa base. |
| `member_capacity_uniform` | Optionnel observed/attested, de type booléen ; l'absence de valeur indique une valeur inconnue. |
| `features` | Des faits fermés et triés tels que `autoscaling`, `burstable`, `container-limits-visible`, `database-resource-governed`, `serverless` ou `shared-host`. |
| `limitations` | Limitations de traçabilité fermées et triées. `oracle-client-version-mismatch` ou `oracle-client-version-unreadable` indique qu'une version cliente Oracle SQL*Plus n'a pas pu être entièrement attestée. `oracle-client-version-below-tested-floor` indique un client attesté antérieur à 12.1, le seuil de comparaison encodé par ce contrat. La capture du catalogue se poursuit car la provenance de la bannière client ne détermine pas la structure de la base de données. |
| ensembles de catalogues. | Des preuves triées et distinctes concernant les catalogues exacts de l'environnement source tenté, y compris la classification de l'édition de SQL Server, même lorsque sa DMV de capacité facultative n'est pas lisible. |

Une capacité inconnue n'est pas une capacité nulle. Une connexion distante n'autorise pas la lecture du processeur ou de la mémoire de l'hôte du collecteur, ni leur réétiquetage en tant que capacité du serveur.

Pour Oracle, un catalogue de capacité qui a été complété pour seulement une partie de l'ensemble de requêtes prévu reste une preuve `catalogs_read` positive, mais ses valeurs sont retenues et `capacity_visibility` est `unknown`. Les lignes partielles ne doivent pas être présentées comme une limite CPU ou mémoire globale.

Les paramètres des fonctionnalités Oracle SQL*Plus sont sélectionnés à partir de la version numérique de la session en cours lorsqu'elle est lisible, sinon à partir de la bannière de l'exécutable, puis à partir d'un protocole conservateur qui ne sélectionne ni `ROWLIMIT` ni le balisage CSV comme fonctionnalité de sortie. Les deux passages de configuration tentent toujours d'effacer un `ROWLIMIT` hérité et le mode CSV ; le diagnostic d'option inconnue d'un client plus ancien n'est toléré que dans la fenêtre de réinitialisation délimitée. Une incompatibilité de version, une attestation partielle, une erreur d'analyse ou un client attesté antérieur à 12.1 affaiblissent uniquement la provenance. Cela ne bloque jamais la capture du catalogue.

## `[statistics_evidence]` et `[tables.<id>.statistics]` (schéma v7)

Chaque table v7 possède un bloc de statistiques. Le bloc de niveau supérieur contient des décomptes exacts par `statistics_state`, `row_count_quality` et `size_quality` ; chaque mappage doit couvrir toutes les tables et être exactement égal aux classifications au niveau de la table. La visibilité agrégée est `full` uniquement pour une population totale non vide lorsque chaque table comptée a une visibilité de taille complète, des preuves de lignes connues et un état de statistiques classifié, et qu'aucun catalogue de statistiques n'est illisible. Les objets externes, temporaires et dérivés, intentionnellement exclus, restent répertoriés ; leur indisponibilité de lignes et de taille, déterminée par une politique, ne réduit pas la visibilité de cette population, mais un état de statistiques non classifié le fait. Lorsque toutes les tables sont exclues, la visibilité agrégée est `unknown` avec `statistics-visibility-unknown` ; une population vide ne doit pas obtenir `full` de manière fallacieuse. `catalog-capture-truncated` enregistre que le travail prévu sur le catalogue de statistiques s'est arrêté avant que tous les propriétaires n'aient été atteints, tout en conservant toute preuve de lecture du catalogue positive qui a déjà été obtenue. La même règle de preuve positive s'applique lorsqu'un propriétaire réussit la lecture et qu'un autre est refusé : le catalogue reste dans `catalogs_read`, tandis que `statistics-partial` et la visibilité agrégée enregistrent que la population sélectionnée n'a pas été entièrement observée.

Les champs au niveau de la table sont :

| Champ. | Valeurs / règle. |
|---|---|
| `row_count_method` | Engine/version-aware méthode de catalogue, `bounded-complete-read` lorsqu'une instruction de niveau 2 a énuméré de manière sécurisée la table visible, un compteur de fichier structuré, ou `unknown` ; la capture ordinaire ne revient pas silencieusement à `COUNT(*)`. |
| `row_count_quality` | `exact-counter`, `exact-read`, `engine-counter`, `engine-estimate`, `cached-engine-estimate`, `sample-extrapolation`, `unavailable`, ou `unknown`. Un compteur SQL Server positif connu, inférieur au premier seuil de confidentialité non nul, utilise `engine-estimate` après la sérialisation de `rows = 100` ; cela distingue le seuil de confidentialité d'un compteur exact et d'un zéro mesuré. |
| `statistics_state` | `current`, `possibly-stale`, `known-stale`, `never-analyzed`, `locked`, `user-supplied`, `not-applicable`, ou `unknown`. |
| `refresh_age_band` | `under-1h`, `1h-1d`, `1-7d`, `1-4w`, `1-3m`, `3m-plus`, `unknown`, ou `not-applicable`. |
| `modification_ratio_band` | `none`, `under-1pct`, `1-5pct`, `5-10pct`, `10-20pct`, `20-50pct`, `over-50pct`, `unknown`, ou `not-applicable`. |
| `sample_fraction_band` | `full`, `75-99pct`, `50-74pct`, `25-49pct`, `under-25pct`, `unknown`, ou `not-applicable`. |
| `statistics_scope` | `global`, `partition`, `subpartition`, `session`, `local-member`, `logical-dataset`, `database-resource`, `structured-dataset`, `selected-object`, ou `unknown`. |
| `size_method`, `size_quality`, `size_scope`, `size_accounting`, `size_visibility` | Décrivez séparément l'origine de la taille, s'il s'agit d'un compteur ou d'une estimation, si elle inclut le stockage LOB/index, si elle est allouée ou logique, et si la visibilité est totale, partielle, indisponible ou inconnue. |

Le bloc de statistiques de niveau supérieur utilise `visibility = "full"`, `"partial"` ou `"unknown"` pour la population totale de copies non vides décrite ci-dessus. Il n'utilise jamais uniquement des objets exclus pour obtenir une visibilité `full`.

Les preuves Oracle `oracle-segment-bytes` ne sont valides qu'avec `exact-counter`, `allocated-segment`, une visibilité complète ou partielle, et un `segment_state` prouvant que le recensement du segment a été attribué (`created`, `deferred`, `mixed`, ou `mixed-table-and-index`). Une solution de repli logique utilise `oracle-table-logical-estimate`, `engine-estimate`, `logical-estimate`, une visibilité partielle, et un état de segment indisponible. Cela empêche une classe de stockage non attribuée de devenir un zéro mesuré. L'état est dérivé des preuves de catalogue attribuées avant l'arrondi de confidentialité. `created` peut donc accompagner des octets de table nuls sérialisés lorsqu'un compteur de table brut positif connu est inférieur au premier compartiment d'octets. Les preuves Oracle mesurées partielles utilisent `size_scope = "unknown"` : les octets attribués restent exacts, mais l'absence d'un LOB, d'un stockage imbriqué ou d'une correspondance d'index signifie que le collecteur ne peut pas honnêtement prétendre à la totalité de la portée table/LOB/index. Pour Oracle, `mixed` signifie qu'une allocation d'index positive attribuée a été supprimée et sérialisée en `index_bytes = 0` par arrondi ; cela permet de distinguer le zéro d'une table pour laquelle le recensement du segment n'a trouvé aucune allocation d'index. `mixed-table-and-index` signifie que les allocations de table et d'index étaient positives avant l'arrondi et que les deux compteurs sérialisés sont nuls, préservant ainsi les deux faits sans divulguer les valeurs d'octets de sous-compartiment. `deferred` signifie que les compteurs bruts attribués étaient nuls et que les deux valeurs d'octets sérialisées doivent être nulles. Utilisez l'état pour distinguer une allocation de sous-compartiment arrondie d'un stockage dont il est prouvé qu'il n'est pas matérialisé.

Le bloc de niveau supérieur enregistre également des catalogues disjoints triés et des limitations fermées. Un zéro `rows` ou `table_bytes` ne peut être utilisé comme zéro observé qu'avec des preuves quality/visibility correspondantes ; ne pas ignorer le bloc de provenance. Une table comptée dont la qualité des lignes ou de la taille est `unavailable` ou `unknown` éloigne la complétude correspondante de l'ensemble de données `complete` ; le validateur rejette un espace réservé numérique présenté comme une couverture complète. En particulier, une table PostgreSQL qui ne possède ni statistiques d'optimiseur ni une lecture complète et bornée prouvée a un volume de lignes inconnu, et non un zéro mesuré.

Oracle Basic omet `check_count` lorsque le dictionnaire ne peut pas distinguer une contrainte `NOT NULL` déclarée d'une contrainte `CHECK` explicite et textuellement identique. Il ne déduit pas à partir d'un nom de contrainte généré ou de la nullabilité actuelle de la colonne. D'autres tables dont les lignes de contraintes sont sans ambiguïté peuvent toujours contenir un nombre exact.

## `[activity_snapshot]`

DBWarp Blueprint 1.6 n'écrit pas ce bloc.

## `[network]` (facultatif)

Temps de trajet aller-retour depuis la machine exécutant le collecteur jusqu'à votre base de données. Ce n'est pas le temps de trajet aller-retour entre la source et la cible de la migration.

La sonde s'exécute après l'établissement de la connexion et avant les requêtes de catalogue, afin que les mesures ne soient pas faussées par le préchauffage du cache de requêtes. Elle exécute **5× `SELECT 1`** et émet la latence médiane. Chaque `SELECT 1` renvoie l'entier constant 1 : cette sonde ne lit jamais de données de ligne.

Absent lorsque `--no-rtt-probe` est utilisé ou lorsque la sonde elle-même échoue pendant son exécution (enregistré comme un avertissement non fatal vers stderr et le journal d'audit ; le fichier Blueprint est toujours généré sans ce bloc).

| Champ | Type | Précision |
|---|---|---|
| `sample_count` | int | exacte (toujours 5 dans v1) |
| `connect_total_ms` | int | temps total écoulé entre le début de la connexion TCP et la disponibilité de la session authentifiée, en millisecondes. Comprend la négociation TCP, la négociation TLS le cas échéant et le défi/réponse d'authentification. Arrondi à la milliseconde la plus proche. Généralement 3 à 6 fois `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | latence médiane d'un aller-retour parmi les 5 échantillons `SELECT 1`, en millisecondes. Arrondie à la milliseconde la plus proche. Le bruit réseau naturel (≥ 1 ms en pratique) est supérieur à la granularité d'arrondi ; celui-ci élimine donc tout canal caché dans les bits de poids faible sans perdre de précision utile. Les valeurs LAN inférieures à la milliseconde deviennent 0 ou 1. |
| `query_rtt_ms_p95` | int | 95e centile des 5 échantillons calculé selon la méthode du rang le plus proche (l'observation la plus lente), en millisecondes. Arrondi à la milliseconde la plus proche. Utilisez-le avec p50 pour repérer de brefs pics de latence ; cinq échantillons servent uniquement de repère et ne constituent pas un test de performance de charge de travail. |

Les cinq requêtes de sonde apparaissent dans le journal d'audit sous la forme d'**une seule entrée récapitulative** (et non de cinq lignes distinctes), intitulée `5x SELECT 1 (RTT probe; constant integer 1, no row data)`. Cela correspond au principe de confiance selon lequel aucun contenu de ligne n'est lu.

## `[tables.<id>]`

L'identifiant est `table-NNN`, où `NNN` est l'ordinal à un indice dans un ordre HMAC-SHA256 séparé par des domaines, basé sur le nom du schéma et de la table. La clé par défaut est générée à chaque exécution et n'est jamais affichée. L'utilisation de la même `--anonymization-key-file` protégée préserve l'ordre lors des comparaisons approuvées. Le schéma v7 nécessite l'ensemble complet et continu des ordinaux de `table-001` jusqu'au nombre de tables affichées (la largeur augmente naturellement à `table-1000`) ; les suffixes omis, nuls, non décimaux ou dérivés de la source sont invalides.

| Champ | Type | Précision / valeurs |
|---|---|---|
| `rows` | int | Les estimations du catalogue sont arrondies : au plus proche de 100 (≤10k), de 1000 (≤1M), de 10000 (>1M). Une estimation positive connue qui, autrement, serait arrondie à zéro utilise le premier intervalle non nul (`100`) ; zéro est réservé pour un catalogue nul ou une absence de données identifiée par la qualité statistique adjacente. Lorsqu'une lecture limitée de niveau 2 prouve qu'elle a énuméré la table visible complète, `rows` est le nombre exact déjà divulgué par l'échantillon exact `sample_rows` ; cela évite des décomptes contradictoires par table, de cardinalité et d'agrégats sans ajouter un nouveau canal. |
| `table_bytes` | int | arrondi : au 1KiB, 1MiB ou 100MiB le plus proche selon l'ordre de grandeur |
| `index_bytes` | int | arrondi : identique à `table_bytes` |
| `schema` | chaîne de caractères | Un identifiant anonymisé `schema-A`, `schema-B`, ..., `schema-AA`. Le schéma v7 nécessite un ensemble ordinal alphabétique dense pour chaque schéma référencé par une table émise ou un artefact graph/analyzed ; un schéma sélectionné contenant uniquement des objets non tabulaires est donc conservé. |
| `object_kind` | chaîne de caractères | La version V7 requiert un jeton de fin : `ordinary-table`, `materialized-view`, `external-table`, `temporary-table`, `nested-table` ou `object-table`. L'identité de l'objet est indépendante du stockage physique et du partitionnement. |
| `storage_organization` | chaîne de caractères | La version V7 requiert un jeton de fin : `heap`, `index-organized`, `clustered`, `external` ou `unknown`. `external` n'est valide que pour `object_kind = "external-table"`. |
| `partitioning` | chaîne de caractères | V7 a requis un jeton de fin : `none`, `range`, `list`, `hash`, `interval`, `reference`, `composite`, `system`, `key`, `linear-hash`, `linear-key` ou `unknown`. |
| `segment_state` | chaîne de caractères | La version V7 requiert un jeton de fin : `created`, `deferred`, `mixed`, `mixed-table-and-index`, `unavailable` ou `unknown`. Ceci permet de distinguer les objets contenant uniquement des métadonnées des objets contenant des données matérialisées. Il s'agit d'une preuve catégorique établie avant l'arrondi des octets, de sorte que `created` peut accompagner des octets de table sérialisés nuls pour une allocation de sous-répertoire positive. Avec la preuve du compteur de segments Oracle, `mixed` enregistre une allocation d'index positive attribuée dont le `index_bytes` sérialisé est arrondi à zéro ; `mixed-table-and-index` enregistre que les deux allocations brutes étaient positives, tandis que les deux compteurs sérialisés sont arrondis à zéro. |
| `parent_table`, `child_tables` | chaîne / tableau | Liens de tables anonymes réciproques facultatifs pour les objets imbriqués, partitionnés ou autrement contenus. Les identifiants des éléments enfants sont triés et uniques ; le graphe parent doit être acyclique. |
| `table_features` | tableau | Tokens fermés et triés : `graph-edge`, `graph-node`, `memory-optimized`, `temporal-current`, ou `temporal-history`. |
| `unlogged` | bool | Observation facultative de l'état enregistré de PostgreSQL. Omise si elle n'est pas capturée ; `false` explicite signifie que le catalogue a confirmé que la table est enregistrée. |
| `partition_count` | int | Le nombre exact de partitions physiques concernées doit être indiqué lorsque `partitioning` désigne une stratégie de partitionnement connue. PostgreSQL signale les partitions feuilles récursives et exclut les partitions qui ne se trouvent pas dans les schémas résolus, conformément à `selection-limited`. Les tables composites MySQL comptent les sous-partitions, car celles-ci sont leurs partitions physiques ; par exemple, quatre partitions de niveau supérieur avec huit sous-partitions chacune signalent `32`. La valeur zéro n'est valide que pour une partition racine logique avec `segment_state = "unavailable"` et aucune partition feuille concernée. |
| `partition_key_cols` | tableau d'entiers | Compléter les numéros d'ordre des colonnes de clé de partition simple. Omis pour une clé basée entièrement ou partiellement sur une expression, ou lorsque les informations du catalogue sont indisponibles ; une liste partielle de numéros d'ordre et les expressions de clé ne sont jamais sérialisés. |
| `partition_rows_max` | int | Estimation optionnelle et arrondie du nombre maximal de lignes dans la plus grande partition feuille. Pour les totaux de tables de qualité estimation, une valeur positive connue utilise le premier compartiment de lignes non nul, plafonné par `rows`. Pour une population de table lue avec précision, une estimation maximale dont le compartiment de confidentialité serait nul ou dépasserait cette population exacte est omise car non représentable, plutôt que d'être contrainte à une valeur fausse. Lorsqu'elle est présente, elle ne peut pas être nulle si `rows` est positif ou dépasser `rows`. |
| `temporal_history` | chaîne de caractères | Identifiant de table anonyme de la table d'historique temporelle associée, requis avec la fonctionnalité `temporal-current` sauf si cette table contient un jeton `table_limitations` applicable par objet. La sélection globale ne supprime jamais le lien. |
| `table_limitations` | tableau | Preuves complètes et triées par objet. `table-classification-unavailable` identifie une table dont les entrées de type d'objet étaient incomplètes. `column-inventory-unavailable` identifie une table avec un ou plusieurs enregistrements de colonnes manquants ou illisibles ; `dependent-structure-suppressed` indique que la structure d'index et de relation pour cette table ne peut pas être considérée comme complète car une colonne est absente. `index-inventory-unavailable` et `relationship-inventory-unavailable` réduisent une lacune de raffinement uniquement à la famille dépendante concernée, sans retirer l'inventaire de colonnes requis. Tout index, clé de partition ou relation qui fait référence à une colonne émise absente est omis plutôt que d'être autorisé à invalider toute la capture. `relationship-target-outside-selected-scope` enregistre qu'au moins une clé étrangère déclarée sur cette table cible un objet en dehors de la portée du schéma sélectionné résolu ; elle n'est valide que dans une capture `selection-limited`. `relationship-target-visibility-unknown` enregistre que le catalogue a révélé une cible de clé étrangère qui n'a pas pu être résolue dans l'inventaire visible ; la complétude de la relation doit alors être incomplète. `row-security-filter-active` enregistre un prédicat de filtre SQL Server visible et activé. `row-security-visibility-unknown` enregistre qu'il n'a pas été possible de prouver une visibilité complète du catalogue des politiques de sécurité de SQL Server, de sorte que l'échantillonnage de niveau 2 est supprimé plutôt que de traiter un sous-ensemble potentiellement filtré comme la population de la table. `temporal-history-outside-selected-scope` n'est valide que pour une table temporelle actuelle non liée dans une capture `selection-limited` après que le collecteur a résolu le schéma d'historique en dehors de la sélection. `temporal-history-visibility-unknown` enregistre que le catalogue a révélé un ID d'objet d'historique, mais pas suffisamment de métadonnées pour le résoudre. |
| `counted_in_totals` | bool | Omettre signifie inclus. Une `external-table`, `materialized-view`, `temporary-table`, ou une table contenant `memory-optimized` nécessite une spécification explicite `false`, excluant les données externes, dérivées, spécifiques à la session ou actuellement non mesurées provenant de `table_count`, `row_count`, `table_bytes` et `index_bytes`. Les preuves spécifiques à chaque objet restent disponibles pour la planification de la reconstruction, sans présenter de valeurs indisponibles comme des totaux mesurés. Aucune autre valeur explicite n'est considérée comme la valeur de référence. |
| `check_count` | int | Comptage optionnel et exact des contraintes CHECK structurelles. Omis signifie inconnu ; `0` signifie que le catalogue concerné n'en a trouvé aucune. |
| `has_clustered_index` | bool | toujours `false` pour PostgreSQL |
| `[tables.<id>.statistics]` | sous-table | Provenance requise en version 7 pour le nombre de lignes, l'état des statistiques de l'optimiseur et les informations de taille. Le champ `stats_freshness` de la version 6 n'est accepté que lors de la lecture de fichiers plus anciens et n'est jamais émis en version 7. |
| `[tables.<id>.cols.<cid>]` | sub-tables | une par colonne |
| `[tables.<id>.idxs.<iid>]` | sub-tables | une par index |
| `[tables.<id>.compression]` | sub-table | uniquement avec Tier 2 |

## `[tables.<id>.cols.<cid>]`

L'identifiant est `col-N`, où `N` est l'ordre naturel des attributs de la colonne (indexé à partir de 1, en conservant l'ordre sur disque). Stable d'une exécution à l'autre. Dans la version 7 du schéma, le suffixe décimal doit être exactement égal à `ordinal` ; les zéros, les zéros initiaux et les étiquettes dérivées de la source sont invalides. Les moteurs sources peuvent conserver des lacunes dans les ordonnes physiques des colonnes après la suppression d'une colonne.

| Champ | Type | Notes |
|---|---|---|
| `ordinal` | int | le même N que dans l'identifiant |
| `type` | string | famille de types normalisée, par exemple `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"` ou `"user-defined"`. Les noms réels de domaines, d'énumérations, d'alias, de composites et de types définis par l'utilisateur ne sont pas émis. |
| `nullable` | bool |  |
| `value_source` | string | Jeton fermé facultatif du schéma v6 : `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time` ou `rowversion`. Omis pour une valeur fournie ordinaire ou une preuve inconnue. |
| `has_default` | bool | Observation facultative du catalogue dans le schéma v6. L'absence signifie inconnu ; `false` explicite signifie que le catalogue a confirmé l'absence de valeur par défaut. |
| `default_kind` | string | Classification facultative `constant`, `function` ou `expression` dans le schéma v6, valide uniquement avec `has_default = true`. Le texte et les littéraux de la valeur par défaut ne sont jamais sérialisés. |
| `default_on_null` | bool | V7 : observation facultative du catalogue source pour Oracle `DEFAULT ON NULL` ; valide uniquement lorsqu'un défaut est présent. "Omis" signifie non observé. |
| `type_kind` | string | Jeton fermé facultatif du schéma v6 : `enum`, `set`, `domain`, `composite`, `array`, `range` ou `alias`. Omis pour un type de base ou une preuve inconnue. |
| `member_count` | int | Nombre structurel exact et positif de membres dans le schéma v6, requis uniquement pour `enum` et `set`. Les noms des membres ne sont jamais sérialisés. |
| `domain_has_check` | bool | Observation facultative du CHECK d'un domaine dans le schéma v6, valide uniquement avec `type_kind = "domain"`. |
| `hidden`, `invisible`, `masked`, `encrypted`, `sparse` | bool | Observations facultatives du catalogue. `invisible` est distinct d'une colonne cachée créée par le moteur. "Omis" signifie inconnu ; "explicite `false`" signifie que le catalogue a prouvé l'absence de cette propriété. |
| `has_check` | bool | Observation facultative d'un CHECK sur une seule colonne dans le schéma v6. Chaque `true` explicite est couvert par le `check_count` de la table. |
| `null_fraction` | flottant | Fraction de valeurs nulles observées, facultative, provenant de `0.0` à `1.0`. Lorsque la cardinalité est présente, elle est dérivée des comptages publics arrondis pour la confidentialité de ce bloc ; sinon, elle est arrondie indépendamment. Aucune bitmap de valeurs nulles n'est conservée. |
| `native_type` | string | Type de base facultatif et assaini du moteur, par exemple `varchar` ou `longtext` ; aucun identifiant, membre d'énumération, valeur par défaut ou expression. Émis par les collecteurs natifs MySQL et SQL Server. |
| `declared_max_chars` | int | Capacité déclarée facultative en caractères. Exacte pour les valeurs de catalogue PostgreSQL `character`/`character varying` et dans les modes MySQL équilibré par défaut/exact ; arrondie grossièrement uniquement avec MySQL `--length-fidelity strict`. |
| `declared_max_bytes` | int | Capacité déclarée facultative en octets. Exacte dans les modes MySQL équilibré par défaut et exact ; arrondie grossièrement uniquement avec `--length-fidelity strict`. |
| `length_semantics` | chaîne de caractères | V7 : unité de longueur déclarée facultative : `characters`, `bytes`, `not-applicable` ou `unknown`. Cela préserve la sémantique Oracle CHAR par rapport à BYTE sans sérialiser les déclarations. |
| `numeric_model` | chaîne de caractères | V7 nécessite un ensemble fermé : `integer`, `fixed-decimal`, `unconstrained-decimal`, `decimal-float`, `binary-float`, `not-applicable` ou `unknown`. `not-applicable` indique un type non numérique connu ; `unknown` est réservé pour un type numérique ou défini par l'utilisateur dont la sémantique n'a pas été classifiée. `decimal-float` inclut les valeurs exactes d'Oracle `FLOAT(p)` et n'est pas un nombre à virgule flottante IEEE. |
| `numeric_precision` | int | Précision déclarée positive facultative, limitée par le moteur et le modèle source : pour Oracle `NUMBER` et SQL Server, la précision décimale jusqu'à 38 ; pour Oracle `FLOAT(p)`, jusqu'à 126 chiffres binaires ; pour MySQL, la précision décimale jusqu'à 65 ; et pour PostgreSQL, la précision numérique jusqu'à 1 000. |
| `numeric_scale` | int | Échelle déclarée signée facultative, validée par rapport au moteur source. Oracle `NUMBER` utilise `-84..127` ; PostgreSQL prend en charge sa plage de déclaration plus large, qui dépend de la version, tandis que MySQL, SQL Server, Parquet et Avro nécessitent une échelle non négative qui ne dépasse pas la précision. Les valeurs négatives de Oracle/PostgreSQL et les échelles supérieures à la précision, lorsque le moteur le permet, sont conservées. |
| `numeric_precision_radix` | chaîne de caractères | `decimal` ou `binary` lorsque cela est requis par le modèle numérique. Oracle `FLOAT(p)` utilise une précision binaire avec le modèle de valeur exact `decimal-float` ; `BINARY_FLOAT` et `BINARY_DOUBLE` utilisent `binary-float`. |
| `numeric_unsigned`, `bit_width` | bool / int | Sémantique optionnelle des entiers lorsque le moteur source les expose. |
| `datetime_precision` | int | Précision fractionnelle date/time optionnelle, déclarée par le moteur. |
| `charset`, `collation` | string | Métadonnées de caractères facultatives et assainies. MySQL émet les noms de catalogue de son jeu de caractères et de sa collation. SQL Server émet `utf-16le` pour `nchar`/`nvarchar`/`ntext`, `utf-8` pour la page de codes 65001, `windows-N` pour les pages de codes Windows 1250 à 1258, ou `code-page-N` pour une autre page de codes positive du catalogue, ainsi que le nom de collation du catalogue. Ce sont des faits d'encodage et des noms de catalogue, jamais vos identifiants ni vos valeurs. |
| `len_avg` | int | Moyenne échantillonnée des octets pour les valeurs de longueur variable. Les classes relatives par défaut ont une erreur maximale d'environ 3,2 % et conservent exactement les valeurs jusqu'à 32 octets ; valeur exacte avec `--length-fidelity exact --yes` ; arrondi grossier à la dizaine uniquement en mode strict. 0 = longueur fixe ou non mesurée. |
| `len_p95` | int | 95e centile échantillonné avec les mêmes classes relatives par défaut ; valeur exacte avec `--length-fidelity exact --yes` ; arrondi grossier à la centaine uniquement en mode strict. 0 = non mesuré. |
| `style` | string | Tier 2 uniquement. L'une des valeurs `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"` ou `"precompressed"` ; vide si aucune classification n'est disponible. `"precompressed"` n'est émis que pour un échantillon matériellement dominant en octets de valeurs binaires portant des signatures reconnues de conteneurs standard. La famille de conteneurs détectée n'est volontairement pas divulguée. |
| `[tables.<id>.cols.<cid>.lob_storage]` | sous-table | Preuve du stockage optionnel de LOB (Large Objects) de base de données V7 : `storage_class` (`basicfile`, `securefile`, `external`, `unknown`), compression (`none`, `low`, `medium`, `high`, `not-applicable`, `unknown`), déduplication (`enabled`, `disabled`, `not-applicable`, `unknown`), options in-row/encrypted facultatives, et visibilité (`full`, `partial`, `unknown`). Le contenu externe nécessite que les deux contrôles de stockage soient `not-applicable` et omet les options internes à la base de données. Aucun nom de chemin ou de segment n'est conservé. |
| `magnitude_min`, `magnitude_max` | int | Exposants décimaux signés facultatifs du schéma v6 délimitant l'ordre de grandeur des nombres non NULL échantillonnés. Ils sont émis avec `has_negative` ; les valeurs exactes ne sont jamais sérialisées. |
| `has_negative` | bool | Observation facultative du signe dans le schéma v6, émise uniquement avec les deux limites d'ordre de grandeur. |
| `time_span` | string | Plage date/heure échantillonnée facultative du schéma v6 : `intraday`, `days`, `weeks`, `months`, `years` ou `decades`. |
| `time_recent_decade` | int | Décennie contenant la date/heure échantillonnée la plus récente dans le schéma v6, émise uniquement avec `time_span` et toujours divisible par 10. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Tier 2 uniquement. Présente pour les colonnes candidates de texte ou de données binaires échantillonnées. Même disposition des champs que la compression au niveau de la table, mais limitée à une colonne anonymisée. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Synthèse de la distribution des valeurs échantillonnées du schéma v3. Contient uniquement des comptages et fréquences bornés ou arrondis. |

`numeric_model` est l'autorité en matière de sémantique numérique. `type` conserve l'orthographe de la famille d'engines : la famille `NUMBER` d'Oracle est accompagnée de `type = "number"` et `FLOAT(p)` par `"float"`, tandis que `native_type` conserve la déclaration originale nettoyée.

### `[tables.<id>.cols.<cid>.cardinality]` (schéma v3)

Lorsque l'échantillonnage des lignes est activé, le collecteur conserve au maximum 8 192 empreintes numériques temporaires de 64 bits par colonne en mémoire, calcule les statistiques agrégées NDV/skew et supprime les empreintes numériques. Ni les valeurs ni les empreintes numériques ne sont sérialisées. Le bloc contient `measured`, `sample_rows`, `non_null_rows`, `observed_distinct_count`, `estimated_distinct_count`, `top_value_fraction`, `frequency_p50`, `frequency_p95`, `frequency_p99`, `frequency_max`, `sample_method`, `complete_source_read`, `sample_layout`, `sampled_with_bias`, et `bias_reason`. `complete_source_read = true` est une preuve lisible par machine indiquant qu'une instruction limitée a observé l'ensemble complet de la population de données source et a conservé cette colonne sans troncature au niveau des valeurs. Une lecture complète d'une ligne validée maintient `sample_rows` dans le domaine exact de la ligne de la table, même lorsqu'une limite de cellule ou un réservoir d'empreintes digitales borné rend `complete_source_read` faux ; la population exacte de la table est déjà présente dans `tables.<id>.rows`, donc cela ne révèle aucune information supplémentaire. `non_null_rows` est d'abord traité de manière à préserver la confidentialité, et `null_fraction` est ensuite dérivé de `(sample_rows - non_null_rows) / sample_rows`. La fraction reste donc parfaitement cohérente avec les données publiques et peut être supprimée de la grille de référence de 0,005 sans révéler d'autres informations. Les valeurs nulles et les points de terminaison non nuls sont préservés tels quels ; Un recensement mixte conserve une population positive et non nulle, et reste inférieure à `sample_rows`. Les valeurs `non_null_rows` mixtes utilisent la même grille de comptage relatif à la magnitude que les autres comptages de cardinalité. Dans cette situation, cela peut entraîner une diminution du nombre, pouvant aller jusqu'à un compartiment complet en dessous de la population réelle (par exemple, `9,728` pour `9,999`). ce n'est pas un décompte quasi-exact. Le point de terminaison exact, qui ne contient que des valeurs non nulles, révèle délibérément qu'aucune valeur NULL n'a été observée dans les lignes conservées, tandis que toute valeur NULL observée maintient le nombre total en dessous de `sample_rows`. Les décomptes distincts et les fréquences restent conformes à leur grille de confidentialité documentée, même lorsqu'ils sont limités par cette population. Pour les sources de bases de données, une lecture incomplète limite `sample_rows` à l'estimation arrondie du nombre de lignes du catalogue ; Pour Parquet et Avro, le nombre exact de lignes dans le pied de page est la limite. Dans les deux chemins, `sample_rows` représente le nombre exact de lignes conservées, déjà divulgué par le bloc de compression au niveau de la table, via `sample_rows`, sauf si ce plafond est inférieur. Il ne doit jamais être augmenté de manière à impliquer une couverture complète. La troncature des valeurs permet de conserver des preuves distinctes et une estimation de la fréquence, et ne doit jamais l'augmenter au-delà de la population de la table. Ne déduisez pas la complétude en analysant `sample_method`. `sample_layout` est une énumération facultative lisible par machine. La valeur actuelle émise est `primary-key-range-windows` ; "absence" signifie qu'aucun contrat de commande n'est disponible. Ne déduisez pas de sémantique en analysant le champ `sample_method` lisible par l'homme.

Les nombres et les fractions sont arrondis pour protéger la confidentialité, le cas échéant. Les statistiques décrivent la densité des doublons, la concentration des valeurs les plus fréquentes et les domaines finis. Elles ne contiennent aucune valeur échantillonnée, mais des distributions distinctives peuvent identifier une charge de travail ; ne les considérez pas comme irréversibles ou comme une preuve que le sens métier ne peut pas être déduit à partir de connaissances externes. Une déclaration limitée peut prouver la population de lignes visibles sans prouver que chaque cellule échantillonnée a été conservée intégralement. Si une limite de cellule côté serveur tronque une colonne, sa cardinalité reste une estimation limitée et biaisée, même lorsque le nombre de lignes de la table est enregistré à partir d'une lecture complète et limitée ; les colonnes non affectées peuvent toujours conserver la provenance de la cardinalité de la lecture complète.

### `[tables.<id>.cols.<cid>.compression]` (Tier 2 uniquement)

La compression par colonne est générée uniquement pour les candidats text/binary limités lorsque `--measure-compression --yes` est utilisé. Elle fournit une estimation de la compression par colonne.

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
sample_method = "TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "server_side_cell_cap"
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "blueprint-compression-probe-v2"
```

Aucune valeur de colonne échantillonnée n'est écrite dans le fichier Blueprint.

Pour les colonnes binaires, le même échantillon borné de Tier 2 peut émettre le
profil grossier `style = "precompressed"`. La reconnaissance n'a lieu qu'aux
limites des valeurs échantillonnées et exige une observation matériellement
dominante en octets. Blueprint n'analyse ni ne décompresse la valeur, ne
conserve pas sa signature et ne distingue pas les images, archives, médias
compressés, données chiffrées et données aléatoires au-delà de cette seule
étiquette à haute confiance. Les encodages textuels et base64 restent classés
selon leur style de texte et ne sont pas traités comme des conteneurs binaires
précompressés.

## `[tables.<id>.idxs.<iid>]`

L'identifiant est `idx-N`, où `N` est l'ordinal 1-indexé de l'index dans la table, trié par un HMAC-SHA256 séparé par des domaines du nom de l'index. Le schéma v7 nécessite l'ensemble dense `idx-1` à `idx-N` pour chaque table ; les zéros, les zéros non significatifs, les lacunes et les suffixes non décimaux sont invalides.

| Champ | Type | Valeurs |
|---|---|---|
| `type` | string | Famille de méthodes d'index normalisée, par exemple `"btree"`, `"hash"`, `"gin"`, `"gist"`, `"brin"`, `"spgist"`, `"fulltext"`, `"spatial"`, `"clustered"`, `"nonclustered"`, `"clustered columnstore"`, `"nonclustered columnstore"` ou `"other"`. Les noms de méthodes d'extension ou personnalisées ne sont pas émis. |
| `primary` | bool | Facultatif ; émis avec la valeur `true` pour les index de clé primaire. Omis ou faux dans les autres cas. |
| `unique` | bool |  |
| `cols` | array of int | ordinaux des colonnes participantes, dans l'ordre des colonnes de l'index |
| `prefix_lengths` | array of int | Longueurs facultatives des préfixes d'index MySQL alignées sur `cols` ; zéro signifie la colonne entière. Exactes par défaut ; arrondies vers le bas uniquement avec `--length-fidelity strict`. |
| `include_cols` | array of int | Facultatif ; ordinaux des colonnes INCLUDE hors clé lorsque le moteur source les expose. |
| `expression` | bool | Facultatif ; vrai lorsqu'un élément de clé basé sur une expression ou une fonction existe et ne peut pas être représenté par de simples ordinaux de colonnes. |
| `filtered` | bool | Facultatif ; vrai pour les index filtrés ou partiels. |
| `descending` | bool | Facultatif ; vrai lorsqu'au moins une colonne de clé est explicitement descendante. |
| `partitioning` | chaîne de caractères | Partitionnement physique optionnel V7 : `none`, `local`, `global` ou `unknown`. |
| `visibility` | chaîne de caractères | V7 : visibilité optionnelle de la source : `visible`, `invisible` ou `unknown`. |
| `state` | chaîne de caractères | État opérationnel optionnel V7 : `usable`, `unusable`, `in-progress`, `failed` ou `unknown`. |
| `prefix_distinct_counts` | array of int | Nombre estimé par le schéma v3 de tuples distincts pour chaque préfixe de clé, d'une à N colonnes. Zéro signifie que la valeur n'est pas disponible pour ce préfixe. |
| `cardinality_sample_method` | string | Provenance bornée de `prefix_distinct_counts` ; les produits inférés sont explicitement étiquetés et ne sont pas présentés comme des échantillons directs de tuples. |

## `[tables.<id>.compression]` et `[tables.<id>.cols.<cid>.compression]` (Tier 2 uniquement)

Présent uniquement lorsque le fichier a été généré avec `--measure-compression --yes`. Le bloc au niveau de la table mesure une projection colonnare neutre de l'échantillon complet et reste le ratio de référence pour les estimations de transfert de tables entières. Les blocs au niveau des colonnes sont projetés à partir des mêmes lignes échantillonnées, une colonne à la fois, et indiquent quelles colonnes se compressent bien sans exposer les valeurs échantillonnées. Ils ne déclenchent pas de lectures supplémentaires de la base de données.

Les tables PostgreSQL soumises à une sécurité au niveau des lignes, y compris les tables enfants héritées ou partitionnées dont la politique de l'ancêtre serait contournée par une requête directe, et les tables SQL Server soumises à un prédicat de filtre de sécurité activé, ne sont pas échantillonnées. Leur catalogue est conservé, et les enregistrements d'exécution `DBP1407W` sont conservés, plutôt que d'extrapoler un sous-ensemble filtré par la politique comme étant la totalité de la table.

| Champ | Type | Précision |
|---|---|---|
| `measured` | bool | toujours `true` si le bloc est présent |
| `sample_rows` | int | exacte |
| `sample_bytes` | int | taille du tampon d'échantillons en mémoire, **regroupée par paliers** : au multiple de **64 KiB** le plus proche pour une valeur inférieure à 1 MiB, au multiple de **1 MiB** le plus proche pour une valeur inférieure à 1 GiB, au multiple de **100 MiB** le plus proche au-delà. Les octets ne sont jamais écrits sur disque. Cette discrétisation élimine le canal caché dans les bits de poids faible par table qu'un `buf.len()` exact exposerait autrement. |
| `sample_method` | string | description bornée de l'échantillonnage propre au moteur, par exemple `"TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"`, `"LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"` ou `"SELECT TOP N bounded projection FROM <table> (compression sample; server-side cell cap)"` |
| `sampled_with_bias` | bool | vrai si l'échantillon n'est pas uniforme, par exemple en cas de repli sur LIMIT uniquement |
| `bias_reason` | string | Lorsque `sampled_with_bias = false`, ce champ est vide. Sinon, il contient une étiquette telle que `"unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"`. |
| `ratio_zstd_3` | float | arrondi au **0,05** le plus proche, selon la politique de mesure zstd niveau 3 du contrat. Mesuré sur des octets encodés selon `sample_encoding`. |
| `ratio_zstd_19` | flottant | Non rédigé par cette version ; peut apparaître dans des fichiers provenant de versions antérieures. |
| `ratio_stddev` | float | arrondi au **0,05** le plus proche, écart-type des ratios de niveau 3 sur des trames de sonde de table bornées. Les blocs de projection au niveau des colonnes émettent actuellement `0.0`, car il s'agit d'indications consultatives sur l'entropie et non d'un modèle de variance. |
| `sample_encoding` | chaîne de caractères | Identifiant pour la politique de codage au niveau des octets et de compression de session utilisée pour la mesure. Les blocs de tables en direct de PostgreSQL utilisent `"blueprint-columnar-transfer-probe-v2"`. MySQL et SQL Server utilisent `"blueprint-columnar-transfer-probe-v3"`, qui effectuent également un vidage à chaque limite de bloc de 256 Ko. Les charges utiles SQL Server `nvarchar`/`nchar`/`ntext` conservent la distribution native des octets UTF-16LE ; `varchar`/`char`/`text` conservent leur largeur d'octet échantillonnée, et le champ `charset` identifie la page de code du catalogue. La version 1 est acceptée en entrée. Les blocs par colonne utilisent `"blueprint-compression-probe-v2"`. Les ratios mesurés avec différentes valeurs de `sample_encoding` ne sont pas comparables. |

PostgreSQL utilise la version 2, tandis que MySQL et SQL Server utilisent la version 3 ; comparez les ratios uniquement au sein d'un même encodage.

### Encodage au niveau des octets `blueprint-compression-probe-v2`

L'échantillonneur Tier 2 concatène les lignes ou les valeurs de colonnes échantillonnées dans un tampon en mémoire au format suivant, puis applique zstd au niveau 3. Le tampon est supprimé. Le Blueprint conserve uniquement les champs agrégés documentés de compression, densité de valeurs NULL, cardinalité/fréquence, longueur et style.

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

Les balises de type font partie du contrat de la sonde et ne seront pas
renumérotées sans nouvel identifiant de sonde versionné.

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

### Encodage au niveau des octets `blueprint-columnar-transfer-probe-v1`, `v2` et `v3`

Les ratios de table issus d'une base active transforment les mêmes échantillons
bornés par colonne v2 en trames neutres de 1 000 lignes. Chaque trame possède un
en-tête de sonde versionné et, pour chaque colonne, un ordinal, une balise de
type, une longueur sur quatre octets par ligne, puis les octets de charge utile
contigus par colonne. La longueur `0xffffffff` représente NULL. La
représentation des octets est commune aux trois versions. V1 compressait la
séquence de trames jointe en une seule opération zstd niveau 3 avec taille
d'entrée annoncée. V2 alimente les trames dans un contexte zstd niveau 3
persistant et effectue un vidage après chaque trame. V3 conserve ce contexte et
la représentation neutre des groupes de lignes, mais effectue également un
vidage à chaque limite de bloc de compression de sonde de 256 KiB dans un groupe.
MySQL et SQL Server utilisent v3 ; v2 reste la mesure PostgreSQL actuelle. Le
texte Unicode SQL Server est mesuré en UTF-16LE. Le texte étroit SQL Server
conserve la largeur d'octet
source et enregistre un jeu de caractères fermé et assaini dérivé de la page de
codes de collation. Les sorties des groupes de lignes externes fournissent les
observations `ratio_stddev`. Les balises versionnées empêchent qu'une politique
de tramage ou de vidage soit silencieusement interprétée comme une autre.

Cette représentation modélise les propriétés génériques pertinentes pour la
compression d'un transfert en masse colonnaire. Il ne s'agit ni d'une capture
de protocole de base de données, ni d'un format de transport de migration, ni
d'un export de données encodé. Les octets échantillonnés restent uniquement en
mémoire et sont supprimés après dérivation des mesures agrégées.

### Limites de précision

`ratio_zstd_3` décrit l'élément nommé `sample_encoding` ; il ne s'agit pas d'une capture de bytes du protocole de base de données ou du processus de migration. La suite de tests dans ce référentiel valide l'encodage déterministe, l'échantillonnage limité et la sérialisation, mais ne prétend pas à un pourcentage d'erreur universel entre les différents moteurs pour chaque chemin d'extraction.

Avant d'utiliser le ratio pour une décision importante concernant la capacité, vérifiez le ratio par rapport à des données sources représentatives et au mécanisme d'extraction prévu. Enregistrez la méthode de comparaison, la taille de l'échantillon, le hachage binaire, la version du moteur et l'erreur observée avec le plan résultant. La relation primitive est `compressed_bytes ≈ sample_bytes / ratio_zstd_3` en fonction de la distribution des octets produite par l'encodage enregistré.

## `[fk_edges]`

Facultatif. Table en ligne dont chaque clé est un identifiant `table-NNN`
associé à une liste d'arêtes. Le schéma v3 conserve les ordinaux parents, les
actions référentielles, le mode de correspondance, le caractère différable,
l'état de validation/confiance et une synthèse relationnelle facultative,
bornée et sans noms. Les arêtes sont triées par destination,
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
colonne sont explicitement marquées comme inférées.

## `[artifact_inventory]` (depuis la version du schéma 4 ; requis dans la version 7)

La version 7 du schéma utilise le contrat `dbwarp-blueprint-artifacts/v2`, versionné indépendamment, pour décrire les objets non tabulaires sans sérialiser les noms ou les définitions de la source. Les versions antérieures du schéma conservent le contrat v1. La version 7 génère toujours ce bloc : `--artifact-detail none` enregistre explicitement un inventaire de la base de données qui n'a pas été demandé, tandis que les sources de fichiers structurés génèrent un inventaire explicite qui n'est pas applicable. Un bloc manquant ne sera donc jamais confondu avec un catalogue vide vérifié.

Par défaut, `--artifact-detail summary` émet `object_count`,
`external_prerequisite_count`, `counts_by_kind` et
`counts_by_external_class`. `graph` ajoute un enregistrement d'objet anonyme
par artefact et les arêtes de dépendance. `analyzed` ajoute des enregistrements
bornés `dbwarp-language-feature-census/v1`, dérivés transitoirement des
définitions disponibles. `graph` et `analyzed` exigent explicitement `--yes`,
car la topologie du graphe peut identifier une application.

`object_count` est le nombre d'enregistrements d'artefacts émis par le collecteur, et non le nombre de lignes renvoyées par un catalogue natif quelconque. Un package ou un type peut donc contribuer des enregistrements de spécification, de corps et de membres distincts. Un objet natif qui apparaît dans plus d'un catalogue est toujours un seul enregistrement : par exemple, les lignes de déclencheur Oracle provenant des catalogues de déclencheur et de source sont liées par leur identité d'objet natif, et les lignes de source enrichissent plutôt que de dupliquer l'enregistrement du déclencheur.

Les packages et les types d'objets Oracle utilisent la même structure de données : un enregistrement `specification`, un enregistrement `body` lié en tant qu'implémentation, et un enregistrement `package_member` procedure/function par membre de catalogue, avec la spécification comme parent. Seul le corps possède le texte source combiné et le décompte linguistique ; les membres conservent leurs informations de catalogue, mais utilisent une analyse de définition non applicable. Cela empêche un analyseur lexical de prétendre pouvoir diviser le code source du package en corps de membres.

Les preuves au niveau de l'inventaire comprennent :

| Champ | Valeurs / règle |
|---|---|
| `detail` | `none`, `summary`, `graph` ou `analyzed` |
| `scope` | V7 : `all-visible-schemas`, `selected-schemas`, `structured-source` ou `unknown` ; cela doit être cohérent avec les informations de sélection du schéma ailleurs dans le fichier. |
| `visibility` | `full`, `privilege_filtered` ou `unknown` |
| `inventory_complete` | Ne peut être vrai qu'avec une visibilité complète, aucun catalogue illisible et aucune famille non modélisée déclarée |
| `dependencies_complete` | Ne peut être vrai que si les catalogues de dépendances modélisés étaient lisibles |
| `requirements_complete` | Agrégat V7 : vrai uniquement avec une couverture complète de la population d’évaluation pour la portée sélectionnée et `requirement_status = complete | not_applicable` pour chaque artefact émis ; l’omission signifie faux et une liste d’exigences vide ne prouve pas la complétude |
| `analysis_complete` | Ne peut être vrai qu'au niveau analyzed et seulement si chaque analyse émise est complète |
| `catalogs_read` | Libellés fermés et standard des catalogues moteur inspectés avec succès |
| `catalogs_unreadable` | Les étiquettes de catalogue qui ont échoué ; chaque entrée empêche les affirmations de complétude fournies par ce catalogue, tandis que les preuves de conformité spécifiques à chaque objet peuvent rester complètes. |
| `catalogs_not_applicable` | Les étiquettes du catalogue V7 se sont avérées inapplicables ; elles sont distinctes des catalogues lisibles et illisibles. |
| `families_not_inventoried` | Familles d'objets connues qui ne sont pas répertoriées dans cette version |

### `[artifact_inventory.complexity]` (schéma v7)

Le bloc `dbwarp-blueprint-artifact-complexity/v1` est une évaluation qui ne concerne que les données agrégées, basée sur le recensement des artefacts anonymes. Il est absent dans les détails `none` et `summary`, et requis dans les détails `graph` et `analyzed`, où sa présence signifie que l'évaluation a été tentée. Un échec de calcul produit un résultat inconnu par défaut plutôt que d'interrompre l'exécution de DBWarp Blueprint.

Les champs de niveau supérieur sont fixes :

| Champ. | Valeurs / règle. |
|---|---|
| `contract` | `dbwarp-blueprint-artifact-complexity/v1` |
| `assessor_version` | `1` |
| `scope` | Doit être exactement égal à `artifact_inventory.scope`. |
| `population_policy` | `exclude-known-engine-generated-and-secondary` ; les indicateurs manquants restent valides et les objets temporaires restent valides. |
| `assessment_population_complete` | Vrai uniquement lorsque tous les objets éligibles selon la politique de population sont connus ; l'omission signifie faux, et cette affirmation est indépendante du champ `inventory_complete` plus large. |
| `eligible_object_count` | Objets évalués par la politique. |
| `fully_assessed_object_count` | Chaque dimension est soit connue, soit prouvée `not-applicable`. |
| `partially_assessed_object_count` | Au moins une dimension applicable connue et au moins une dimension inconnue. |
| `unassessed_object_count` | Aucune dimension applicable connue. |
| `excluded_object_count` | Objets exclus par la politique enregistrée. |
| `analyzer_version` | L'unique analyseur utilisé par la capture v7 : `lexical-v2`, ou `not-applicable` en mode graphique. |
| `analysis_spans` | Les étendues fermées, uniques et triées présentes dans les registres du recensement éligibles sont : `executable-body`, `not-applicable` ou `unknown` ; vides en mode graphique. |
| `dialects` | Les jetons de dialecte fermés, uniques et triés présents dans les registres du recensement éligibles. |
| `grammar_profiles` | Profils grammaticaux uniques et triés présents dans les registres du recensement admissibles. |
| `overall_band` | `trivial`, `low`, `moderate`, `high`, `very-high`, `not-applicable`, ou `unknown` |
| `overall_score` | Non rédigé par cette version. |
| `limitations` | Les raisons de fermeture classées sont décrites ci-dessous. |

Les deux équations de population utilisent une arithmétique vérifiée :

```text
artifact_inventory.object_count = eligible_object_count + excluded_object_count
eligible_object_count = fully_assessed_object_count
                      + partially_assessed_object_count
                      + unassessed_object_count
```

`dimensions` contient exactement `volume`, `control_flow`, `feature_breadth`, `entanglement`, `environment_coupling`, `opacity` et `dialect_coupling`. Chaque dimension a une limite `band`, une valeur `coverage` (`complete`, `partial`, `not-applicable` ou `unknown`), et un histogramme fixe. La dimension `volume` `band`, comme les autres verdicts de dimension, utilise `trivial`, `low`, `moderate`, `high`, `very-high`, `not-applicable` ou `unknown`. Son histogramme utilise les clés de taille `0`, `1-255`, `256-1k`, `1k-4k`, `4k-16k`, `16k-64k` et `64k+`. Les six autres histogrammes utilisent les clés de comptage `0`, `1`, `2-4`, `5-8`, `9-16`, `17-32` et `33+`. Chaque histogram a également des compartiments `not_applicable` et `unknown`. Pour chaque dimension, les vérifications arithmétiques nécessitent :

```text
eligible_object_count = assessed evidence-band counts
                      + not_applicable
                      + unknown
```

La couverture est donc par dimension, et non pas un simple indicateur au niveau de l'ensemble. Un résultat de recensement `not_applicable` constitue une preuve et contribue au "bucket" `not_applicable` de la dimension ; il ne rend pas l'objet non évalué. Les nombres d'objets fully/partially/unassessed de niveau supérieur sont un résumé dérivé : tous les objets non applicables sont entièrement évalués, "partiel" signifie qu'au moins une dimension applicable est connue et qu'une autre est inconnue, et "non évalué" signifie qu'aucune dimension applicable n'est connue.

Une dimension partielle est évaluée comme une borne inférieure et une borne supérieure. Sa `band` est `unknown` sauf si la borne inférieure connue est déjà `very-high`, car toute observation inconnue peut se trouver dans le compartiment le plus élevé. Cela empêche un histogramme partiel de présenter sa borne inférieure observée comme un verdict définitif.

`external_binary` est un état de visibilité des définitions, et non un indicateur d'exclusion. Les plugins installés sur le site, les assemblies CLR, les objets Java et les bibliothèques externes restent des éléments de migration éligibles et contribuent généralement à des informations inconnues dépendantes de la définition. Seul l'indicateur `generated_by_engine = true` explicite exclut un objet fourni par le moteur, selon l'évaluateur v1.

Les histogrammes sont intentionnellement unidimensionnels. Les tableaux croisés par type, caractéristique, schéma ou tout autre attribut ne font pas partie du contrat. Les décomptes exacts n'apportent aucune information supplémentaire par rapport au recensement sérialisé par objet en mode analyse, tandis que la forme fixe évite de publier un outil officiel de reconnaissance de l'infrastructure.

Les raisons de limitation en cours sont `definition-analysis-not-requested`, `definitions-withheld`, `unsupported-dialect`, `wrapped-source`, `graph-incomplete`, `requirements-incomplete`, `outside-selected-scope`, `computation-limit` et `computation-failed`. `requirements-incomplete` signifie que la capture des exigences n'est pas complète. Les objets avec un statut d'exigence `partial` ou `unavailable` contribuent à des observations de couplage environnemental et dialectal inconnues plutôt que nulles ; les objets entièrement complets restent évalués. `unsupported-dialect` signifie que la définition a été obtenue, mais que sa langue ou son dialecte ne dispose pas d'un analyseur pris en charge ; elle n'est ni retenue ni intentionnellement opaque. Les détails du graphe utilisent `definition-analysis-not-requested` ; il ne doit pas prétendre à une limitation de lecture de définition, car aucune tentative de lecture n'a été effectuée.

Les preuves inconnues sont limitées indépendamment pour chaque dimension concernée. La plage globale n'est émise que lorsque les évaluations inférieure et supérieure concordent. Une population entièrement vide et éligible est `not-applicable`, et jamais `trivial`. Le mode graphique utilise toujours une `unknown` globale pour une population non vide, car il ne lit pas les définitions requises par l'évaluation globale. L'absence d'arêtes dans le graphique rend les preuves d'interconnexion concernées inconnues. `computation-limit` n'est pas écrit par cette version. Une erreur de calcul inattendue enregistre `computation-failed`, conserve l'inventaire complet des artefacts et marque l'évaluation agrégée comme "échec", plutôt que de la supprimer.

`assessment_population_complete`, plutôt que `artifact_inventory.inventory_complete` de manière générale, détermine si le résultat limité peut être définitif. Si la population évaluée est incomplète, la plage globale est `unknown` sauf si la borne inférieure connue est déjà `very-high` ; aucune borne supérieure finie n'est supposée pour les objets qui peuvent être invisibles.

Les objets entièrement inclus contribuent `unknown` à l'opacité ; ils ne sont pas omis de l'histogramme de l'opacité simplement parce qu'aucun recensement partiel n'a pu être produit. Présentez l'opacité à côté de sa couverture afin qu'une faible bande d'opacité observée ne puisse pas masquer une grande population inconnue.

`unsupported-dialect` reste une limitation distincte car le recensement peut identifier un dialecte et signaler `unavailable`, mais n'a pas de statut `unsupported`. Il est dérivé uniquement lorsqu'une définition était disponible et que le dialecte enregistré n'est pas pris en charge par l'analyseur spécifié. Les autres limitations de définition sont également dérivées de la visibilité de la définition, du statut du recensement et des preuves des artefacts, plutôt que d'être maintenues comme une affirmation indépendante.

L'éligibilité à la comparaison est calculée entre les fichiers à partir du contrat de complexité, de la version de l'assesseur, de la version de l'analyseur, de l'étendue exacte de l'analyse, des ensembles de dialectes et de profils grammaticaux, de la portée et de la politique de population. Un indicateur homogeneous/mixed de granularité grossière n'est pas sérialisé car différents ensembles mixtes ne sont pas nécessairement comparables.

La complexité est toujours propre à la source. Un ensemble conserve l'évaluation de chaque Blueprint enfant et ne crée jamais de niveau de complexité global ou d'histogramme entre les moteurs, les versions de l'analyseur, les dialectes ou les profils de grammaire.

Les identifiants propres à chaque objet ont la forme `<kind>-NNN`, tels que `view-001`, `package-002` ou `procedure-003`. V7 reconnaît les familles d'objets courantes, ainsi que les packages Oracle, les objets de planification, les liens de base de données, les répertoires, les bibliothèques, les objets Java, les opérateurs, les types d'index, les domaines, les annotations et les graphes de propriétés, ainsi que les types `queue` et `edition` indépendants de l'environnement. Le minimum à trois chiffres est complété par des zéros, et chaque type possède son propre ensemble ordinal dense commençant par `001` ; la largeur augmente au-delà de 999. L'enregistrement contient uniquement des jetons kind/subkind/tier fermés, des identifiants schema/parent anonymes, le mode de définition visibility/security, des indicateurs de validité et de catalogue facultatifs, une couverture des exigences fermée, une prérequis externe facultatif et un recensement facultatif du langage. Un parent peut être une table anonyme ou un autre artefact, de sorte qu'une hiérarchie de packages à procédures peut être préservée sans noms ; les graphes parents doivent être acycliques.

V7 utilise un vocabulaire fermé unique `subkind` pour tous les moteurs :

```text
ordinary, other, materialized, integer_sequence, stored_procedure,
stored_function, scalar_function, inline_table_function, table_function,
user_defined_aggregate, table_trigger, ddl_event_trigger, before_insert,
before_update, before_delete, after_insert, after_update, after_delete,
generated_column, column_default, default_constraint, check_constraint,
row_security, rewrite_rule, legacy_rule, enum, domain, composite, range,
alias_type, table_type, clr_type, clr_procedure, clr_scalar_function,
clr_table_function, clr_aggregate, clr_trigger, clr_assembly,
server_extension, loadable_udf, foreign_data_wrapper_server, foreign_table,
federated_table, external_table, external_data_source, external_file_format,
logical_replication_publication, logical_replication_subscription,
full_text_catalog, partition_scheme, partition_function, tablespace, filegroup,
database_certificate, symmetric_key, asymmetric_key, column_master_key,
column_encryption_key, database_scoped_credential, linked_server,
enabled_event, disabled_event, enabled_agent_job, disabled_agent_job,
database_synonym, specification, body, package_member, public, private,
java_source, java_class, java_resource, external_library,
user_defined_operator, domain_indextype, scheduler_job, scheduler_program,
scheduler_schedule, scheduler_chain, advanced_queuing, service_broker
```

V7 remplace la liste de dépendances ambiguë v1 par une liste triée et typée `relationships`. Les types de relations distinguent les appels, les lectures, les écritures, les références table/object, la propriété des déclencheurs, l'implémentation, le placement physique, la sécurité, l'utilisation des extensions, les références externes binaries/services et l'utilisation distante database/server. Chaque relation enregistre un jeton de preuve fermé (`catalog-confirmed`, `dependency-confirmed`, `syntax-confirmed`, `lexical-hint` ou `unresolved`). `dependency_edge_count` doit être exactement égal au graphe généré.

`requirements` Utilisez des jetons qualifiés par le moteur et une plage de comptage limitée. Ils identifient les besoins de compatibilité tels qu'une source Oracle encapsulée, un déclencheur composite, l'état du package, le SQL dynamique, une transaction autonome, pipelined/parallel/aggregate une routine, une bibliothèque externe, un lien de base de données, un index de domaine, un planificateur, object/collection/spatial/vector un type, un objet Java ou un graphe de propriétés. Ils ne sont qu'une preuve de planification.

Les exigences proviennent d'une information catalogue limitée ou d'une vérification syntaxique spécifique au moteur. L'analyse lexicale générique ne crée jamais d'exigence qualifiée par le moteur. Chaque graphe schema-v7 ou artefact analysé contient `requirement_status = complete | partial | unavailable | not_applicable`. `complete` indique que la liste est exhaustive pour cet artefact ; `not_applicable` indique que le modèle d'exigence ne s'applique pas et interdit donc les enregistrements d'exigences et de prérequis externes. `partial` et `unavailable` rendent les observations de couplage environnemental et dialectal de cet objet inconnues. `partial` signifie qu'au moins une source d'information a réussi sans couverture exhaustive ; `unavailable` signifie qu'aucune source d'exigence n'a établi une couverture utilisable et ne peut donc pas contenir de preuves d'exigences ou de prérequis externes connus. De telles preuves nécessitent `partial`. Cela permet à un objet inaccessible de se dégrader localement au lieu d'effacer une couverture utile pour le reste du système.

Le niveau d'inventaire `requirements_complete` est une affirmation globale. Il ne peut être vrai que si chaque artefact généré est `complete` ou `not_applicable` et que la population évaluée est complète pour la portée sélectionnée ; il peut rester faux même si chaque artefact est `complete`. N'interprétez jamais un tableau `requirements` vide comme un couplage nul, sauf si l'état de cet artefact est `complete`.

`unresolved_relationships` est une carte bornée reliant les raisons aux nombres. Elle distingue les références distantes et inter-bases de données, les limites des schémas sélectionnés, les cibles masquées par les privilèges, les définitions chiffrées ou omises, le SQL dynamique, les liaisons ambiguës, les identités natives manquantes ou incomplètes, les familles de cibles non modélisées et les cas inconnus. Une preuve complète de dépendance nécessite que cette carte soit vide. Les noms d'objets source, le texte SQL, les principaux, les points de terminaison, les identifiants, les clés, les certificats et les binaires ne sont pas des champs du contrat.

Les prérequis externes enregistrent une `class` fermée, la portée du déploiement,
le besoin d'éléments binaires/secrets/points de terminaison non capturés et une
catégorie de compatibilité bornée. Leur nombre est une preuve de planification
de migration, pas une affirmation que DBWarp peut les provisionner ou les
traduire automatiquement.

Les enregistrements de recensement de la version 7 utilisent `analyzer_version = "lexical-v2"` et enregistrent `analysis_span`. L'analyseur ne reçoit que le corps exécutable ou déclaratif, en excluant l'enveloppe de création externe, l'identité, la signature, la déclaration de retour et les options du module. Les informations d'en-tête restent des exigences de catalogue ou des indicateurs. Un collecteur qui ne peut pas isoler en toute sécurité le corps enregistre `analysis_span = "unknown"` et des preuves indisponibles au lieu d'analyser l'enveloppe. Une définition non applicable prouvée utilise `analysis_span = "not-applicable"`. Les preuves d'étendue omise sont interprétées de manière conservatrice comme `unknown` ; elles ne sont jamais déduites du moteur ou du type d'objet. Une définition prise en charge analysée par cette implémentation lexicale utilise `status = "partial"` ; les preuves de définition manquantes ou non prises en charge utilisent `unavailable`, et un objet non applicable prouvé peut utiliser `not_applicable`. Le nombre, la taille, l'imbrication, la complexité et les valeurs des régions opaques sont des plages, et non des empreintes digitales exactes de la source. Les fonctionnalités sont sélectionnées à partir d'un vocabulaire fermé. L'analyseur supprime les commentaires, les littéraux et les identificateurs entre guillemets ; il ne s'agit pas d'un analyseur syntaxique, d'un lieur sémantique ou d'une garantie de succès de la traduction.

"Wrapped PL/SQL ne constitue jamais une preuve du contenu exécutable. Le collecteur l'identifie comme étant chiffré et retient ses octets de l'analyse ; l'analyseur partagé refuse également un élément PL/SQL dont l'en-tête contient le marqueur "wrapped", empêchant ainsi une erreur de classification de produire des tranches de données censitaires plausibles mais fausses."

Consultez l'[Inventaire des artefacts hors tables](ARTIFACT_INVENTORY.md) pour
les instructions opérationnelles et la couverture des moteurs.

## Défenses contre la stéganographie, par vecteur

| Vecteur | Défense |
|---|---|
| Ordre des identifiants. | L'utilisation de HMAC-SHA256 avec une clé spécifique au processus, séparée par des domaines, empêche les vérifications hors ligne des noms de candidats. Réutilisez une clé uniquement lorsque des étiquettes stables entre les exécutions sont nécessaires. |
| Bits de poids faible des nombres | Les statistiques sont arrondies par défaut selon la précision documentée. Le mode de longueurs exactes est explicite, soumis au consentement, enregistré dans le journal d'audit et doit être traité comme une métadonnée plus sensible. |
| Horodatage inférieur à la seconde | Un seul horodatage UTC au début, à la seconde uniquement |
| Formatage TOML | La sortie standard utilise un ordre de clés et une indentation qui ne varient pas. Elle inclut uniquement l'en-tête standard et les commentaires du producteur, sans aucun commentaire provenant de l'entrée. |
| Aléatoire d'échantillonnage. | L'échantillonnage utilise des graines fixes (déterministes `TABLESAMPLE SYSTEM` dans PostgreSQL). De plus, l'anonymisation des identifiants obtient intentionnellement une clé secrète auprès du générateur de nombres aléatoires cryptographiquement sécurisé (CSPRNG) du système d'exploitation, sauf si vous en fournissez une. |
| Champs inutilisés | Chaque champ est documenté ci-dessus ; aucun champ « metadata »/« comment »/« reserved » susceptible de transporter des données de taille illimitée |
| Texte source des artefacts et éléments externes | Les définitions sont transitoires et effacées après l'analyse bornée ; noms, texte SQL, points de terminaison, chaînes de fournisseur, informations d'identification, clés, certificats, noms de paquets et binaires n'ont aucun champ sérialisé |

## Compatibilité des versions de schéma

Les producteurs actuels émettent la version 7 du schéma. Les versions 1 à 6 restent acceptées pour assurer la compatibilité descendante. Un fichier v1/v2 ne contient aucun bloc de distribution. Un fichier v3 contient des métadonnées de distribution, mais aucun inventaire d'artefacts. Un fichier v4 peut contenir un inventaire d'artefacts, mais il est antérieur aux identifiants de contrat Blueprint actuels. Les lecteurs normalisent les anciens identifiants v4 à l'entrée et réémettent ce document avec les identifiants Blueprint canoniques. Un fichier v5 est antérieur aux preuves de topologie et de portée des ensembles de données ajoutées dans la version 6. La version 6 utilise le contrat de topologie v1, le contrat d'artefact v1, les champs de table combinés kind/partition, une échelle décimale non signée et des statistiques de fraîcheur facultatives. La version 7 utilise le contrat de topologie v2 et le contrat d'artefact v2, nécessite des preuves explicites structure/environment/statistics, sépare les sémantiques de table orthogonales, prend en charge les échelles décimales signées et les modèles numériques Oracle, et nécessite un état explicite de l'inventaire des artefacts. Elle réserve également le contrat d'agrégation fixe `dbwarp-blueprint-artifact-complexity/v1` sans ajouter de score par objet ni de champ de croisement. Les lecteurs rejettent les versions de schéma futures inconnues avec un message de mise à niveau clair plutôt que de supprimer silencieusement les champs. Les lecteurs appliquent la même validation stricte v7 aux Blueprints autonomes et intégrés plutôt que de réécrire les preuves non valides pendant l'analyse.

## Pourquoi TOML plutôt que JSON

- TOML sépare plus lisiblement les sections structurelles des données terminales (`[tables.table-001.cols.col-2]` au lieu d'un JSON imbriqué).
- Les comparaisons sont plus simples (une clé par ligne ; les sous-tables identifiées restent contiguës).
- Vérifiez conformément à la politique de classification des données de votre organisation avant de partager.

JSON est utilisé comme **format intermédiaire** dans le chemin de repli SQL. Chaque script `sql/blueprint.*.sql` produit du JSON et `blueprint_format.py` le normalise en TOML. Le JSON intermédiaire contient les identifiants source réels ; MySQL peut aussi inclure des déclarations enum/set dans `COLUMN_TYPE`. Il doit donc rester protégé dans l'environnement source. Le normaliseur utilise une nouvelle clé secrète par défaut et accepte le même contrat `--anonymization-key-file` protégé pour les comparaisons approuvées entre exécutions. Le fichier final vérifié pour partage avec DBWarp est toujours au format TOML.

## Extensions de provenance des fichiers structurés

Lorsque `engine` ou `source_kind` est `"parquet"` ou `"avro"`, la version 3 ou supérieure du schéma peut également générer les champs limités suivants. Les lecteurs doivent conserver la distinction entre le stockage des fichiers sources et les mesures échantillonnées et limitées ; les lecteurs qui ne prennent pas en charge la version du schéma du document doivent le rejeter avec un message de mise à niveau plutôt que de supprimer les champs inconnus.

Les Blueprints de fichiers structurés V7 nécessitent des blocs `[structure_scope]` et `[statistics_evidence]` complets, omettent `[database_topology]`, `[source_environment]` et `[activity_snapshot]`, et émettent un `[artifact_inventory]` explicite indiquant qu'ils ne sont pas applicables. Ils ne déduisent jamais la topologie de la base de données ni la capacité du serveur-machine à partir de l'hôte du collecteur.

Les Blueprints de fichiers structurés utilisent les mêmes identifiants anonymisés
que les Blueprints de bases de données : `table-NNN` dans l'ordre fondé sur la clé secrète et
`col-N` dans l'ordre ordinal du schéma. Les noms de fichiers, les
chemins Parquet, les noms de champs Avro et la valeur `logical_table` du
manifeste ne deviennent jamais des identifiants de table ou de colonne.

À l'échelle de la table, `table_bytes` est l'estimation logique de la taille du transfert, tandis que `storage_bytes` est la taille réelle de l'objet source sur le disque. Parquet avec uniquement des métadonnées utilise des octets non compressés pour les blocs de colonnes pour `table_bytes` ; un échantillonnage décodé facultatif remplace cette estimation par les octets projetés `blueprint-compression-probe-v2`. Avro le dérive de sa lecture complète décodée. Les champs facultatifs `source_partitions`, `row_group_count` et `source_codec` décrivent la structure du fichier. Les ensembles de données multi-fichiers agrègent ces valeurs. `row_group_count` est spécifique à Parquet ; `source_partitions` est `1` pour un seul objet d'entrée.

Au niveau colonne, `null_fraction` est une observation comprise entre `0.0` et
`1.0`. `length_sample_rows` et `length_sample_method` indiquent l'origine de
`len_avg` et `len_p95`. `source_semantics` conserve des faits de compatibilité
bornés comme `"repeated-leaf"`, `"nested-json"` ou `"multi-type-union"` ; il ne
contient jamais vos noms de champs ni vos valeurs. La précision et l’échelle
décimales, la précision et la sémantique UTC/locale des horodatages, les UUID et
les métadonnées binaires de taille fixe utilisent les champs scalaires existants expurgés et
`native_type`.

Au niveau table, `ratio_storage` compare `table_bytes` aux octets réels de
l'objet source. Au niveau d'une colonne Parquet, il compare les octets non
compressés et compressés du segment de colonne dans le footer. Ce sont des
signaux de planification de fichier, pas des estimations d'échantillons décodés.
`ratio_zstd_3` et `ratio_zstd_19` ne constituent des entrées valides que lorsque
`sample_encoding` vaut `"blueprint-compression-probe-v2"`. Les ratios de footer
Parquet ou de conteneur Avro ne doivent jamais être copiés dans ces champs zstd.
