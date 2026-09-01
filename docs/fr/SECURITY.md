# Modèle de sécurité

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../SECURITY.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../SECURITY.md) | [Deutsch](../de/SECURITY.md) | **Français** | [Español](../es/SECURITY.md) | [Polski](../pl/SECURITY.md) | [日本語](../ja/SECURITY.md) | [简体中文](../zh/SECURITY.md)

`dbwarp-blueprint` possède des modes distincts pour les bases de données actives, les fichiers structurés, les traitements batch, les bundles et les présentations. Le mode sélectionné détermine son périmètre réseau et système de fichiers. Il ne possède aucun chemin de télémétrie, de vérification des mises à jour, de vérification des licences, d'analyse ou de téléversement.

Cette page explique les limites de sécurité afin que votre équipe puisse décider d'exécuter ou non l'outil.

## Signaler une vulnérabilité

Signalez les vulnérabilités présumées de manière privée au moyen de la
[fonction de signalement privé des vulnérabilités de GitHub](https://github.com/DBWarp/dbwarp-blueprint/security/advisories/new).
Ne publiez pas de détails sensibles dans un ticket public. Indiquez la version
exacte, le système d’exploitation, les étapes de reproduction et le plus petit
élément de preuve sûr permettant d’évaluer le signalement.

## Réseau

| Mode | Utilisation du réseau à l'exécution |
|---|---|
| `--connect` actif | Une session du pilote de base de données vers le point de terminaison nommé. La résolution DNS peut contacter le résolveur configuré. L'authentification Kerberos/SSPI intégrée peut aussi contacter une infrastructure d'identité configurée, telle qu'un KDC ou un contrôleur de domaine. |
| `--batch-manifest` | Une session du pilote pour chaque source de base de données du manifeste, traitée séquentiellement. Les sources Parquet et Avro locales n'utilisent pas le réseau. Les qualifications DNS et d'authentification intégrée ci-dessus restent applicables. |
| `--from-toml`, `--from-parquet`, `--from-avro`, `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Aucune connexion réseau initiée par l'application. Les entrées situées sur des systèmes de fichiers réseau restent du ressort du système d'exploitation et du stockage. |

L'outil n'appelle aucun service DBWarp ni aucune API cloud. Les pilotes de base de données et le système d'exploitation hôte peuvent produire le trafic de prise en charge des protocoles décrit ci-dessus.

`--max-wall-secs` établit deux protections indépendantes. PostgreSQL utilise un
`statement_timeout` local à la session, et MySQL un `max_execution_time` local à
la session pour les instructions `SELECT` en lecture seule du collecteur. SQL
Server ne dispose pas d’un réglage de session équivalent pour la durée totale
d’une instruction ; le collecteur définit donc un `LOCK_TIMEOUT` local à la
session afin de borner les attentes de verrou et conserve l’échéance du client
pour les autres blocages. Si cette échéance expire, l’outil ferme sa connexion ;
il n’affirme pas que SQL Server a accusé réception d’une annulation côté
serveur. Vérifiez que le travail du serveur est arrêté avant de réessayer.

## Fichiers lus

À l'exécution, l'outil lit uniquement les entrées sélectionnées sur la ligne de commande ou référencées par une entrée batch ou bundle :

| Fichier | Quand |
|---|---|
| `--user-file` | source du nom d'utilisateur |
| `--password-file` | source du mot de passe |
| `--anonymization-key-file` | clé HMAC facultative conservée par le client afin de préserver les libellés d’objet anonymes entre les exécutions approuvées ; sous Unix, le mode ne doit pas autoriser la lecture par le groupe ou les autres utilisateurs |
| `--azure-token-file` | source du jeton SQL Server Entra ID |
| `--tls-ca` | bundle d'autorités de certification approuvées |
| `--tls-cert` | certificat TLS client |
| `--tls-key` | clé privée TLS client |
| `--from-toml` | fichier TOML dbwarp-blueprint existant utilisé pour créer une présentation hors ligne |
| `--from-parquet` | métadonnées du fichier Parquet et, avec un consentement explicite à l'échantillonnage, lignes décodées en quantité limitée |
| `--from-avro` | métadonnées et enregistrements du conteneur d'objets Avro ; le conteneur doit être parcouru pour compter les enregistrements |
| `--batch-manifest` | manifeste batch et chaque fichier structuré local, fichier d'informations d'identification, fichier de jeton et fichier TLS qu'il référence |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | fichier TOML du bundle et fichiers Blueprint relatifs nécessaires à l'opération sélectionnée |
| `/dev/tty` | invite interactive de mot de passe sur les systèmes de type Unix |

Il ne lit pas `~/.pgpass`, `~/.my.cnf`, les fichiers d'informations d'identification cloud, les clés SSH, l'historique du shell ou les variables d'environnement de mot de passe par défaut.

Pour PostgreSQL et MySQL, un bundle PEM fourni avec `--tls-ca` remplace les
certificats racines Mozilla intégrés. SQL Server utilise le magasin de confiance
du système d'exploitation lorsque `--tls-ca` est omis ; un fichier `.pem` ou
`.crt` fourni doit contenir exactement un certificat d'autorité de
certification et remplace ces certificats racines. SQL Server valide le nom
d'hôte dans les deux modes de vérification des certificats et refuse
`--tls-cert`/`--tls-key` avec `DBP1015E`, car son pilote n'implémente pas
l'authentification par certificat client.

## Fichiers écrits

À l'exécution, l'outil peut écrire :

| Fichier | Quand |
|---|---|
| `--out` | sortie Blueprint pour les modes base de données active, fichier structuré, extraction de bundle ou empaquetage de bundle |
| `--deck` | synthèse PowerPoint (.pptx) facultative, générée localement depuis le Blueprint anonymisé ou l'entrée `--from-toml` (aucune lecture supplémentaire de la base de données, aucun réseau, aucune bibliothèque tierce) |
| `--audit-log` | copie facultative du journal d'audit |
| `--out-dir` | répertoire batch contenant `bundle.toml`, `blueprints/*.blueprint.toml`, `audits/*.audit.txt`, un marqueur de propriété et `errors.txt` lorsqu'une ou plusieurs sources échouent ; un répertoire de prépublication adjacent est utilisé pendant la publication atomique et supprimé en cas d'échec traité |

Le journal d'audit est également affiché sur stderr.

Traitez chaque audit et chaque fichier batch `errors.txt` comme une preuve opérationnelle à accès contrôlé. Ils peuvent contenir des noms de points de terminaison, des chemins locaux, des identifiants de source du manifeste, des erreurs de pilote et des données de temps. Pour SQL Server, l'audit contient le login authentifié exact (`ORIGINAL_LOGIN()`),
le principal serveur effectif (`SUSER_SNAME()`) et le principal de base de
données (`USER_NAME()`), ainsi qu'un principal attendu facultatif et le résultat
de l'assertion. Ces identités ne sont pas écrites dans un Blueprint monosource ni dans une présentation. Les métadonnées du bundle conservent les identifiants de source, les tags et les identifiants de groupe de jeux de données fournis par l'opérateur ; choisissez des valeurs anonymes et vérifiez le TOML du bundle avant le transfert.

## Variables d'environnement

Par défaut, aucune variable d'environnement d'exécution n'est lue pour obtenir des informations d'identification.

Si vous fournissez `--password-env NAME`, `--user-env NAME` ou `--azure-token-env NAME`, l'outil lit exactement la variable nommée. Il ne se rabat pas sur des valeurs courantes par défaut telles que `PGPASSWORD`, `MYSQL_PWD` ou `MSSQL_PASSWORD`.

## Informations d'identification

Les informations d'identification sont encapsulées dans un type `Secret` qui, volontairement, n'implémente pas `Debug`, `Display`, `Clone` ou la sérialisation. Cela rend difficile à compiler un code qui les journaliserait accidentellement.

Les informations d'identification ne sont transmises au pilote de base de données que pour établir la connexion. Elles ne sont écrites ni dans le fichier de sortie ni dans le journal d'audit. Le journal d'audit enregistre la source des informations d'identification, par exemple `file:/etc/dbwarp/db.pass`, mais pas leur valeur.

## Modèles d'informations d'identification refusés

Les mots de passe intégrés à l'URI de connexion sont refusés. Par exemple, ceci n'est pas accepté :

```text
postgresql://user:password@host/db
```

Utilisez plutôt `--password-file`, `--password-env` ou l'invite interactive. Cela évite la divulgation des mots de passe dans l'historique du shell, la liste des processus ou le défilement du terminal.

## Sécurité de la sortie

Le fichier Blueprint est conçu pour être lisible et vérifiable par un humain :

- les identifiants réels sont remplacés par des noms anonymes associés à une clé tels que `table-001` et `col-1` ;
- les valeurs numériques sont arrondies selon des intervalles documentés ;
- les commentaires sont fixes et ne servent pas de canal de données ;
- les valeurs de ligne ne sont jamais émises ;
- lorsqu'ils sont activés, les échantillons de compression sont compressés localement puis supprimés.

Le Tier 2 actif applique un plafond strict de 16 MiB de charge utile projetée
par table avant que le pilote ne reçoive les données de lignes. Il réduit le
nombre de lignes demandé pour les tables extrêmement larges et projette les
cellules de largeur variable au moyen d’une troncature native au moteur côté
serveur. Les sondes de style ont un plafond distinct dans leur projection SQL.
L’encodeur local de trames de lignes impose indépendamment le même plafond par
table. Une petite valeur de `--sample-rows` ne peut donc pas transférer une
charge utile LOB non bornée ; les valeurs très volumineuses ne contribuent aux
estimations de compression et de longueur que par leurs préfixes bornés.

L’ordre des tables, schémas, index et objets hors tables utilise HMAC-SHA256
avec séparation de domaines. Par défaut, l’outil obtient une nouvelle clé
locale au processus auprès du système d’exploitation et ne l’émet jamais, ce
qui empêche un lecteur hors ligne de tester des noms source candidats.
N’utilisez `--anonymization-key-file` que si les mêmes libellés anonymes doivent
être conservés entre des exécutions de comparaison approuvées. Le fichier doit
contenir exactement 32 octets bruts ou 64 caractères hexadécimaux et être
protégé comme une information d’identification. L’audit indique seulement si
une clé éphémère ou conservée par le client a été utilisée, jamais sa valeur.

Cela réduit le risque de divulgation, mais ne rend pas chaque sortie sûre pour chaque destinataire. La forme anonyme du schéma, les graphes de dépendances, les versions des moteurs, les champs exacts facultatifs et les distributions de taille inhabituelles peuvent caractériser une charge de travail. Vérifiez les sorties Blueprint et bundle selon la politique de classification des données de votre organisation avant de les partager. N'envoyez pas les audits ou `errors.txt` comme s'il s'agissait de Blueprints anonymisés.

Consultez [`FORMAT.md`](FORMAT.md) pour connaître les champs exacts.

## Journal d'audit

Chaque exécution produit un journal d'audit qui répertorie :

- le point de terminaison de base de données contacté ;
- la source d'informations d'identification utilisée ;
- les principaux SQL Server correspondant à l'identité authentifiée, au serveur
  effectif et à la base de données, lorsque la session peut les communiquer ;
- le mode TLS ;
- les fichiers lus ;
- les fichiers écrits ;
- les requêtes exécutées ;
- si l'échantillonnage des lignes était activé ;
- le résultat final.

Consultez [`AUDIT.md`](AUDIT.md).

## Points de départ pour la revue des sources

Pour une revue ciblée :

- `src/secret.rs` : encapsulation des informations d'identification
- `src/main.rs` : CLI, barrières de consentement, émission de l'audit
- `src/audit.rs` : rendu du journal d'audit
- `src/format.rs` : format de sortie anonymisé
- `src/tls.rs` : configuration TLS
- `src/engine_pg.rs`, `src/engine_mysql.rs`, `src/engine_mssql.rs` : lecteurs de catalogue propres à chaque base de données
