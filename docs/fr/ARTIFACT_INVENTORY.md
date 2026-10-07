# Inventaire des artefacts hors table

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../ARTIFACT_INVENTORY.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../ARTIFACT_INVENTORY.md) | [Deutsch](../de/ARTIFACT_INVENTORY.md) |
**Français** | [Español](../es/ARTIFACT_INVENTORY.md) |
[Polski](../pl/ARTIFACT_INVENTORY.md) | [日本語](../ja/ARTIFACT_INVENTORY.md) |
[简体中文](../zh/ARTIFACT_INVENTORY.md)

Les Blueprints peuvent décrire les objets de base de données qui ne sont pas des tables et les prérequis de déploiement sans publier leurs noms de source, leurs définitions, leurs chaînes de point de terminaison, leurs secrets, leurs certificats, leurs clés ou leurs binaires. Cet inventaire aide DBWarp à estimer la complexité de la migration et à identifier les tâches qui nécessitent des packages, une infrastructure, une approbation de sécurité ou une conversion assistée.

L'inventaire ne constitue pas une affirmation de capacité. Le fait qu'un objet soit signalé ne signifie pas que DBWarp peut automatiquement le recréer ou le traduire. Veuillez vérifier auprès de DBWarp quels types d'objets sont pris en charge.

## Niveaux de détail

Utilisez `--artifact-detail` pour choisir le compromis entre confidentialité et
planification :

| Valeur | Lectures en base | Sortie Blueprint | Consentement |
|---|---|---|---|
| `none` | Aucun catalogue d'inventaire ni définition (la sonde de topologie limitée au comptage est toujours exécutée) | Inventaire v7 explicitement non demandé ; aucun compteur ni graphe | Aucun consentement supplémentaire |
| `summary` | Catalogues d'artefacts, sans définitions | Compteurs par type et classe de prérequis externe | Valeur par défaut ; aucun consentement supplémentaire |
| `graph` | Catalogues et métadonnées de dépendance, sans définitions | Compteurs, objets anonymes stables et arêtes | Nécessite `--yes` |
| `analyzed` | Catalogues, dépendances et définitions disponibles | Graphe et classes bornées de caractéristiques du langage de programmation et de complexité | Nécessite `--yes` |

La valeur par défaut est `summary`. Utilisez `none` si la politique autorise la
structure des tables mais interdit les catalogues hors table. Utilisez `graph` pour
une planification par dépendances sans lire les définitions, et `analyzed`
uniquement après approbation de leur lecture transitoire.

```bash
./dbwarp-blueprint \
  --connect postgresql://blueprint_user@db.internal/appdb \
  --password-file /etc/dbwarp/blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --artifact-detail analyzed \
  --out appdb.blueprint.toml \
  --audit-log appdb.blueprint.audit.txt \
  --yes
```

## Contrat de confidentialité

La sortie d'artefacts contient uniquement des métadonnées bornées issues d'un
vocabulaire fermé :

- des identifiants anonymes cohérents au sein d'une exécution, tels que
  `view-001`, `function-002` et `schema-A` ; leur stabilité entre les
  exécutions exige de réutiliser le même fichier protégé
  `--anonymization-key-file` ;
- des jetons fermés pour le type, sous-type, niveau, visibilité et mode de sécurité ;
- des relations typées exprimées uniquement par des identifiants anonymes d'artefact ou de table, avec des jetons fermés pour les preuves et les motifs non résolus ;
- des exigences fonctionnelles bornées et qualifiées par moteur pour la planification de migration ;
- des compteurs et classes bornées plutôt que des descriptions libres ;
- des noms de catalogues standard tels que `pg_proc`, `information_schema.views` ou `sys.objects` ;
- des classes de prérequis externes, jamais leurs noms ni leur contenu.

La sortie ne contient pas les noms d'objets source, le texte source SQL ou
procédural, les noms de schéma, les principaux, les chaînes de point de
terminaison, les chaînes de fournisseur, les informations d'identification,
les clés, les corps de certificats, les fichiers d'assembly, les noms de paquets
d'extension ou les noms de bibliothèques chargeables.

En mode `analyzed`, les définitions ne restent en mémoire que le temps de
supprimer commentaires et littéraux et de produire des agrégats lexicaux
bornés. Elles sont détenues par un conteneur effacé à la libération et ne sont
ni sérialisées, ni journalisées, ni envoyées à un autre service. Il s'agit d'une
réduction de l'exposition mémoire, pas d'une garantie contre la pagination du
système ou un débogueur privilégié.

Même un graphe anonyme peut caractériser une application par ses comptes et sa
topologie. C'est pourquoi `graph` et `analyzed` échouent avec `DBP1014E` sans
`--yes` explicite.

## Preuves d'exhaustivité

