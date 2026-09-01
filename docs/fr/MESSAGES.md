# Codes de message opérateur

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../MESSAGES.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../MESSAGES.md) | [Deutsch](../de/MESSAGES.md) | **Français** | [Español](../es/MESSAGES.md) | [Polski](../pl/MESSAGES.md) | [日本語](../ja/MESSAGES.md) | [简体中文](../zh/MESSAGES.md)

`dbwarp-blueprint` utilise des identifiants de message opérateur stables pour les échecs de validation et de flux de travail propres à DBWarp.
Le format s'inspire des messages opérateur de style IBM : un préfixe de sous-système, un identifiant numérique et un suffixe de gravité.
La documentation IBM CICS décrit un identifiant de programme suivi d'un numéro de message à quatre chiffres et d'une lettre de gravité ; IBM MQ utilise de même des champs de composant/préfixe, un identifiant numérique et un code final de type de message. Les recommandations de Microsoft relatives aux messages d'erreur renforcent la règle pratique selon laquelle une erreur doit décrire le problème et fournir une action que l'utilisateur peut entreprendre.

Références :

- Format des messages IBM CICS : https://www.ibm.com/docs/en/cics-pa/5.3.0?topic=messages-message-format
- Présentation des informations de message IBM CICS : https://www.ibm.com/docs/en/cics-ts/6.x?topic=messages-format-cics-message-information
- Format des messages IBM MQ for z/OS : https://www.ibm.com/docs/SSFKSJ_9.2.0/com.ibm.mq.ref.doc/q050270_.htm
- Recommandations de Microsoft relatives aux messages d'erreur : https://learn.microsoft.com/en-us/windows/win32/uxguide/mess-error

## Format

```text
DBPnnnnS message text. Next: corrective action.
```

Champs :

- `DBP` signifie DBWarp Blueprint.
- `nnnn` est un numéro de message stable à quatre chiffres.
- `S` indique la gravité : `E` erreur, `W` avertissement, `I` information.

Le code est stable et indépendant de la langue. Son résumé, sa cause et son
action corrective sont localisés lorsque `--lang` ou les paramètres régionaux
du processus sélectionnent une langue prise en charge. Les détails dynamiques
du système d'exploitation, du pilote de base de données, des chemins et de la
chaîne causale restent inchangés afin que les ingénieurs du support puissent
rechercher l'échec d'origine. Le texte du message ne doit contenir ni secrets
ni URI de connexion non expurgées.

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
| `DBP1007E` | Un mode explicite de fidélité des longueurs est demandé pour un moteur qui n'expose pas encore ce contrat. |
| `DBP1008E` | L'alias historique de longueurs exactes entre en conflit avec la fidélité stricte des longueurs. |
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
| `DBP1402E` | La limite de capture MySQL ou MariaDB a échoué. |
| `DBP1403E` | La limite de capture SQL Server a échoué. |
| `DBP1404W` | Le mode TLS `prefer` de PostgreSQL s'est rabattu sur une connexion en clair en bouclage. |
| `DBP1405W` | La sonde RTT facultative de la base de données n'était pas disponible. |
| `DBP1406W` | Le budget temporel d'échantillonnage Tier 2 a été épuisé. |
| `DBP1407W` | Un échantillon de compression n'était pas disponible. |
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
| `DBP1501E` | La limite de capture du fichier structuré a échoué. |
| `DBP1502E` | La sortie du Blueprint ou du bundle a échoué. |
| `DBP1503E` | La génération de la présentation PowerPoint a échoué. |
| `DBP1504W` | Le journal d'audit n'a pas pu être écrit. |
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

Chaque langue annoncée doit contenir le résumé, la cause et l'action de chaque
code DBP actuel. Le binaire le vérifie au démarrage et échoue avec `DBP1010E`
plutôt que de se rabattre silencieusement sur l'anglais.

Les échecs prévisibles aux limites de décision sont exercés par une matrice CLI
adversariale. Une condition connue doit émettre son code spécifique comme
premier code opérateur et ne doit pas se rabattre sur `DBP0001E`. Le moteur de
rendu analyse aussi toute la chaîne d'erreurs afin qu'un contexte
d'implémentation non codé ne puisse pas masquer une cause interne codée.

Les avertissements non fatals d'échantillonnage de base de données sont affichés
avec leur code d'avertissement stable et consignés dans l'audit de l'exécution.
Cela permet de distinguer une capture Tier 2 complète d'une capture réussie mais
partiellement échantillonnée, sans transformer l'échec d'une sonde facultative
en échec total de la collecte.

## Liste de contrôle du support

Lorsqu'un client signale un échec, demandez :

- la sortie complète du terminal, y compris le code `DBP` ;
- le journal d'audit si `--audit-log` a été utilisé ;
- la ligne de commande expurgée ;
- pour les erreurs de bundle, la sortie de `dbwarp-blueprint --bundle-list ...`.

Ne demandez pas les fichiers de mot de passe, fichiers de jeton, clés privées ou échantillons de lignes brutes de la base de données.
