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

Pour les colonnes text/binary sélectionnées, le niveau 2 peut également échantillonner cette colonne seule. Cela permet d'obtenir un taux de compression par colonne, au lieu de seulement des moyennes au niveau de la table.

Les ratios de tables de bases de données en direct utilisent une séquence neutre de groupes de 1 000 lignes, avec un descripteur par colonne, des longueurs de valeurs fixes et des charges utiles contiguës aux colonnes. Cela mesure la structure pertinente pour la compression sans capturer aucune information de base de données ou de protocole de transfert. Les ratios par colonne conservent `blueprint-compression-probe-v2`, dont les valeurs préfixées par leur longueur restent l'entrée d'entropie la plus spécifique.

Les blocs de tables PostgreSQL utilisent `blueprint-columnar-transfer-probe-v2`, ce qui transmet des groupes de lignes via un contexte zstd de niveau 3 persistant et effectue une vidange après chaque groupe. MySQL et SQL Server utilisent `blueprint-columnar-transfer-probe-v3` : les mêmes octets neutres et un contexte persistant, avec des vidanges supplémentaires aux limites de blocs de 256 Ko. Les échantillons `nvarchar`, `nchar` et `ntext` de SQL Server sont mesurés en tant que distributions d'octets UTF-16LE. Les échantillons `varchar`, `char` et `text` de SQL Server conservent leur largeur d'octet étroite échantillonnée ; le Blueprint enregistre la page de code du catalogue de collation de la source sous forme de `utf-8`, `windows-N` ou `code-page-N`. Le pilote de base de données expose toujours des chaînes décodées à l'échantillonneur, de sorte que l'identité d'octet de la page de code héritée n'est pas revendiquée. La table `ratio_stddev` est mesurée sur les sorties de groupes de lignes externes. Les blocs de projection par colonne restent des mesures d'entropie indépendantes ponctuelles et émettent `0.0`. Les Blueprints des versions antérieures peuvent contenir `blueprint-columnar-transfer-probe-v1` ; les ratios avec des étiquettes différentes ne sont pas comparables.

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
sample_method = "LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"
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

Ces valeurs sont utilisées pour estimer la taille du transfert réseau.

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
statistique aléatoire. Les fenêtres de plage non finales ont une borne supérieure
exclusive. Les zones de clé primaire clairsemées ou asymétriques peuvent donc
sous-remplir une fenêtre, mais aucune fenêtre ne peut relire les lignes de la
suivante. Les autres replis de moteur moins idéaux sont également consignés par
`sampled_with_bias` et `bias_reason`.

Blueprint enregistre la structure d'un échantillon limité séparément des champs textuels correspondants. L'échantillonnage de plages de clés primaires numériques pour MySQL génère `sample_layout = "primary-key-range-windows"` et trie chaque fenêtre en fonction de la clé primaire complète.

Les échantillons biaisés restent utiles, mais ils ont une fiabilité moindre. Le journal d'audit enregistre que l'échantillonnage des lignes a été activé, ainsi que le nombre d'octets de la sonde encodée localement. Les totaux d'octets de la session de base de données sont signalés comme `unknown` lorsque le pilote ne les expose pas.

## Paramètres d'échantillonnage pratiques

Premier passage sûr en production :

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

Des mesures plus précises lorsque une réplique de lecture ou une fenêtre de maintenance est disponible :

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

## Comment les mesures sont interprétées.

Le champ `sample_encoding` fait partie du contrat. Les ratios ne sont comparables que dans une seule balise d'encodage, car différents encodages d'échantillons peuvent produire des ratios de compression différents pour les mêmes données logiques. En particulier, le ratio de transfert colonne par colonne au niveau de la table et les ratios v2 par colonne sont des mesures complémentaires et ne doivent pas être substitués l'un à l'autre.
