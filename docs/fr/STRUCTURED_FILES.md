# Sources Blueprint depuis des fichiers structurés

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../STRUCTURED_FILES.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../STRUCTURED_FILES.md) | [Deutsch](../de/STRUCTURED_FILES.md) | **Français** | [Español](../es/STRUCTURED_FILES.md) | [Polski](../pl/STRUCTURED_FILES.md) | [日本語](../ja/STRUCTURED_FILES.md) | [简体中文](../zh/STRUCTURED_FILES.md)

`dbwarp-blueprint` peut créer un Blueprint TOML borné et anonymisé à partir
d'entrées Parquet et Avro locales lorsque la source est déjà un fichier plutôt
qu'une base de données active.

Il s'agit d'un mode hors ligne :

- aucune connexion à une base de données ;
- aucune information d'identification ;
- aucune télémétrie ;
- aucune valeur de ligne écrite dans la sortie ;
- les identifiants de table et de colonne sont uniquement `table-NNN` et `col-N` ;
- l'audit enregistre les chemins locaux d'entrée et de sortie, le hachage de la
  sortie et les preuves opérationnelles normales comme le mode, le temps, le
  travail d'échantillonnage et les avertissements ; aucun point de terminaison
  de base de données n'est enregistré dans ce mode.

## Parquet

```bash
dbwarp-blueprint \
  --from-parquet /data/orders.parquet \
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
  --from-avro /data/events.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Les conteneurs d'objets Avro n'exposent pas un nombre de lignes dans un pied de page comme Parquet. Le mode Avro parcourt donc le conteneur une fois pour compter les enregistrements, calculer les `table_bytes` logiques et observer `len_avg`, `len_p95` et `null_fraction` par colonne. Le schéma d'écriture fournit les métadonnées de type logique. `storage_bytes` et `ratio_storage` décrivent le conteneur Avro, et non une estimation de transfert DBWarp.

## Fidélité des types logiques

La capture de fichiers structurés conserve les métadonnées logiques limitées nécessaires pour le dimensionnement : nombres décimaux precision/scale, familles de dates et d'heures, précision des horodatages et sémantique UTC/local, UUID, largeur binaire de taille fixe, chaînes UTF-8 et octets bruts. Les champs contenant uniquement des valeurs nulles restent `type = "null"` au lieu de devenir du texte synthétique.

Les structures imbriquées Parquet et les tableaux, les tableaux associatifs, les enregistrements ou les unions multi-types Avro ne peuvent pas être représentés comme un scalaire SQL exact unique. Le Blueprint enregistre un type `json` normalisé, ainsi que `source_semantics` tels que `"repeated-leaf"`, `"nested-json"` ou `"multi-type-union"`. Ces colonnes sont dimensionnées en JSON ; le schéma imbriqué n'est pas reproduit exactement.

Les noms de fichiers source, chemins Parquet, noms de champs Avro et libellés
`logical_table` d'un lot ne sont pas écrits comme identifiants Blueprint. Un jeu
multifichier émet des identifiants `table-NNN` protégés par une clé secrète, agrège les octets
d'objet, partitions, groupes de lignes, codecs, largeurs, taux de valeurs nulles
et provenances de compression compatibles, puis rejette les fichiers dont les
contrats logiques de colonnes diffèrent.

## Échantillonnage de compression après décodage

Le mode fichier structuré prend en charge un échantillonnage facultatif de la compression après décodage :

```bash
dbwarp-blueprint \
  --from-parquet /data/orders.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Les mêmes options fonctionnent avec `--from-avro`.

Lorsque cette fonction est activée, `dbwarp-blueprint` :

- décode jusqu'à `--sample-rows` enregistrements du fichier ;
- encode les valeurs échantillonnées avec la même représentation transitoire
  `blueprint-compression-probe-v2` que la capture Blueprint depuis une base de
  données active ;
- émet des synthèses de compression zstd-3 au niveau de la table et de chaque colonne ;
- enregistre `sample_encoding = "blueprint-compression-probe-v2"` dans le TOML généré ;
- conserve les octets échantillonnés uniquement en mémoire et n'écrit jamais les valeurs de lignes sur disque.

`--measure-compression` nécessite `--yes` car il lit des valeurs de données décodées. Il conserve les mesures relatives à la compression agrégée, à la densité des valeurs nulles, à cardinality/frequency, à la longueur et au style, et non les valeurs échantillonnées.

L'échantillonneur actuel utilise un échantillon déterministe des N premiers éléments. C'est reproductible et peu coûteux, mais il peut être biaisé si un fichier est trié ou regroupé. Pour les estimations importantes, il est préférable d'utiliser un fichier représentatif ou de générer plusieurs fichiers Blueprint à partir de différentes partitions.

## Périmètre

Le mode Blueprint à partir de fichiers structurés est utile pour :

- dimensionner une importation Parquet/Avro avant une exécution DBWarp ;
- planification d'un transfert de données vers une base de données Parquet/Avro.

Il ne remplace pas la capture Blueprint depuis une base de données active lorsque la véritable source est une base de données prise en charge, c’est-à-dire PostgreSQL, MySQL ou SQL Server. Le catalogue d'une base de données contient des informations sur les index, les clés, les clés étrangères, la fraîcheur des statistiques et l'organisation propre au moteur qui ne figurent pas dans les métadonnées génériques d'un fichier.
