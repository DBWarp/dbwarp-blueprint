# Mesure de la compression

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../COMPRESSION_MEASUREMENT.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../COMPRESSION_MEASUREMENT.md) | [Deutsch](../de/COMPRESSION_MEASUREMENT.md) | **Français** | [Español](../es/COMPRESSION_MEASUREMENT.md) | [Polski](../pl/COMPRESSION_MEASUREMENT.md) | [日本語](../ja/COMPRESSION_MEASUREMENT.md) | [简体中文](../zh/COMPRESSION_MEASUREMENT.md)

`dbwarp-blueprint` peut mesurer facultativement le degré de compression de données de table représentatives. Cette mesure améliore la précision des estimations DBWarp, car la durée de transfert WAN et le coût de sortie réseau dépendent des octets compressés, et non de la taille brute des tables.

La mesure de la compression est facultative et exige un consentement explicite. Une exécution active interactive peut accepter l'invite de prévol ; une exécution sans surveillance ou sur fichiers structurés utilise :

```bash
--measure-compression --yes
```

Lorsque la mesure de compression est désactivée, la capture sur une base de
données active n’échantillonne pas les valeurs des lignes des tables
utilisateur. Le comportement des fichiers structurés diffère : les
enregistrements Avro doivent toujours être parcourus pour recueillir les
nombres de lignes, les longueurs et les métadonnées NULL ; consultez
[Fichiers structurés](STRUCTURED_FILES.md).

## Contenu échantillonné

Pour chaque table utilisateur admissible dont il n'est pas possible de prouver
sûrement qu'elle est vide, l'outil lit en mémoire un nombre borné de lignes,
les encode dans des tampons de sonde transitoires stables, compresse localement
ces tampons avec zstd au niveau 3 et dérive des mesures agrégées de compression, de
densité NULL, de cardinalité/fréquence, de longueur et de style, avant de
supprimer les valeurs échantillonnées et les empreintes temporaires.

Pour certaines colonnes de texte ou de données binaires, Tier 2 peut également échantillonner uniquement cette colonne. Cela permet aux outils de planification en aval de reproduire l'entropie par colonne au lieu de reposer uniquement sur des moyennes au niveau de la table.

Les ratios de tables de bases actives utilisent une séquence neutre de groupes
bornés de 1 000 lignes, avec un descripteur par colonne, des longueurs de valeur
à largeur fixe et des charges utiles contiguës par colonne. Cela mesure la
structure pertinente pour la compression que partagent les transports de masse,
sans capturer un protocole de base de données ni un format réseau DBWarp. Les
ratios par colonne conservent `blueprint-compression-probe-v2`, dont les valeurs
balisées et préfixées par leur longueur restent l'entrée d'entropie la plus précise.

Les blocs de table PostgreSQL utilisent actuellement
`blueprint-columnar-transfer-probe-v2`, qui transmet les groupes de lignes à un
contexte zstd persistant de niveau 3 et le vide après chaque groupe. MySQL et SQL
Server utilisent `blueprint-columnar-transfer-probe-v3` : les mêmes octets neutres
et le même contexte persistant, avec des vidages supplémentaires aux limites des
blocs de sonde de 256 KiB. Les échantillons SQL Server `nvarchar`, `nchar` et
`ntext` sont mesurés sous forme de distributions d'octets UTF-16LE.
`varchar`, `char` et `text` conservent leur largeur d'octets étroite
échantillonnée ; le Blueprint enregistre la page de codes de la collation source
comme `utf-8`, `windows-N` ou `code-page-N`, afin qu'un consommateur approuvé
puisse choisir un encodeur natif compatible au lieu d'élargir les valeurs. Le
pilote expose encore des chaînes décodées à l'échantillonneur : aucune identité
d'octets n'est donc revendiquée pour les anciennes pages de codes. Le
`ratio_stddev` de table est mesuré entre les sorties des groupes de lignes
externes. Les blocs de projection par colonne restent des mesures d'entropie
indépendantes en une passe et émettent `0.0`. Les anciennes mesures de table
balisées `blueprint-columnar-transfer-probe-v1` utilisaient une seule opération
avec taille annoncée sur les trames jointes ; la version explicite empêche leur
réinterprétation silencieuse selon la politique de streaming actuelle.

Les octets échantillonnés transitent uniquement par la session de base de
données sélectionnée vers le processus local. Ils ne sont ni écrits sur disque,
ni inclus dans `blueprint.toml` ou dans le journal d'audit, ni téléversés, ni
envoyés à l'infrastructure DBWarp.

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
ses contextes zstd et la file d'entrée est bornée au nombre de workers.
Le nombre de workers ne modifie pas les mesures. L'ordre des
libellés anonymes varie intentionnellement avec la nouvelle clé utilisée par
défaut ; ne réutilisez un fichier protégé `--anonymization-key-file` que pour
des comparaisons approuvées entre exécutions.

