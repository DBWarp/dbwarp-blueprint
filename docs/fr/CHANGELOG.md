# Historique des modifications

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../CHANGELOG.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../CHANGELOG.md) | [Deutsch](../de/CHANGELOG.md) | **Français** | [Español](../es/CHANGELOG.md) | [Polski](../pl/CHANGELOG.md) | [日本語](../ja/CHANGELOG.md) | [简体中文](../zh/CHANGELOG.md)

Les numéros de version identifient le collecteur. La version du schéma
Blueprint et l’encodage des échantillons de compression sont des contrats de
compatibilité distincts ; consultez [FORMAT.md](FORMAT.md) et la
[mesure de la compression](COMPRESSION_MEASUREMENT.md).

## 1.6.0

### Schéma Blueprint v7

- Émet le schéma v7 avec une classification explicite des tables,
  l’organisation du stockage, le partitionnement, l’état des segments, les
  positions des colonnes et des sémantiques plus riches pour les types, la
  nullabilité, les valeurs générées et les identités.
- Ajoute, par table et pour l’ensemble de la capture, des preuves relatives à
  la structure, au nombre de lignes, aux octets alloués, aux statistiques et
  aux observations de l’environnement source. Les preuves absentes, refusées,
  malformées ou limitées par la sélection restent explicites au lieu d’être
  représentées par un zéro mesuré ou un inventaire complet.
- Ajoute à l’inventaire des artefacts un état des exigences par objet fondé sur
  le catalogue et des preuves de complexité bornées. L’exhaustivité globale
  reste fausse lorsqu’un catalogue requis ou la population d’évaluation
  sélectionnée ne peut pas être prouvé complet.
- Conserver la compatibilité avec les versions de schéma de la v1 à la v6. Les versions antérieures peuvent ne pas pouvoir lire les fichiers de schéma v7.
### Fidélité et sécurité de la capture

- Distingue les lectures bornées complètes, les échantillons partiels et les
  estimations de catalogue pour PostgreSQL, MySQL et SQL Server, y compris les
  limites liées à la sécurité des lignes et à l’héritage, les fenêtres de
  plages MySQL adaptatives et les tables SQL Server optimisées en mémoire.
- Renforce la cohérence de la cardinalité, des comptes NULL, des partitions,
  des relations et des agrégats afin qu’une preuve arrondie ou incomplète ne
  devienne pas une affirmation exacte.
- Signale la couverture des tables externes SQL Server comme une limitation
  explicite lorsque PolyBase n’est pas installé.
- Maintient la capture dans les limites de lignes, d’octets, de valeurs et de
  délai. Toute dégradation non fatale reste visible dans le Blueprint et
  l’audit avec un code de message stable.

### Limite de portée Oracle

- Cette version ajoute la capture de catalogue Oracle Basic pour Oracle 12c,
  19c, 21c et 23ai/26ai en version préliminaire : cette fonctionnalité est
  activée par une confirmation, ne concerne que le catalogue et ne lit aucune
  ligne de table. Consultez `sql/grants/ORACLE_PREVIEW.md` pour connaître les
  limitations.

### Artefacts de publication et authentification

- Les archives de publication Linux incluent l'authentification Kerberos/GSSAPI
  de SQL Server. Elles ne chargent l'environnement d'exécution Kerberos de la
  plateforme que lorsque l'authentification intégrée est sélectionnée. Le
  collecteur démarre donc sans bibliothèques Kerberos ; une absence
  d'environnement d'exécution n'est signalée avec `DBP1604E` que pour
  l'authentification intégrée. Les binaires de publication Windows continuent
  d'inclure l'authentification SSPI de SQL Server.
- Les deux modes intégrés utilisent les informations d'identification du
  système d'exploitation et refusent la connexion lorsque le principal du
  serveur ne correspond pas à celui attendu.

### Exploitation et compatibilité