Le bloc `[artifact_inventory]` est volontairement auto-auditable :

| Champ | Signification |
|---|---|
| `contract` | Contrat versionné indépendamment ; v7 utilise `dbwarp-blueprint-artifacts/v2` et les anciens schémas Blueprint conservent v1 |
| `detail` | Niveau de détail demandé |
| `scope` | Portée de catalogue v7 : `all-visible-schemas`, `selected-schemas`, `structured-source` ou `unknown` |
| `visibility` | `full`, `privilege_filtered` ou `unknown` |
| `inventory_complete` | Vrai uniquement avec visibilité totale, aucun catalogue illisible et aucune famille non modélisée déclarée |
| `dependencies_complete` | Vrai uniquement si les sources de dépendances étaient lisibles et si les familles modélisées sont couvertes |
| `requirements_complete` | Agrégat V7 : vrai uniquement après vérification de la version et de l’édition du moteur, couverture complète de la population d’évaluation pour la portée sélectionnée et `requirement_status = complete | not_applicable` pour chaque artefact émis ; l’omission signifie faux |
| `analysis_complete` | Vrai uniquement avec `analyzed` et une analyse complète de toutes les définitions disponibles |
| `catalogs_read` | Familles de catalogues standard inspectées avec succès |
| `catalogs_unreadable` | Familles de catalogues en échec ou indisponibles ; les affirmations concernées sont dégradées sans effacer les preuves indépendantes par objet |
| `catalogs_not_applicable` | Familles prouvées inapplicables ; ensemble disjoint des catalogues lisibles et illisibles |
| `families_not_inventoried` | Familles d'objets connues non inventoriées par cette version |

L'échec d'un catalogue optionnel ne supprime pas silencieusement des objets. Le
programme émet `DBP1410W`, enregistre le catalogue concerné et force les
indicateurs d'exhaustivité correspondants à faux. Un compte peu privilégié peut
donc fournir un inventaire partiel utile sans présenter l'absence comme preuve.

`object_count` compte les enregistrements d'artefacts émis, pas les lignes d'un
seul catalogue natif. Les packages et types d'objets Oracle sont modélisés sous
forme de spécification, corps et membres ; le corps porte l'analyse linguistique
combinée. Un objet vu dans plusieurs catalogues n'est compté qu'une fois. Les
métadonnées et la source d'un déclencheur sont jointes par identité native avant
l'anonymisation.

## Contrat de complexité agrégée

Le schéma v7 définit `[artifact_inventory.complexity]`, un enregistrement
strictement agrégé pour `graph` et `analyzed`. Il n'ajoute ni lecture ni
permission : l'évaluation dérive du graphe anonyme et du recensement linguistique
déjà autorisés. Il est obligatoire pour `graph` et `analyzed`, absent pour
`none` et `summary`.

L'évaluation rapporte sept dimensions : volume, flux de contrôle, étendue des fonctionnalités, enchevêtrement des dépendances, couplage environnemental, opacité et couplage dialectal. Les résultats sont des plages plutôt qu'un score numérique. `overall_score` est réservé et n'est pas renseigné, car une valeur de 0 à 100 impliquerait une précision non prise en charge.

Chaque dimension contient un histogramme exact unidimensionnel de la population
éligible. Le volume utilise les bandes de taille du recensement ; les six autres
utilisent ses bandes de compte. Les deux ajoutent `not_applicable` et `unknown`,
avec `eligible = assessed + not_applicable + unknown`. Il n'existe ni composite
par objet ni tableau croisé par type, fonction ou schéma. Les objets générés par
le moteur et secondaires, lorsqu'ils sont identifiés positivement, restent dans
l'inventaire mais sortent de l'évaluation ; les objets temporaires et ceux dont
les indicateurs manquent restent éligibles. Les plugins installés sur le site, assemblies CLR,
Java et bibliothèques restent du vrai travail de migration même si leur corps
est illisible ; seul un indicateur moteur explicite les exclut.

`assessment_population_complete` indique si tous les objets éligibles sont
connus. Il est indépendant de `inventory_complete` et son omission signifie
faux. Une population incomplète force une bande globale `unknown`, sauf si la
borne inférieure connue est déjà `very-high`.

La couverture est enregistrée par dimension. `not_applicable` est une
évaluation achevée. Partiellement évalué signifie qu'une dimension applicable
est connue et qu'une autre est inconnue ; non évalué signifie qu'aucune
dimension applicable n'est connue. Une preuve inconnue n'est jamais une faible
complexité. Une dimension partielle vaut `unknown`, sauf si sa borne inférieure
est déjà `very-high`; la bande globale n'est émise que lorsque ses bornes
coïncident. Le mode `graph` n'émet aucun verdict global pour une population non
vide, car les définitions ne sont pas lues. Une population vide et complète est
`not-applicable`.

