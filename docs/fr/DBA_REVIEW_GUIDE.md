# Guide de revue pour les DBA

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../DBA_REVIEW_GUIDE.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../DBA_REVIEW_GUIDE.md) | [Deutsch](../de/DBA_REVIEW_GUIDE.md) | **Français** | [Español](../es/DBA_REVIEW_GUIDE.md) | [Polski](../pl/DBA_REVIEW_GUIDE.md) | [日本語](../ja/DBA_REVIEW_GUIDE.md) | [简体中文](../zh/DBA_REVIEW_GUIDE.md)

Ce guide s'adresse aux DBA et aux responsables de la sécurité qui doivent décider s'ils peuvent exécuter `dbwarp-blueprint` dans un environnement de production ou similaire à la production.

## Modèle d'exécution

`dbwarp-blueprint` est un binaire local en ligne de commande. En mode actif, il ouvre une connexion à la base de données indiquée par l'URI fournie et écrit un fichier TOML local. Il ne contacte ni l'infrastructure DBWarp, ni les API cloud, ni des points de terminaison de télémétrie, ni des serveurs de licences ou de mises à jour.

En mode de présentation `--from-toml`, il ne se connecte à aucune base de données.

## Compte recommandé

Utilisez un compte dédié à faibles privilèges, autorisé à lire les métadonnées du catalogue et, si la compression Tier 2 est activée, à échantillonner des lignes dans les tables utilisateur.

Propriétés recommandées :

- aucun privilège d'écriture ;
- aucun privilège DDL, sauf si la revue approuve explicitement la capture
  MySQL améliorée, dont les privilèges de métadonnées `TRIGGER` et `EVENT`
  permettent des opérations DDL ;
- aucun rôle de superutilisateur/administrateur ;
- accès en lecture limité à la base de données évaluée ;
- mot de passe ou jeton fourni par fichier ou invite, et non intégré à l'URI.

Les permissions exactes varient en fonction du moteur et de votre politique. Si le compte ne peut pas lire certaines vues du catalogue ou échantillonner certaines tables, l'outil échoue clairement ou génère un Blueprint réduit ; conservez le journal d'audit.

Utilisez les scripts tenant compte des versions et les réserves décrites dans
[`../../sql/grants/README.md`](../../sql/grants/README.md). Après la capture
approuvée, supprimez le compte de collecte dédié à l’aide du script
correspondant sous `sql/revoke/` ; avant l’exécution, vérifiez précisément les
cibles de base de données, de motif d’hôte, de rôle et de connexion.

## Tier 1 : métadonnées uniquement (aucun échantillonnage de lignes)

Le Tier 1 est utilisé par défaut lorsque `--measure-compression` est absent.

Il lit :

- la version du moteur ;
- la liste des tables et les entrées d'ordonnancement anonymisées ;
- le nombre approximatif de lignes ;
- la taille des tables et des index ;
- les familles de types des colonnes, leur nullabilité et, lorsqu'elles sont disponibles, les statistiques de longueur arrondies ;
- le type d'index, son unicité et les ordinaux anonymisés des colonnes ;
- la structure du graphe des clés étrangères lorsqu'elle est disponible ;
- les bandes approximatives de capacité de la source renvoyées, dans la mesure du possible, par le point de terminaison de la base ;
- des comptages bornés d'objets hors tables et de prérequis externes provenant
  des catalogues d'objets avec la valeur par défaut
  `--artifact-detail summary` (aucune définition) ;
- sonde RTT facultative, sauf si `--no-rtt-probe` est activé.

Il ne lit pas les valeurs des lignes.

## Environnement source

Le bloc `[source_environment]` du schéma v7 est dérivé uniquement des valeurs
renvoyées par la connexion à la base sélectionnée. Le collecteur n’inspecte
jamais son propre hôte et ne présente pas ce poste de travail comme le serveur
de base de données.

PostgreSQL et MySQL exposent un paramètre de tampon de base de données en dessous des droits minimaux habituels, de sorte que la mémoire constitue une preuve partielle, avec pour base `database-buffer-cache`, et l'utilisation du processeur reste inconnue.