- Les scripts d'autorisation Enhanced pour SQL Server ajoutent une autorisation
  au niveau du serveur dans un lot distinct : `VIEW SERVER STATE` pour SQL
  Server 2019 et `VIEW SERVER PERFORMANCE STATE` pour 2022 et 2025. Cela permet
  à une capture Enhanced avec `--artifact-detail graph` ou `analyzed` de
  signaler des plages approximatives de CPU et de mémoire pour un serveur
  autogéré. Basic et Standard ne l'accordent pas et signalent ces plages comme
  inconnues ; un DBA peut supprimer le lot pour conserver Enhanced sans cette
  autorisation.
- Met à jour la pile de dépendances de SQL Server, PostgreSQL, Parquet et de
  l'authentification Windows vers des versions qui corrigent des avis de
  sécurité publiés. Le pilote SQL Server passe à la version 0.13 ; `--tls-ca`
  conserve sa signification restrictive et ne fait confiance qu'à l'autorité
  de certification fournie. `--max-wall-secs` reste le délai unique pour
  l'ensemble de la capture.
- SQL Server ne lit la capacité du système d'exploitation que lorsque
  l'analyse des objets autres que les tables est demandée
  (`--artifact-detail graph` ou `analyzed`). Les autres captures enregistrent
  les plages de capacité comme non demandées plutôt que comme une lecture
  échouée.
- La documentation anglaise reste la référence. Le Markdown traduit est
supplémentaire et comporte sa propre mention de traduction.

### Présentation

- Ajoute à la présentation une diapositive sur les objets non tabulaires et une
  diapositive distincte sur la complexité des artefacts. La diapositive de
  complexité n'apparaît que si la complexité a été capturée et affiche la
  couverture à côté de chaque plage, de sorte que des preuves incomplètes ne
  soient jamais présentées comme une faible complexité.

## 1.5.1

### Capture et fidélité

- Maintien du schéma Blueprint v6 et distinction entre estimations du
  catalogue, observations échantillonnées et preuves de fraîcheur des
  statistiques indisponibles.
- Amélioration de la mesure des charges utiles binaires, du profilage des
des tests de compression limités. Les ratios provenant de différentes valeurs `sample_encoding` ne sont pas interchangeables ; vérifiez l'encodage avant de comparer les ratios.
- Nouvelles tentatives adaptatives d’échantillonnage MySQL et SQL Server
  bornées, y compris pour les valeurs surdimensionnées et l’expansion liée au
  jeu de caractères. Le biais de préfixe restant est consigné tout en
  conservant les métadonnées de longueur d’origine des valeurs échantillonnées.
- Correction de la détection de troncature MySQL lorsque le jeu de caractères
  de la connexion modifie la longueur en octets renvoyée.

### Exploitation et revue

- Clarification de la configuration des comptes dédiés à privilèges minimaux,
vérification de la construction et des versions de base de données prises en charge.
- Actualisation des documents traduits automatiquement et des formulations à
  l’exécution ; l’anglais reste la référence et les traductions restent
  complémentaires.
- Ajout de l’historique des versions et des consignes d’assistance et de
  contribution aux distributions de sources et de binaires.
### Compatibilité.

Les Blueprints existants restent lisibles par le nouveau collecteur. L'inverse n'est pas garanti : la version 1.5.0 ne prend pas en charge le nouveau champ optionnel `sample_layout`, et les versions antérieures peuvent ne pas prendre en charge les nouveaux encodages d'échantillons de compression. Utilisez la même version pour créer un fichier et pour générer une présentation à partir de celui-ci.

Fixez précisément l'artefact de version et la somme de contrôle. Les résultats d'une version ne sont pas garantis pour correspondre à ceux d'une autre.

## 1.5.0

La version précédente fournit la capture en schéma v6 pour PostgreSQL, MySQL et
SQL Server, l’inspection de fichiers structurés, la sortie locale de Blueprints
et de présentations, ainsi que des scripts d’octroi de droits tenant compte
des versions. Consultez le
[tag de version](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0)
pour les sources et artefacts exacts.

Pour signaler un problème, consultez [SUPPORT.md](SUPPORT.md). Pour les
consignes de contribution, consultez [CONTRIBUTING.md](CONTRIBUTING.md).
