# Mesure de la compression

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../COMPRESSION_MEASUREMENT.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../COMPRESSION_MEASUREMENT.md) | [Deutsch](../de/COMPRESSION_MEASUREMENT.md) | **Français** | [Español](../es/COMPRESSION_MEASUREMENT.md) | [Polski](../pl/COMPRESSION_MEASUREMENT.md) | [日本語](../ja/COMPRESSION_MEASUREMENT.md) | [简体中文](../zh/COMPRESSION_MEASUREMENT.md)

`dbwarp-blueprint` peut mesurer facultativement le degré de compression de données de table représentatives. Cette mesure améliore la précision des estimations DBWarp, car la durée de transfert WAN et le coût de sortie réseau dépendent des octets compressés, et non de la taille brute des tables.

La mesure de la compression est facultative et exige un consentement explicite. Une exécution active interactive peut accepter l'invite de prévol ; une exécution sans surveillance ou sur fichiers structurés utilise :

```bash
--measure-compression --yes
```

Sans ces options, l'outil lit uniquement les métadonnées du catalogue.

## Contenu échantillonné

Pour chaque table utilisateur, l'outil lit en mémoire un nombre borné de lignes, les encode dans un tampon de trames de lignes déterministe, compresse localement ce tampon avec zstd au niveau 3, enregistre des ratios arrondis, puis supprime le tampon.

Pour certaines colonnes de texte ou de données binaires, Tier 2 peut également échantillonner uniquement cette colonne. Cela permet aux outils de planification en aval de reproduire l'entropie par colonne au lieu de reposer uniquement sur des moyennes au niveau de la table.

Chaque mesure est une trame zstd indépendante en une seule passe, avec la taille d'entrée annoncée. La variance des ratios (`ratio_stddev`) est mesurée sur des tronçons de 64 KiB alignés sur les lignes du même tampon, de sorte qu’elle décrit le transfert prédit par l’estimateur plutôt qu’une seule moyenne du tampon entier. La taille d'entrée étant annoncée, zstd choisit des paramètres adaptés à la taille et cohérents avec la façon dont l’estimateur modélise le transfert. Sur de petits échantillons (en dessous d'environ 1 MiB), les ratios peuvent s'écarter sensiblement des captures de versions antérieures mesurées via un contexte de streaming sans taille annoncée ; les ratios des petites tables ne sont pas directement comparables à travers cette frontière. La mesure avec taille annoncée est celle qui correspond au transfert.

Les octets échantillonnés ne sont pas écrits sur disque, ne sont pas inclus dans `blueprint.toml` ni dans le journal d'audit et ne sont envoyés nulle part, sauf depuis le serveur de base de données vers le processus local que vous avez exécuté.

## Parallélisme des workers locaux

L'échantillonnage de la base utilise toujours une seule connexion séquentielle.
Le réglage facultatif `--compression-workers N` ne parallélise que la
compression locale des échantillons en mémoire déjà lus. Il accepte de 1 à 32
workers et utilise 1 par défaut afin de limiter l'impact sur l'hôte source.
Augmentez-le explicitement pour utiliser davantage de CPU locale :

```bash
--measure-compression --yes \
--compression-workers 4
```

Des valeurs supérieures peuvent réduire la durée lorsque zstd est le goulot
d'étranglement, mais augmentent le CPU local et la mémoire de pointe. Elles ne
créent pas de connexions d'échantillonnage concurrentes. Chaque worker possède
ses contextes zstd et la file d'entrée est bornée au nombre de workers. L'ordre
de sortie et les valeurs du Blueprint v6 restent déterministes.

Le collecteur évite les requêtes de lignes et de style uniquement lorsqu'une
valeur de catalogue maintenue par le moteur prouve qu'une table était vide au
moment de la lecture. PostgreSQL exige des statistiques analysées à jour sans
modification ultérieure ; SQL Server utilise son compteur de lignes de
partition. Les estimations de lignes MySQL peuvent indiquer zéro pour une table
non vide : le collecteur ne les utilise donc pas pour ignorer
l'échantillonnage. Cette différence prudente protège la fidélité.

## Contenu du fichier Blueprint

Seuls des nombres récapitulatifs sont émis. Pour les colonnes assimilables à du texte, le passage Tier 2 peut émettre une étiquette de style bornée telle que `json`, `xml`, `natural-text`, `base64`, `hex`, `numeric-text` ou `mixed`.

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
sample_method = "column LIMIT N (engine-specific bounded sample)"
sampled_with_bias = true
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "dbwarp-blueprint-rowframe-v1"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

Ces valeurs aident les outils en aval approuvés à estimer la taille du transfert réseau et à générer des données synthétiques textuelles/binaires offrant une compressibilité similaire.

## Importance de la mesure

Deux bases de données de même taille brute peuvent se comporter très différemment pendant une migration :

- JSON, XML, les codes métier répétés, le texte clairsemé et le texte en langue naturelle se compressent souvent bien.
- Les valeurs chiffrées, les blobs déjà compressés, les jetons aléatoires et les données binaires à forte entropie se compressent mal.
- Les données SQL Server `nvarchar` présentent une distribution d'octets différente de celle du texte UTF-8 et sont encodées en conséquence pour l'échantillonnage.

Une petite mesure locale est généralement plus utile qu'une estimation fondée sur les types de colonnes.

## Biais et transparence

Certains moteurs ne proposent pas un échantillonnage de table parfaitement uniforme. Lorsque l'outil se rabat sur une méthode moins idéale, le fichier Blueprint le signale au moyen de `sampled_with_bias` et `bias_reason`.

Les échantillons biaisés restent utiles, mais les outils en aval doivent leur accorder un niveau de confiance inférieur. L'audit indique que l'échantillonnage était activé et le nombre d'octets row-frame encodés localement. Les octets réseau restent `unknown` si le pilote ne les expose pas.

## Paramètres d'échantillonnage pratiques

Premier passage sûr en production :

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

Meilleure entrée pour l'estimateur lorsqu'une réplique en lecture ou une fenêtre de maintenance est disponible :

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

Les grandes bases de données ne nécessitent pas d'échantillons immenses. L'objectif est d'obtenir un signal de compression stable, et non un profilage exact au niveau des lignes. `--max-wall-secs` est une échéance stricte pour toute la capture active, connexion, catalogues, RTT et échantillonnage compris, et non un nouveau budget par phase.

L’échantillonnage d’une base de données active est aussi soumis à un plafond non
configurable de 16 MiB de charge utile projetée par table. La projection SQL
tronque les cellules de largeur variable sur le serveur et réduit la limite de
lignes des tables exceptionnellement larges avant que le pilote ne reçoive les
données. Les très grandes valeurs LOB ne contribuent donc que par des préfixes
bornés, et non par leur contenu complet. L’audit consigne le plafond actif de
charge utile par table et le nombre exact d’octets de la trame de lignes encodée
localement.

## Utilisation par les consommateurs en aval

Un consommateur en aval doit utiliser les éléments de compression dans l'ordre suivant :

1. blocs de compression par colonne reconnus ;
2. blocs de compression au niveau de la table reconnus ;
3. valeurs par défaut de type/style lorsqu'aucun ratio mesuré n'est disponible.

Le champ `sample_encoding` fait partie du contrat. Les consommateurs ne doivent utiliser que les ratios portant une balise d'encodage reconnue, car des encodages d'échantillons différents peuvent produire des ratios de compression différents pour les mêmes données logiques.
