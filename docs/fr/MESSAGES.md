# Codes de message opérateur

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../MESSAGES.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../MESSAGES.md) | [Deutsch](../de/MESSAGES.md) | **Français** | [Español](../es/MESSAGES.md) | [Polski](../pl/MESSAGES.md) | [日本語](../ja/MESSAGES.md) | [简体中文](../zh/MESSAGES.md)

`dbwarp-blueprint` utilise des identifiants de messages stables pour les erreurs de validation et de flux de travail propres à DBWarp. Chaque message comporte un préfixe de sous-système, un identifiant numérique et un suffixe de gravité, et indique le problème ainsi qu'une action corrective.

## Format

```text
DBPnnnnS message text. Next: corrective action.
```

Champs :

- `DBP` signifie DBWarp Blueprint.
- `nnnn` est un numéro de message stable à quatre chiffres.
- `S` indique la gravité : `E` erreur, `W` avertissement, `I` information.

Le code est stable et indépendant de la langue. Son résumé, sa cause et l'action corrective sont localisés lorsque `--lang` ou la locale du processus sélectionne une langue prise en charge. Les détails dynamiques du système d'exploitation, du pilote de base de données, du chemin d'accès et de la chaîne de causes restent inchangés afin que l'erreur d'origine puisse être recherchée. Le texte du message ne doit pas contenir de secrets ni d'URI de connexion non masqués.

## Plages

| Plage | Domaine |
|---|---|
| `DBP0001E` | Échec encapsulé réellement non classé, accompagné de la chaîne causale |
| `DBP10xxE` | Validation de la commande, de l'entrée de connexion et de la politique de collecte |
| `DBP11xxE` | Validation du manifeste de lot et des entrées source |
| `DBP12xxE` | Sélecteurs de bundle et sélecteurs d'URI Blueprint |
| `DBP13xxE` | Validation hors ligne de TOML, de présentation et de schéma |
| `DBP14xxE/W` | Échecs de capture de base de données active et dégradation non fatale de l'échantillonnage |
| `DBP15xxE/W` | Sortie des fichiers structurés, Blueprints, présentations et audits |
| `DBP16xxE/W` | Politique relative aux informations d'identification, à l'authentification, à TLS et aux fichiers sensibles |
| `DBP17xxE` | Consentement de l'opérateur |
| `DBP18xxE` | Initialisation de l'environnement d'exécution du processus |

## Codes actuels