Les objets enveloppés contribuent au histogramme de l'opacité dans le compartiment `unknown`, même lorsqu'aucun recensement partiel de la langue ne peut être produit. Lisez la bande d'opacité avec sa couverture, de sorte qu'une petite bande observée ne soit pas lue sans sa population inconnue. Si l'évaluation elle-même échoue, l'inventaire complet est conservé avec un agrégat canonique "tout inconnu". Les définitions enveloppées ou retenues, les graphes incomplets, les preuves de conformité incomplètes, les limites de portée sélectionnées et les langues ou dialectes non pris en charge restent des limitations explicites. Les assertions de limitation sont dérivées de l'artefact et des preuves de recensement lorsque cela est possible. `unsupported-dialect` reste distinct car le recensement peut nommer un dialecte et signaler `unavailable`, mais n'a pas de statut `unsupported` ; cela signifie que la définition a été lue, mais que l'analyseur nommé ne prend pas en charge ce dialecte, et non que la source a été retenue ou enveloppée.

L'enregistrement contient la version unique de l'analyseur et des ensembles triés d'intervalles d'analyse, de dialectes et de profils de grammaire présents dans le recensement éligible. Deux captures ne sont comparables que si ces ensembles, le contrat, l'évaluateur, la portée et la politique de population correspondent tous. Un ensemble conserve la complexité pour chaque source enfant et ne l'agrège jamais entre les moteurs ou les analyseurs.

La version du contrat de complexité et de l'outil d'évaluation sont indépendantes. Pour les champs et les invariants exacts, consultez la [Référence de format](FORMAT.md).

## Couverture par moteur

Le collecteur actuel modélise les familles suivantes :

| Moteur | Familles d'objets modélisées |
|---|---|
| PostgreSQL | vues, vues matérialisées, séquences, routines, agrégats, types enum/domain/composite/range, déclencheurs, valeurs par défaut, contrôles, politiques, règles, déclencheurs d'événements, extensions, tables/serveurs étrangers, publications, abonnements, espaces de tables et fonctions natives |
| MySQL | vues, fonctions et procédures stockées, déclencheurs, événements planifiés, dépendances de vues, tables FEDERATED et enregistrements UDF chargeables |
| SQL Server | vues, procédures stockées, fonctions scalaires/tabulaires, modules CLR, déclencheurs, valeurs par défaut, contrôles, règles, synonymes, séquences, types utilisateur, assemblies CLR, objets de données externes, catalogues de texte intégral, objets de partitionnement, groupes de fichiers non PRIMARY, certificats, clés, informations d'identification limitées à la base, serveurs liés et tâches SQL Server Agent |

Chaque Blueprint énumère les familles connues non modélisées. Un compteur nul ne
prouve pas l'absence tant que `visibility`, les indicateurs d'exhaustivité et la
liste des familles non inventoriées ne le permettent pas.

## Preuves d'exigences

Les exigences relatives aux artefacts sont des informations spécifiques au moteur, provenant de colonnes de catalogue définies ou de vérifications de syntaxe spécifiques au moteur. Une analyse lexicale générique ne crée pas d'exigences spécifiques au moteur. Lorsqu'une fonctionnalité du langage analysée reflète la même information, l'exigence a la priorité et la fonctionnalité reste une observation lexicale. L'absence d'une exigence ne prouve pas que tous les jetons d'exigence ont été vérifiés.

Chaque objet v7 graph/analyzed enregistre `requirement_status` comme `complete`, `partial`, `unavailable` ou `not_applicable`. Seul `complete` rend une liste vide une preuve de l'absence de exigences pour cet objet. `partial` enregistre qu'un certain fait ou producteur a réussi sans couverture exhaustive ; `unavailable` enregistre qu'aucun producteur n'a établi une couverture utilisable et ne peut donc accompagner de preuves d'exigences connues ou de prérequis externes. De telles preuves nécessitent `partial`. Les deux contribuent à des observations de complexité dérivées d'exigences, tout en laissant les objets complets et non affectés évaluables. `not_applicable` interdit les enregistrements d'exigences et de prérequis externes. La valeur `requirements_complete` au niveau de l'inventaire est vraie uniquement après les vérifications de la version et de l'édition du moteur, une population d'évaluation complète, et lorsque chaque objet émis est complet ou non applicable. PostgreSQL, MySQL et SQL Server définissent l'agrégat uniquement après que chaque catalogue d'artefacts applicable a été tenté et que la population sélectionnée est prouvée complète. Un catalogue refusé ou illisible, ou une limite de sélection dont la population ne peut être prouvée, maintient l'agrégat à faux sans effacer les preuves complètes par objet des catalogues qui ont été lus. Les exigences des artefacts ne sont pas signalées pour Oracle.