Le collecteur évite les requêtes de lignes et de style uniquement lorsqu'une
valeur de catalogue maintenue par le moteur prouve qu'une table était vide au
moment de la lecture. PostgreSQL exige des statistiques analysées à jour sans
modification ultérieure ; SQL Server utilise son compteur de lignes de
partition. Les estimations de lignes MySQL peuvent indiquer zéro pour une table
non vide : le collecteur ne les utilise donc pas pour ignorer
l'échantillonnage. Cette différence prudente protège la fidélité.

## Contenu du fichier Blueprint

Seuls des résumés agrégés sont émis. Pour les colonnes assimilables à du texte, le passage Tier 2 peut émettre une étiquette de style bornée telle que `json`, `xml`, `natural-text`, `base64`, `hex`, `numeric-text` ou `mixed`.

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
bias_reason = "unordered_limit_after_empty_TABLESAMPLE"
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "blueprint-compression-probe-v2"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "blueprint-columnar-transfer-probe-v3"
```

Ces valeurs aident les outils en aval approuvés à estimer la taille du transfert réseau et à générer des données synthétiques textuelles/binaires offrant une compressibilité similaire.

## Importance de la mesure

Deux bases de données de même taille brute peuvent se comporter très différemment pendant une migration :

- JSON, XML, les codes métier répétés, le texte clairsemé et le texte en langue naturelle se compressent souvent bien.
- Les valeurs chiffrées, les blobs déjà compressés, les jetons aléatoires et les données binaires à forte entropie se compressent mal.
- Les textes Unicode et étroits de SQL Server ont des distributions d'octets différentes. L'échantillonneur modélise `nvarchar` en UTF-16LE et enregistre la page de codes de la collation nécessaire pour interpréter `varchar`, sans traiter toutes les colonnes de texte comme UTF-8.

Une petite mesure locale est généralement plus utile qu'une estimation fondée sur les types de colonnes.

## Biais et transparence

Certains moteurs ne proposent pas un échantillonnage de table parfaitement
uniforme. MySQL répartit un échantillon borné sur quatre plages de clé primaire
numérique lorsque ce chemin d'accès existe, sinon il se rabat sur `LIMIT N` ; les
deux restent explicitement marqués comme biaisés, car aucun n'est un échantillon
statistique aléatoire. Les autres replis de moteur moins idéaux sont également
consignés par `sampled_with_bias` et `bias_reason`.

Lorsqu'un échantillon borné possède une disposition qui affecte la génération
synthétique, Blueprint l'enregistre séparément de ces champs textuels.
L'échantillonnage MySQL par plages de clé primaire numérique émet
`sample_layout = "primary-key-range-windows"` et ordonne chaque fenêtre selon la
clé primaire complète. Les consommateurs peuvent ainsi préserver la localité
groupée des clés composites sans analyser `sample_method` ni `bias_reason`.

Les échantillons biaisés restent utiles, mais les outils en aval doivent leur accorder un niveau de confiance inférieur. L'audit indique que l'échantillonnage était activé et le nombre d'octets de sonde encodés localement. Les totaux d'octets de la session de base restent `unknown` si le pilote ne les expose pas.

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
initiale est budgétée par type et observe séparément les longueurs originales en
octets. Lorsqu’une valeur projetée a été tronquée, MySQL et SQL Server peuvent
réessayer avec moins de lignes et des limites par colonne ajustées qui
respectent toujours le budget. Les valeurs trop larges pour ce budget restent
des préfixes bornés ; la provenance des mesures de compression et de synthèse
des valeurs consigne cette limite, tandis que les statistiques de longueur
conservent les longueurs d’origine des valeurs échantillonnées signalées par
le serveur, conformément à la politique de fidélité des longueurs sélectionnée.

Ce plafond ne limite ni les octets réseau ni la mémoire du processus.
L’encodage du protocole, les métadonnées de longueur d’origine, les nouvelles
tentatives et les tampons des pilotes ajoutent un surcoût. L’audit consigne le
plafond de charge utile configuré, les requêtes exécutées et le nombre exact
d’octets de sonde encodés localement ; il ne rapporte pas une mesure du trafic
réseau de la base de données.

## Utilisation par les consommateurs en aval

Un consommateur en aval doit utiliser les éléments de compression dans l'ordre suivant :

1. blocs de compression par colonne reconnus ;
2. blocs de compression au niveau de la table reconnus ;
3. valeurs par défaut de type/style lorsqu'aucun ratio mesuré n'est disponible.

Le champ `sample_encoding` fait partie du contrat. Les consommateurs ne doivent
utiliser que les ratios portant une balise d'encodage reconnue, car des encodages
d'échantillons différents peuvent produire des ratios de compression différents
pour les mêmes données logiques. En particulier, le ratio de la sonde de
transfert en colonnes au niveau de la table et les ratios v2 par colonne sont
des mesures complémentaires et ne doivent pas se substituer les uns aux autres.