| Code | Signification |
|---|---|
| `DBP0001E` | Échec non classé ; la chaîne causale suit. |
| `DBP1000E` | `--connect` est absent en dehors des modes hors ligne. |
| `DBP1001E` | Le mot de passe intégré à l'URI est refusé. |
| `DBP1002E` | Le schéma de l'URI `--connect` n'est pas pris en charge. |
| `DBP1003E` | Le remplacement du nom de serveur TLS n'est pas pris en charge. |
| `DBP1004E` | Une option de jeton Azure est utilisée avec un moteur autre que SQL Server. |
| `DBP1005E` | Le mode d’authentification n’est pas disponible pour le moteur sélectionné. |
| `DBP1006E` | L'échantillonnage de fichiers structurés est demandé sans `--yes` explicite. |
| `DBP1007E` | Mode de fidélité de longueur explicite demandé pour un moteur qui ne le prend pas en charge. |
| `DBP1008E` | `--preserve-exact-lengths` est en conflit avec le respect strict de la longueur. |
| `DBP1009E` | La fidélité exacte des longueurs échantillonnées est demandée sans `--yes` explicite. |
| `DBP1010E` | Le catalogue de localisation intégré est incomplet ou incohérent. |
| `DBP1011E` | Les arguments de la ligne de commande ne sont pas valides. |
| `DBP1012E` | Une URI de connexion à une base de données prise en charge est mal formée. |
| `DBP1013E` | `--source-kind` est vide ou non pris en charge. |
| `DBP1014E` | Graphe d'artefacts anonyme ou analyse de définition demandée sans consentement explicite. |
| `DBP1015E` | Options de certificat TLS client utilisées avec SQL Server, dont le pilote ne les implémente pas. |
| `DBP1101E` | Le manifeste de lot ne peut pas être lu. |
| `DBP1102E` | Le manifeste de lot ne peut pas être analysé. |
| `DBP1103E` | Le manifeste de lot ne contient aucune entrée `[[source]]`. |
| `DBP1104E` | Le mode lot nécessite un `--yes` explicite. |
| `DBP1105E` | Une source du lot a échoué. |
| `DBP1106E` | Le type de source du lot n'est pas pris en charge. |
| `DBP1107E` | La source fichier n'a produit aucun fichier d'entrée. |
| `DBP1108E` | Le mode de jeu de données fichier n'est pas pris en charge. |
| `DBP1109E` | L'identifiant de source du lot ne contient aucune lettre ou aucun chiffre ASCII utilisable. |
| `DBP1110E` | La source de base de données contient un nombre incorrect de sources de connexion. |
| `DBP1111E` | La variable `connect_env` est absente ou illisible. |
| `DBP1112E` | `connect_file` est absent ou illisible. |
| `DBP1113E` | La sortie, l'audit, le rapport ou le répertoire du lot n'a pas pu être achevé. |
| `DBP1114E` | Les membres du jeu de données de fichiers structurés sont incompatibles. |
| `DBP1115E` | Toutes les sources batch ont échoué ; seule une sortie de diagnostic a été publiée. |
| `DBP1116E` | Un bundle batch partiel a été publié. |
| `DBP1200E` | Le sélecteur ou la syntaxe `blueprint://` n'est pas valide. |
| `DBP1201E` | Le sélecteur de bundle ne correspond à aucune source. |
| `DBP1202E` | Le sélecteur de bundle correspond à plusieurs sources. |
| `DBP1203E` | Le sélecteur de bundle ne correspond à aucun Blueprint ni à aucune table extractible. |
| `DBP1204E` | L'entrée du bundle n'a pas pu être lue. |
| `DBP1205E` | Le contenu du bundle ou du Blueprint référencé n'est pas valide. |
| `DBP1206E` | La sortie du bundle n'a pas pu être écrite. |
| `DBP1301E` | `--from-toml` est utilisé sans `--deck`. |
| `DBP1302E` | La version du schéma TOML Blueprint n'est pas prise en charge. |
| `DBP1401E` | La limite de capture PostgreSQL a échoué. |
| `DBP1402E` | La limite de capture MySQL a échoué. |
| `DBP1403E` | La limite de capture SQL Server a échoué. |
| `DBP1404W` | Le mode TLS `prefer` de PostgreSQL s'est rabattu sur une connexion en clair en bouclage. |
| `DBP1405W` | La sonde RTT facultative de la base de données n'était pas disponible. |
| `DBP1406W` | Le budget temporel d'échantillonnage Tier 2 a été épuisé. |
| `DBP1407W` | Un échantillon de compression était incomplet ou indisponible ; des lignes exploitables d’un échantillon partiel ont pu être conservées. |
| `DBP1408W` | Un échantillon de style de colonne texte n'était pas disponible. |
| `DBP1409W` | La tâche de connexion asynchrone de PostgreSQL a signalé une erreur. |
| `DBP1410W` | Un catalogue d'artefacts facultatif était indisponible ; la complétude est donc explicitement réduite. |
| `DBP1411W` | Les preuves de topologie sont indisponibles ; le déploiement et le rôle local restent inconnus. |
| `DBP1412W` | Une disposition distribuée ou shardée a été détectée, sans dimensionnement agrégé complet. |
| `DBP1413W` | La couverture des tables, lignes ou octets est incomplète ou inconnue. |
| `DBP1414W` | La relation de source du bundle est inconnue ; le calcul entre sources est dangereux. |
| `DBP1415W` | Les réplicas déclarés divergent ; un représentant déterministe est conservé sans moyenne. |
| `DBP1416W` | Un groupe de shards est incomplet et ne contribue à aucun total agrégé. |
| `DBP1417W` | Les totaux agrégés du bundle ont été supprimés. |
| `DBP1418W` | Une source incluse dans le calcul du bundle présente une couverture incomplète ou inconnue. |
| `DBP1419E` | La capture active a dépassé `--max-wall-secs` ; le client a fermé la connexion et indique la limite propre au moteur côté serveur. |
| `DBP1420E` | Au moins un `--schema` demandé n'était pas visible ; aucun Blueprint de portée ambiguë n'a donc été écrit. |
| `DBP1421W` | Les identités de session SQL Server étaient indisponibles ; la capture a continué sans affirmation d'identité. |
| `DBP1422W` | L’évaluation de la complexité des artefacts a échoué ; l’inventaire a été conservé et les dimensions agrégées concernées sont inconnues. |
| `DBP1423W` | Un catalogue de structure d’index ou de relations était indisponible ; les tables et colonnes principales ont été conservées avec une couverture explicitement incomplète. |
| `DBP1424W` | La visibilité complète du catalogue des politiques de sécurité SQL Server n’a pas pu être prouvée ; l’échantillonnage de niveau 2 a été ignoré pour chaque table concernée. |
| `DBP1425W` | SQL Server a signalé un filtre de sécurité des lignes actif ; l’échantillonnage de niveau 2 a été délibérément ignoré au lieu de mesurer un sous-ensemble filtré. |
| `DBP1426E` | La capture de base Oracle a échoué pendant la configuration, le démarrage de SQL*Plus, la résolution du propriétaire, la capture ou le mappage. |
| `DBP1427W` | La provenance de la version du client Oracle SQL*Plus n’a pas pu être entièrement attestée ; la capture s’est poursuivie avec une limitation explicite. |
| `DBP1428W` | Oracle Basic s'est arrêté avant que toutes les requêtes de catalogue prévues ne soient terminées ; les tables, colonnes, lignes et tailles déjà lues ont été conservées, et le Blueprint est marqué comme incomplet. |
| `DBP1429W` | Oracle Basic a conservé ses données de base concernant les tables, les colonnes, les lignes et la taille, tandis qu'une ou plusieurs requêtes supplémentaires du catalogue n'étaient pas disponibles. |
| `DBP1430W` | Oracle Basic a publié son Blueprint, mais n'a pas pu écrire le fichier de flux hors ligne optionnel. |
| `DBP1501E` | La limite de capture du fichier structuré a échoué. |
| `DBP1502E` | La sortie du Blueprint ou du bundle a échoué. |
| `DBP1503E` | La génération de la présentation PowerPoint a échoué. |
| `DBP1504W` | Le journal d'audit n'a pas pu être écrit. |
| `DBP1505E` | Le flux hors ligne de base Oracle a échoué lors de la validation de ses permissions de fichier, de sa limite de taille, de sa somme de contrôle, de son ensemble de requêtes ou de sa structure de catalogue. |
| `DBP1601E` | L'acquisition des informations d'identification a échoué. |
| `DBP1602E` | La configuration TLS a échoué. |
| `DBP1603E` | L'acquisition du nom d'utilisateur de la base de données a échoué. |
| `DBP1604E` | La configuration de l’authentification de la base de données n’est pas valide. |
| `DBP1605W` | L'application des autorisations de fichiers sensibles n'est pas disponible sur cette plateforme. |
| `DBP1606E` | L'assertion du principal SQL Server authentifié a échoué avant la capture du catalogue. |
| `DBP1607E` | La clé HMAC d’anonymisation n’a pas pu être initialisée en toute sécurité. |
| `DBP1701E` | L'opération a été annulée avant le consentement explicite. |
| `DBP1702E` | La réponse de consentement n'a pas pu être lue depuis l'entrée standard. |
| `DBP1801E` | L'environnement d'exécution asynchrone n'a pas pu être initialisé. |

Chaque langage pris en charge contient tous les résumés, causes et actions de DBP. Le programme vérifie cela au démarrage et échoue avec `DBP1010E` plutôt que de revenir silencieusement à l'anglais.

Les avertissements non fatals d'échantillonnage de base de données sont affichés
avec leur code d'avertissement stable et consignés dans l'audit de l'exécution.
Cela permet de distinguer une capture Tier 2 complète d'une capture réussie mais
partiellement échantillonnée, sans transformer l'échec d'une sonde facultative
en échec total de la collecte.

## Liste de contrôle du support

Lorsque vous demandez de l'aide pour un échec, fournissez :

- la sortie complète du terminal, y compris le code `DBP` ;
- le journal d'audit si `--audit-log` a été utilisé ;
- la ligne de commande expurgée ;
- pour les erreurs de bundle, la sortie de `dbwarp-blueprint --bundle-list ...`.

Ne demandez pas les fichiers de mot de passe, fichiers de jeton, clés privées ou échantillons de lignes brutes de la base de données.
