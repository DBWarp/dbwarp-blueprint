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
- aucun privilège DDL ;
- aucun rôle de superutilisateur/administrateur ;
- accès en lecture limité à la base de données évaluée ;
- mot de passe ou jeton fourni par fichier ou invite, et non intégré à l'URI.

Les autorisations exactes varient selon le moteur et la politique du client. Si le compte ne peut pas lire certaines vues du catalogue ou échantillonner certaines tables, l'outil doit échouer clairement ou produire un Blueprint réduit ; conservez le journal d'audit.

Utilisez les scripts tenant compte des versions et les réserves décrites dans
[`../../sql/grants/README.md`](../../sql/grants/README.md). Après la capture
approuvée, supprimez le compte de collecte dédié à l’aide du script
correspondant sous `sql/revoke/` ; avant l’exécution, vérifiez précisément les
cibles de base de données, de motif d’hôte, de rôle et de connexion.

## Tier 1 : catalogue uniquement

Le Tier 1 est utilisé par défaut lorsque `--measure-compression` est absent.

Il lit :

- la version du moteur ;
- la liste des tables et les entrées d'ordonnancement anonymisées ;
- le nombre approximatif de lignes ;
- la taille des tables et des index ;
- les familles de types des colonnes, leur nullabilité et, lorsqu'elles sont disponibles, les statistiques de longueur arrondies ;
- le type d'index, son unicité et les ordinaux anonymisés des colonnes ;
- la structure du graphe des clés étrangères lorsqu'elle est disponible ;
- une sonde RTT facultative côté client, sauf si `--no-rtt-probe` est défini.

Il ne lit pas les valeurs des lignes.

## Inventaire des artefacts hors tables

Depuis le schéma v4, les Blueprints inventorient les objets hors tables indépendamment de l'échantillonnage des lignes. Par défaut, `--artifact-detail summary` lit les catalogues d'objets mais pas les définitions, et n'émet que des comptages bornés et des classes de prérequis externes.

`--artifact-detail graph --yes` ajoute des identifiants d'objets anonymes et des arêtes de dépendance. `--artifact-detail analyzed --yes` lit aussi transitoirement les définitions disponibles et n'émet que des bandes lexicales bornées de caractéristiques et de complexité. Le texte des définitions, les noms d'objets source, les points de terminaison, les chaînes de fournisseur, les principaux, les secrets, les clés, les certificats, les noms de paquets et les binaires ne sont jamais sérialisés.

Les privilèges de catalogue conditionnent les affirmations d'absence. Examinez `visibility`, `inventory_complete`, `dependencies_complete`, `catalogs_unreadable` et `families_not_inventoried` ; un compte nul n'est pas une preuve si ces champs signalent une lacune. `DBP1410W` indique qu'un catalogue d'artefacts facultatif n'a pas pu être lu.

Une topologie de dépendances anonyme peut néanmoins identifier une application. N'approuvez `graph` ou `analyzed` que si ce risque est acceptable. Consultez [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Tier 2 : mesure de la compression

Le Tier 2 n'est activé que par la paire explicite :

```bash
--measure-compression --yes
```

Le Tier 2 lit en outre en mémoire du processus des échantillons de lignes de taille limitée. Les octets échantillonnés sont encodés dans un tampon interne de trames de lignes, compressés localement avec zstd au niveau 3, résumés sous forme de ratios arrondis, puis supprimés.

Les octets échantillonnés ne sont :

- ni écrits dans `blueprint.toml` ;
- ni écrits dans le journal d'audit ;
- ni écrits dans des fichiers temporaires ;
- ni envoyés sur un réseau autre que celui de la connexion à la base de données ;
- ni conservés après la synthèse de l'échantillon.

Le Tier 2 est utile, car les performances de DBWarp et le coût de sortie réseau dépendent des octets compressés, et non des octets bruts des tables.

## Sonde RTT

Par défaut, l'outil exécute cinq requêtes `SELECT 1` après l'établissement de la connexion. Cela produit un bloc `[network]` contenant `connect_total_ms`, `query_rtt_ms_p50` et `query_rtt_ms_p95`.

La sonde aide les opérateurs à comprendre où l'outil Blueprint a été exécuté par rapport à la base de données source. Elle ne mesure pas le RTT WAN de la migration.

Désactivez-la avec :

```bash
--no-rtt-probe
```

## Fichiers lus

À l'exécution, l'outil lit uniquement les fichiers explicitement désignés sur la ligne de commande, comme les fichiers de mot de passe, d'utilisateur, de CA/certificat/clé TLS, de jeton Entra ou un fichier d'entrée `--from-toml`.

Il ne lit volontairement pas les emplacements implicites courants d'informations d'identification tels que `~/.pgpass`, `~/.my.cnf`, les fichiers d'informations d'identification cloud, les clés SSH, l'historique du shell ou les variables d'environnement de mot de passe par défaut.

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

- l'en-tête est l'en-tête fixe `dbwarp-blueprint v6` ;
- les identifiants de table ressemblent à `table-001` ;
- les identifiants de colonne ressemblent à `col-1` ;
- les identifiants de schéma ressemblent à `schema-A` ;
- aucun nom réel de table, colonne, index, schéma ou utilisateur n'est présent ;
- aucun nom d'objet hors table, texte de définition, chaîne de point de terminaison, information d'identification, clé/certificat, nom de paquet ou binaire n'est présent ;
- aucune valeur de ligne n'est présente ;
- les valeurs numériques sont arrondies conformément à [`../FORMAT.md`](FORMAT.md) ;
- les sections de compression facultatives ne contiennent que des ratios et des métadonnées d'échantillon.
- les champs de complétude des artefacts déclarent la visibilité filtrée, les catalogues illisibles et les familles connues non modélisées.

La sortie MySQL équilibrée par défaut contient les capacités déclarées et les
longueurs de préfixe d'index exactes, ainsi que les échantillons moyen/p95
arrondis de manière relative. Vérifiez explicitement les trois marqueurs de
fidélité. Si `--length-fidelity exact --yes` a été utilisé, approuvez également
les statistiques échantillonnées exactes. Les valeurs de ligne et les noms
réels d'objets doivent toujours être absents. Des marqueurs de fidélité absents
indiquent des métadonnées historiques/inconnues et ne doivent pas être
considérés comme prêts pour un benchmark.

Le marqueur n'affirme pas que l'échantillonnage a couvert chaque table. Une
transmission destinée à un benchmark doit également indiquer zéro colonne
indexée de largeur variable non échantillonnée dans le manifeste de
l'estimateur ; augmentez `--max-wall-secs` et recommencez la capture si cette
condition échoue.

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
