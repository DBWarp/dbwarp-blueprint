# Sources Blueprint depuis des fichiers structurés

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../STRUCTURED_FILES.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../STRUCTURED_FILES.md) | [Deutsch](../de/STRUCTURED_FILES.md) | **Français** | [Español](../es/STRUCTURED_FILES.md) | [Polski](../pl/STRUCTURED_FILES.md) | [日本語](../ja/STRUCTURED_FILES.md) | [简体中文](../zh/STRUCTURED_FILES.md)

`dbwarp-blueprint` peut créer un Blueprint TOML assaini à partir d'entrées Parquet et Avro locales lorsque la source est déjà un fichier plutôt qu'une base de données active.

Il s'agit d'un mode hors ligne :

- aucune connexion à une base de données ;
- aucune information d'identification ;
- aucune télémétrie ;
- aucune valeur de ligne écrite dans la sortie ;
- les identifiants de table et de colonne sont uniquement `table-NNN` et `col-N` ;
- l'audit enregistre uniquement les chemins des fichiers locaux d'entrée et de sortie ainsi que le hachage de la sortie.

## Parquet

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

Le mode Parquet lit le pied de page et les métadonnées des groupes de lignes. Il déduit :

- le nombre de lignes à partir des métadonnées du fichier ;
- les étiquettes de type des colonnes à partir des types physiques/logiques Parquet ;
- la possibilité de valeurs nulles à partir des niveaux de définition ;
- les fractions de valeurs nulles observées lorsque des statistiques de colonne complètes sont disponibles ;
- la largeur moyenne encodée approximative et le ratio de stockage source par colonne à partir des métadonnées des segments de colonne ;
- les octets de l'objet source, le nombre de groupes de lignes, le nombre de partitions et la provenance du codec.

La capture Parquet limitée aux métadonnées n'invente pas une largeur p95
décodée. L'échantillonnage décodé facultatif remplace les indications de largeur
encodée par des observations décodées de `len_avg`, `len_p95`, `null_fraction`
et des `table_bytes` logiques.

Sans échantillonnage décodé, Parquet utilise les octets non compressés des
segments de colonne comme estimation logique `table_bytes`. Le
`ratio_storage` de table compare cette valeur à la taille réelle de l'objet ;
`ratio_storage` d'une colonne compare les octets non compressés et compressés
du segment.
Ce sont des signaux de planification de fichier, pas de compression du transport
DBWarp, et ils ne sont jamais émis comme `ratio_zstd_3`.

## Avro

```bash
dbwarp-blueprint \
  --from-avro /data/customer-sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Les conteneurs d'objets Avro n'exposent pas un nombre de lignes dans un pied de
page comme Parquet. Le mode Avro parcourt donc le conteneur une fois pour compter
les enregistrements, calculer les `table_bytes` logiques et observer `len_avg`,
`len_p95` et `null_fraction` par colonne. Le schéma d'écriture fournit les
métadonnées de type logique. `storage_bytes` et `ratio_storage` décrivent le
conteneur Avro, et non une estimation de transfert DBWarp. Cette méthode
convient à la planification de l'estimateur et des fixtures synthétiques.

## Fidélité des types logiques

La capture de fichiers structurés conserve les métadonnées logiques bornées
nécessaires à l'estimateur : précision/échelle décimale, familles de dates et
d'heures, précision des horodatages et sémantique UTC/locale, UUID, largeur
binaire fixe, chaînes UTF-8 et octets bruts. Les champs contenant uniquement
des valeurs NULL restent `type = "null"` au lieu de devenir du texte synthétique.

Les feuilles Parquet imbriquées et les tableaux, maps, enregistrements ou unions
multitypes Avro ne peuvent pas être représentés par un seul scalaire SQL exact.
Le Blueprint enregistre un type `json` normalisé et une valeur
`source_semantics` telle que `"repeated-leaf"`, `"nested-json"` ou
`"multi-type-union"`. Les générateurs en aval doivent présenter ces valeurs
comme une charge JSON représentative, sans revendiquer un aller-retour exact du
schéma imbriqué.

Les noms de fichiers source, chemins Parquet, noms de champs Avro et libellés
`logical_table` d'un lot ne sont pas écrits comme identifiants Blueprint. Un jeu
multifichier émet des identifiants `table-NNN` déterministes, agrège les octets
d'objet, partitions, groupes de lignes, codecs, largeurs, taux de valeurs nulles
et provenances de compression compatibles, puis rejette les fichiers dont les
contrats logiques de colonnes diffèrent.

## Échantillonnage de compression après décodage

Le mode fichier structuré prend en charge un échantillonnage facultatif de la compression après décodage :

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Les mêmes options fonctionnent avec `--from-avro`.

Lorsque cette fonction est activée, `dbwarp-blueprint` :

- décode jusqu'à `--sample-rows` enregistrements du fichier ;
- encode les valeurs échantillonnées au moyen de la même trame de lignes `dbwarp-blueprint-rowframe-v1` que la capture Blueprint depuis une base de données active ;
- émet des synthèses de compression zstd-3 au niveau de la table et de chaque colonne ;
- enregistre `sample_encoding = "dbwarp-blueprint-rowframe-v1"` dans le TOML généré ;
- conserve les octets échantillonnés uniquement en mémoire et n'écrit jamais les valeurs de lignes sur disque.

`--measure-compression` exige `--yes`, car cette fonction lit les valeurs client décodées, même si elle ne conserve que des ratios agrégés.

L'échantillonneur actuel utilise un échantillon déterministe constitué des N premiers éléments. Cette méthode est reproductible et peu coûteuse, mais peut être biaisée si un fichier est trié ou regroupé. Pour les estimations à enjeux élevés, privilégiez un fichier représentatif ou générez plusieurs fichiers Blueprint à partir de fragments différents. Une version future pourra ajouter un échantillonnage stratifié par groupe de lignes ou par bloc.

## Périmètre

Le mode Blueprint à partir de fichiers structurés est utile pour :

- dimensionner une importation Parquet/Avro avant une exécution DBWarp ;
- générer une fixture synthétique indépendante du client à partir des métadonnées du fichier ;
- planifier des flux Parquet/Avro -> DBWarp columnar -> base de données cible.

Il ne remplace pas la capture Blueprint depuis une base de données active lorsque la véritable source est une base de données prise en charge, c’est-à-dire PostgreSQL, MySQL ou SQL Server. Le catalogue d'une base de données contient des informations sur les index, les clés, les clés étrangères, la fraîcheur des statistiques et l'organisation propre au moteur qui ne figurent pas dans les métadonnées génériques d'un fichier.