SQL Server requiert uniquement une capacité d'environnement source avec `--artifact-detail graph` ou `analyzed`, les modes de niveau supérieur. Les modes de base et standard n'effectuent pas de requête de capacité du système d'exploitation et enregistrent les plages de capacité comme `not-requested`.

Le script amélioré accorde les `VIEW SERVER STATE` (2019) ou `VIEW SERVER PERFORMANCE STATE` (2022/2025) requis à l'échelle du serveur, dans un lot distinct que l'administrateur de base de données peut supprimer. Si une capture améliorée ne peut pas lire la DMV, la capture continue et enregistre le catalogue comme étant illisible, plutôt que d'utiliser des valeurs locales ou d'inventer des capacités.

Ce chemin de capture ne contacte aucune API de cloud, Kubernetes, hyperviseur
ou système d’exploitation.

## Inventaire des artefacts hors tables

Les blueprints inventorient les objets non tabulaires indépendamment de l'échantillonnage des lignes. Par défaut, `--artifact-detail summary` lit les catalogues d'objets mais pas les définitions, et ne génère que des décomptes limités et des classes de prérequis externes.

`--artifact-detail graph --yes` ajoute des identifiants d'objets anonymes et des arêtes de dépendance. `--artifact-detail analyzed --yes` lit aussi transitoirement les définitions disponibles et n'émet que des bandes lexicales bornées de caractéristiques et de complexité. Le texte des définitions, les noms d'objets source, les points de terminaison, les chaînes de fournisseur, les principaux, les secrets, les clés, les certificats, les noms de paquets et les binaires ne sont jamais sérialisés.

Les privilèges de catalogue conditionnent les affirmations d’absence. Examinez
`visibility`, `inventory_complete`, `dependencies_complete`,
`requirements_complete`, `catalogs_unreadable` et `families_not_inventoried` ;
un compte nul ou une liste d’exigences vide n’est pas une preuve si ces champs
signalent une lacune. Aux niveaux graph/analyzed, examinez aussi le
`requirement_status` de chaque objet : seul `complete` transforme une liste
vide en preuve de zéro exigence pour cet objet. `partial` conserve les faits
connus sans revendiquer une couverture exhaustive ; `unavailable` signifie
qu’aucune couverture exploitable n’a été établie. Dans les deux cas,
l’évaluation du couplage dérivée des exigences de cet objet reste inconnue.
`DBP1410W` indique qu’un catalogue d’artefacts facultatif n’a pas pu être lu.

Une topologie de dépendances anonyme peut néanmoins identifier une application. N'approuvez `graph` ou `analyzed` que si ce risque est acceptable. Consultez [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Tier 2 : mesure de la compression

Le Tier 2 n'est activé que par la paire explicite :

```bash
--measure-compression --yes
```

Le niveau 2 lit également des échantillons de lignes limités en mémoire du processus. Les octets échantillonnés sont encodés dans un tampon en mémoire et utilisés pour dériver des mesures d'agrégation, de densité de valeurs nulles, cardinality/frequency, de longueur et de style, avant que les valeurs et les empreintes digitales temporaires ne soient supprimées.

Les octets échantillonnés ne sont :

- ni écrits dans `blueprint.toml` ;
- ni écrits dans le journal d'audit ;
- ni écrits dans des fichiers temporaires ;
- ni envoyés sur un réseau autre que celui de la connexion à la base de données ;
- ni conservés après la synthèse de l'échantillon.

Le niveau 2 est précieux car le temps de transfert et le coût de sortie dépendent des octets compressés, et non des octets bruts de la table.

## Sonde RTT

Par défaut, l'outil exécute cinq requêtes `SELECT 1` après l'établissement de la connexion. Cela produit un bloc `[network]` contenant `connect_total_ms`, `query_rtt_ms_p50` et `query_rtt_ms_p95`.

La sonde aide les opérateurs à comprendre où l'outil Blueprint a été exécuté par rapport à la base de données source. Elle ne mesure pas le RTT WAN de la migration.

Désactivez-la avec :

```bash
--no-rtt-probe
```

## Fichiers lus