## Prérequis externes

Les objets dépendant de plus qu'un DDL de table portable portent une classe
anonyme de prérequis externe :

| Classe | Éléments à résoudre par l'opérateur |
|---|---|
| `postgresql_extension` | Paquet d'extension compatible et version cible |
| `postgresql_native_function` | Bibliothèque native et compatibilité ABI |
| `mysql_loadable_udf` | Binaire UDF chargeable et hypothèses ABI du serveur source |
| `sqlserver_clr_assembly` | Activation CLR, assembly, environnement d'exécution et politique de confiance |
| `foreign_endpoint` | Réseau, fournisseur, base distante et authentification |
| `replication_topology` | Topologie publication/abonnement et politique cible |
| `physical_storage` | Conception des groupes de fichiers ou du placement physique |
| `server_feature` | Disponibilité d'une fonction serveur ou de service géré |
| `certificate_material` | Émission ou import de certificat selon la politique cible |
| `encryption_or_credential_material` | Clés, informations d'identification, magasin externe et gestion des secrets |
| `sqlserver_agent` | Disponibilité de l'agent, environnement et gouvernance des tâches |

Le Blueprint indique si un binaire, un secret ou un point de terminaison est requis
mais non capturé. Les objets externes doivent devenir des tâches explicites de
migration, jamais des omissions au mieux.

## Recensement des caractéristiques du langage de programmation

`analyzed` détail ajoute `dbwarp-language-feature-census/v1` blocs. Le schéma v7 émet `lexical-v2`, qui analyse uniquement le corps exécutable ou déclaratif et enregistre `analysis_span = "executable-body"`. Il exclut l'enveloppe de création externe, l'identité, la signature, la déclaration de retour et les options du module. Si le corps ne peut pas être isolé en toute sécurité, le collecteur enregistre une étendue inconnue et des preuves indisponibles ; les objets sans dimension de définition utilisent `not-applicable`. Une étendue omise est interprétée comme inconnue plutôt que d'être déduite de l'environnement. L'analyseur signale `status = "partial"` pour les définitions prises en charge car il ne s'agit pas d'un analyseur, d'un compilateur, d'un lieur sémantique ou d'une garantie de succès de la traduction. Les preuves de définition manquantes ou non prises en charge sont `unavailable`, tandis qu'une analyse prouvée comme inapplicable est `not_applicable`.

Il enregistre des classes bornées de taille, nombre d'instructions et de jetons,
imbrication, complexité cyclomatique et régions opaques/dynamiques. Un
vocabulaire fermé décrit flux de contrôle, jointures, sous-requêtes, CTE,
agrégats, fenêtres, DML, DDL, objets temporaires, SQL dynamique, JSON, XML,
spatial, vecteur, erreurs levées, contrôle des transactions, ref cursors, types
ancrés, intervalles, fuseaux horaires, Boolean, LOB et modes de sécurité. Le contexte moteur comprend le profil de
grammaire normalisé, les modes SQL MySQL et, pour SQL Server, la compatibilité,
`ANSI_NULLS` et `QUOTED_IDENTIFIER`.

L'analyseur lexical supprime les commentaires, les littéraux entre guillemets et les identificateurs entre guillemets avant de procéder au comptage. Il possède des règles de contexte pour les déclarations d'événements de déclenchement, PostgreSQL `EXECUTE FUNCTION` et les options de module SQL Server. Néanmoins, tous les résultats restent des éléments de preuve de planification approximatifs. L'élément PL/SQL encapsulé est refusé ; les octets obscurcis ne deviennent jamais des mesures corporelles plausibles.

## Processus de revue recommandé

1. Exécuter le niveau `summary` par défaut avec une revue des catalogues
   d'artefacts. Si la politique n'autorise que les catalogues de tables,
   utiliser plutôt `--artifact-detail none` ; v7 enregistre explicitement cette
   décision au lieu d'omettre l'état de l'inventaire.
2. Examiner les compteurs, classes externes, visibilité, catalogues illisibles et familles non modélisées.
3. Approuver `graph` uniquement si la topologie anonyme est acceptable.
4. Approuver `analyzed` uniquement si la lecture transitoire des définitions est acceptable.
5. Conserver le journal d'audit localement comme preuve à accès contrôlé. Ne le partager que si un destinataire nommé a besoin des détails sur le point de terminaison, l'identité, les chemins et les dégradations via un canal sécurisé approuvé.
6. Ne présumez pas qu'un objet inventorié peut être recréé ou traduit automatiquement ; confirmez-le auprès de DBWarp.

Pour connaître les champs sérialisés exacts, consultez la [référence du format](FORMAT.md). Pour les lectures, écritures, avertissements et assertions de confiance à l'exécution, consultez la [référence d'audit](AUDIT.md).