À l'exécution, l'outil lit uniquement les fichiers explicitement sélectionnés
sur la ligne de commande ou référencés par un manifeste de lot ou un bundle
explicitement sélectionné. Il peut s'agir de fichiers de mot de passe,
d'utilisateur ou de clé d'anonymisation, de fichiers de CA/certificat/clé TLS,
de fichiers de jeton Entra, d'entrées de fichiers structurés et d'entrées
Blueprint ou de bundle.

Il ne lit volontairement pas les emplacements implicites courants d'informations d'identification tels que `~/.pgpass`, `~/.my.cnf`, les fichiers d'informations d'identification cloud, les clés SSH, l'historique du shell ou les variables d'environnement de mot de passe par défaut.

Cette déclaration couvre la découverte d'identifiants contrôlée par
l'application. Les bibliothèques de base de données, TLS, DNS et
d'authentification intégrée peuvent consulter les magasins de confiance, la
configuration et les caches d'identifiants du système d'exploitation.
Examinez ou tracez séparément ces dépendances de plateforme lorsque la
politique de l'hôte l'exige.

Consultez [`../AUDIT.md`](AUDIT.md) pour la liste complète.

## Fichiers écrits

L'outil écrit uniquement dans les chemins sélectionnés par le mode actif :

- le TOML Blueprint `--out` en mode de collecte active ;
- `--deck` si demandé ;
- `--audit-log` si demandé ;
- `--out-dir` en mode lot : `bundle.toml`, `blueprints/`, `audits/`, un
  marqueur de propriété et `errors.txt` lorsqu'un échec partiel doit être signalé ;
- le journal d'audit sur stderr à chaque exécution.

Il n'utilise pas de répertoire temporaire implicite du système d'exploitation.
La publication atomique en mode par lots peut créer un répertoire adjacent de
préparation ou de récupération à côté de `--out-dir` ; si une erreur gérée
survient, ce répertoire est supprimé ou le bundle précédent est restauré.

## Liste de contrôle de la revue de sortie

Avant de partager `blueprint.toml`, vérifiez que :

- l'en-tête est l'en-tête fixe `dbwarp-blueprint v7` ;
- les identifiants de table ressemblent à `table-001` ;
- les identifiants de colonne ressemblent à `col-1` ;
- les identifiants de schéma ressemblent à `schema-A` ;
- aucun nom réel de table, colonne, index, schéma ou utilisateur n'est présent ;
- aucun nom d'objet hors table, texte de définition, chaîne de point de terminaison, information d'identification, clé/certificat, nom de paquet ou binaire n'est présent ;
- aucune valeur de ligne n'est présente ;
- les valeurs numériques utilisent la précision exacte ou arrondie documentée
  dans [`../FORMAT.md`](FORMAT.md) ; les champs exacts facultatifs doivent être
  examinés comme plus sensibles ;
- les sections facultatives dérivées d'échantillons contiennent des métadonnées
  agrégées de compression, de densité NULL, de cardinalité/fréquence, de
  longueur, de style et de provenance d'échantillon, jamais les valeurs
  échantillonnées.
- les champs de complétude des artefacts déclarent la visibilité filtrée, les catalogues illisibles et les familles connues non modélisées.

La sortie équilibrée par défaut MySQL contient les capacités déclarées exactes et les longueurs de préfixe d'index, ainsi que des échantillons moyens/p95 relativement arrondis. Examinez attentivement les trois indicateurs de fidélité. Si `--length-fidelity exact --yes` a été utilisé, approuvez également les statistiques échantillonnées exactes. Les valeurs des lignes et les noms réels des objets doivent toujours être absents. Un Blueprint sans indicateurs de fidélité a été produit par une version plus ancienne ; il faut le recréer.

Le marqueur n'indique pas que l'échantillonnage a couvert toutes les tables. Si `DBP1406W` est signalé, augmentez `--max-wall-secs` et réessayez.

## Sécurité opérationnelle

Première exécution recommandée :

```bash
--sample-rows 500 --max-wall-secs 120
```

Exécution de type production recommandée après approbation :

```bash
--sample-rows 1000 --max-wall-secs 300
```

Exécutez l'outil depuis une réplique en lecture si la politique de production interdit l'échantillonnage sur le serveur principal.
