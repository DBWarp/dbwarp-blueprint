# Ce que dbwarp-blueprint lit et écrit

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../AUDIT.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../AUDIT.md) | [Deutsch](../de/AUDIT.md) | **Français** | [Español](../es/AUDIT.md) | [Polski](../pl/AUDIT.md) | [日本語](../ja/AUDIT.md) | [简体中文](../zh/AUDIT.md)

Ce document énumère toutes les actions que l'outil peut effectuer. Comparez-les
à votre politique de sécurité.

## Sortie réseau

Le mode actif `--connect` ouvre une session du pilote de base de données vers le point de terminaison nommé. La résolution DNS peut utiliser le résolveur configuré, et l'authentification Kerberos/SSPI intégrée peut contacter un KDC ou un contrôleur de domaine. Le mode batch traite ses sources séquentiellement et ouvre une session pour chaque source de base de données. Les opérations TOML, Parquet, Avro et bundle hors ligne n'ouvrent aucune connexion réseau initiée par l'application, même si un chemin sur un système de fichiers réseau reste soumis à la pile de stockage de l'hôte.

Le binaire ne possède aucun chemin de télémétrie, de vérification de licence, de mise à jour de version, d'appel d'API cloud ou de téléversement.

Vous pouvez le vérifier avec `strace -f -e trace=connect,sendto,recvfrom`,
`tcpdump` ou eBPF sur la plateforme de votre choix.

## Lectures du système de fichiers

L'outil lit les entrées sélectionnées par le mode actif :

| Fichier | Quand | Contenu |
|---|---|---|
| `--user-file PATH` | Si fourni | Nom d'utilisateur uniquement. Les espaces de fin sont supprimés ; un fichier vide est une erreur. |
| `--password-file PATH` | Si fourni | Lu une fois, puis remis à zéro après utilisation. Refusé si son mode autorise la lecture par le groupe ou par tous. |
| `--azure-token-file PATH` | Si fourni | Jeton SQL Server Entra ID. Lu une fois, puis remis à zéro après utilisation. Refusé si son mode autorise la lecture par le groupe ou par tous. |
| `--tls-ca PATH` | Si fourni | Certificat d'autorité de certification PEM approuvé, lu au moment de la connexion. PostgreSQL/MySQL acceptent un bundle ; SQL Server accepte exactement un certificat. Le fichier fourni remplace les certificats racines par défaut du moteur. |
| `--tls-cert PATH` | Si fourni | Certificat TLS client PostgreSQL/MySQL (PEM), lu au moment de la connexion. Refusé pour SQL Server avec `DBP1015E`. |
| `--tls-key PATH` | Si fourni | Clé TLS client PostgreSQL/MySQL (PEM). Refusée si son mode autorise la lecture par le groupe ou par tous. Lue au moment de la connexion et refusée pour SQL Server avec `DBP1015E`. |
| `--from-toml PATH` | Si fourni | Fichier TOML dbwarp-blueprint existant, lu localement pour créer une présentation sans connexion à une base de données. |
| `--from-parquet PATH` | Si fourni | Métadonnées Parquet et, uniquement avec un consentement explicite à l'échantillonnage, lignes décodées en quantité limitée. |
| `--from-avro PATH` | Si fourni | Métadonnées et enregistrements du conteneur Avro ; le conteneur est parcouru pour obtenir le nombre de lignes. |
| `--batch-manifest PATH` | Si fourni | Manifeste et chaque chemin local d'entrée, d'informations d'identification, de jeton et TLS qu'il référence. |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Si fourni | TOML du bundle et fichiers Blueprint relatifs nécessaires pour l'affichage, l'extraction ou l'empaquetage. |
| `/dev/tty` | Si aucune source de mot de passe n'est fournie | Invite sans écho. |
| (compilation uniquement) `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.dbwarp-source-revision` dans les releases vendues, `vendor/mysql_async`, `vendor-crates/*` dans les bundles hors ligne | Uniquement lors de l'exécution de `./build.sh` | Entrées de chaîne d'outils, provenance source et compilation Cargo |

Ce qu'il ne lit **PAS** :
- `~/.pgpass`, `~/.my.cnf`, `~/.aws/credentials`, `~/.azure/credentials`
- Tout fichier sous `~/.ssh/*`
- `/etc/passwd`, `/etc/shadow`
- Toute variable d'identification de base de données autre que celle désignée par `--password-env`,
  `--user-env` ou `--azure-token-env`. Les compilations Kerberos intégrées
  peuvent également observer `KRB5CCNAME`, car libgssapi utilise le cache de
  tickets Kerberos. Les variables de langue et de présentation du terminal sont décrites ci-dessous.

## Écritures dans le système de fichiers

L'outil écrit uniquement les sorties sélectionnées par le mode actif :

| Fichier | Quand | Contenu |
|---|---|---|
| `--out PATH` (valeur par défaut `./blueprint.toml`) | Exécutions sur base de données active, Parquet, Avro, extraction de bundle et empaquetage de bundle | TOML Blueprint ou de bundle empaqueté. Non écrit par les modes présentation seule, affichage de bundle, simulation, aide ou version. |
| `--deck PATH` | Uniquement si indiqué | Présentation PowerPoint (.pptx) résumant le Blueprint anonymisé. Construite localement à partir du même Blueprint en mémoire ou de l'entrée `--from-toml` : aucune lecture supplémentaire de la base de données, aucun réseau, aucune bibliothèque tierce. |
| `--audit-log PATH` | Uniquement si indiqué | Copie remplacée atomiquement du journal d'audit émis sur stderr ; le contenu existant n'est pas complété. |
| `--out-dir DIR` | Mode batch hors simulation | `bundle.toml`, répertoires `blueprints/` et `audits/` par source, marqueur de propriété et `errors.txt` après un échec partiel. La publication utilise un répertoire de prépublication adjacent et un marqueur de récupération. |
| (compilation uniquement) `./target/`, `./build/` | Uniquement lors de l'exécution de `./build.sh` | Sorties standard de compilation Cargo |

Ce qu'il n'écrit **PAS** :
- `/var/log/*`
- `~/.cache/*`, `~/.local/*`, `~/.config/*`
- aucun répertoire temporaire système implicite (l'utilisateur peut toujours y diriger explicitement une sortie ou un répertoire batch)

## Variables d'environnement lues

L'audit ne répertorie que les variables réellement consultées. Si `--lang` ne
sélectionne pas déjà une langue prise en charge, la sélection peut lire
`DBWARP_BLUEPRINT_LANG`, `LC_ALL`, `LC_MESSAGES` et `LANG`. Le rendu du terminal peut lire `NO_COLOR`,
`TERM`, `COLORTERM` et `COLUMNS` ; ces variables n'affectent que la présentation.

Lorsque `--password-env VAR_NAME` ou `--user-env VAR_NAME` est indiqué,
l'outil lit exactement la variable nommée. Il ne se rabat pas sur les valeurs
courantes par défaut telles que `PGPASSWORD`, `MYSQL_PWD`, `MSSQL_PASSWORD`,
`USER` ou `LOGNAME` ; ces replis ne sont volontairement pas implémentés.

Lors de l'exécution de `./build.sh`, le script lit `PINNED_RUST` (remplacement),
`ALLOW_NETWORK` (autorisation explicite du téléchargement de rustup-init),
`TARGET` (cible de compilation croisée), ainsi que les variables Cargo/rustup
standard. Aucune n'est lue par l'outil lui-même à l'exécution.

## Journal d'audit par exécution

L'outil émet un journal d'audit sur stderr à chaque exécution. Son format est
du texte brut déterministe. Redirigez-le vers un fichier avec `2>audit.txt` ou
utilisez `--audit-log PATH` pour obtenir une copie explicite.

Exemple (Tier 1) :

```
=== dbwarp-blueprint audit ===
build_source_revision: 0123456789abcdef0123456789abcdef01234567
build_source_dirty:    false
build_toolchain:     1.94.0 (vendored)
mode:                tier-1
started_at_unix_ms:  1745596800000
outcome:             ok
schema_selector_count: 1

connection:
  - postgresql://app@db.example:5432/payments
    auth: scram-sha-256-or-md5
    tls: yes (protocol version unavailable from driver)
    tls_ca_only: false

auth:
  user_source:        file:/etc/dbwarp/db.user
  password_source:    file:/etc/dbwarp/db.pass (mode 0o600)
  password_persisted: false
  password_logged:    false
  authenticated_principal: (not observed)
  effective_server_principal: (not observed)
  database_principal: (not observed)
  expected_server_principal: (not requested)
  principal_assertion: not-observed

topology_and_scope:
  topology:
    deployment: unknown
    local_role: unknown
    visibility: partial
    member_count: 2
    identifiers_redacted: true
    role_counts: primary=1, secondary=1
    features: postgresql-streaming-replication
    catalogs_read: pg-is-in-recovery, pg-stat-replication
    catalogs_unreadable: (none)
  dataset_scope:
    layout: full-copy
    table_inventory_completeness: complete
    row_count_completeness: complete
    size_completeness: complete
    row_count_method: postgres-planner-estimate
    size_method: postgres-local-relation-size
    limitations: row-counts-statistical

blueprint_fidelity_estimate:
  basis: evidence-coverage-v1
  overall_score: 79/100
  band: good
  structure_score: 90/100
  sizing_score: 100/100
  column_statistics_score: 68/100
  relationship_score: 75/100
  artifact_score: 50/100
  limitations: biased-column-sampling, cardinality-lower-bounds
  qualification: evidence estimate, not source-truth accuracy or a confidence interval

artifact_inventory:
  detail: summary
  visibility: full
  objects: 42
  dependency_edges: 0
  external_prerequisites: 3
  inventory_complete: false
  dependencies_complete: false
  analysis_complete: false

database_operations_observed:
  1. [succeeded, 14ms, 28 rows]   server version lookup
  2. [succeeded, 9ms, 312 rows]   column catalog lookup
  ... (every observed catalog operation enumerated)

wire_bytes_observed:
  catalog_responses: unknown (driver does not expose wire-byte totals)
  row_data:          unknown (driver does not expose wire-byte totals)

local_sample_processing:
  encoded_rowframe_bytes: 0 B

sampling_work:
  compression_workers: 0
  compression_queue_capacity: 0
  compression_jobs_submitted: 0
  compression_jobs_completed: 0
  compression_pipeline_wall_ms: 0
  compression_worker_ms: 0
  tables_skipped_proven_empty: 0
  chunk_level_3_attempts: 0
  table_level_3_attempts: 0
  column_level_3_attempts: 0

files_read_local:
  - /etc/dbwarp/db.pass        (mode 0o600 ✓)

files_written_local:
  - ./blueprint.toml         (12 KiB, sha256: 7f3e2af1...)

warnings:
  - (none)

network_egress:
  - db.example:5432 (the DB connection only)

env_vars_read:
  - (none)

trust_assertions:
  - no row content was read
  - no telemetry was sent anywhere
  - all numeric statistics rounded to documented precision
  - identifier ordering is deterministic (sha256-based)
  - no random or pseudorandom data in output
  - artifact summary stores bounded counts only; no object identities or definitions
  - artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries
  - credential read once via Secret wrapper, zeroized when dropped at end of engine run; see SECURITY.md for driver-owned copy lifetimes (MySQL clones to non-zeroizing String for the driver API)

run_duration_ms:    142
finished_at_unix_ms: 1745596800142
=== end audit ===
```

Les exécutions MySQL émettent une assertion propre au mode
`length policy balanced|strict|exact`. Elle indique indépendamment si les
longueurs structurelles et échantillonnées sont exactes ou arrondies, afin que
l'audit n'affirme jamais que toutes les valeurs numériques ont été arrondies
pour une exécution équilibrée ou exacte.

Le journal d'audit :

- enregistre uniquement le nombre de sélecteurs actifs répétables `--schema` ; leurs valeurs sont affichées dans le contrôle préalable interactif, mais ne sont pas ajoutées à l'audit. L'URI de connexion expurgé existant continue d'identifier la base connectée, qui est aussi le nom du schéma avec MySQL. Un Blueprint sélectionné porte `selection-limited` dans `dataset_scope` ;
- indique la révision source intégrée à la compilation et l'état de l'arbre de travail ; le SHA-256 binaire final reste un contrôle externe de release/registre, car un binaire ne peut pas intégrer son propre hachage final ;
- enregistre la **source** des informations d'identification (chemin du fichier,
  nom de la variable d'environnement, TTY), jamais leur valeur ;
- sur SQL Server, enregistre les identités exactes de session renvoyées par
  `ORIGINAL_LOGIN()`, `SUSER_SNAME()` et `USER_NAME()`. Lorsque
  `--expect-server-principal` est fourni, l'audit enregistre aussi la valeur
  attendue et le résultat de la comparaison côté serveur avant la capture du
  catalogue ;
- répertorie chaque opération de base observée avec son résultat, sa durée et, si le pilote le fournit, son nombre de lignes ; une opération terminale en échec reçoit un libellé borné sans identifiant ;
- indique les octets réseau de la base comme `unknown` si le pilote ne les expose pas et sépare les octets d'échantillon encodés localement ;
- indique le nombre total d'octets écrits localement (avec le sha256 de chaque fichier) ;
- enregistre les dégradations non fatales de capture et d'échantillonnage avec des
  codes d'avertissement DBP stables ; une section vide signifie qu'aucune
  dégradation connue n'a été observée ;
- copie les preuves validées `[database_topology]` et `[dataset_scope]` dans `topology_and_scope`, uniquement avec des jetons fermés et des nombres ; aucun nom de nœud, endpoint ou identifiant de cluster ou de base ne peut apparaître ;
- conserve `DBP1411W`, `DBP1412W` et `DBP1413W` lorsque la topologie ou la couverture est incomplète, afin qu'une collecte réussie ne masque pas une réserve de dimensionnement ;
- enregistre une estimation déterministe et ventilée par dimension de la fidélité Blueprint. Le score décrit la couverture des preuves capturées pour la structure, le dimensionnement, les statistiques de colonnes, les relations et les artefacts. Il ne s'agit ni d'une erreur mesurée par rapport aux données source ni d'un intervalle de confiance statistique ;
- déclare les assertions de confiance adaptées au mode (Tier 1 ou Tier 2) ;
- est déterministe pour la même entrée : même base de données et mêmes arguments
  produisent le même audit, à l'exception des champs de durée.

**Émission conditionnelle des assertions de confiance.** La ligne
« credential read once via Secret wrapper... » n'est émise que lors des
exécutions où une information d'identification a effectivement été lue. Les
chemins d'échec qui s'interrompent avant son acquisition (erreurs d'analyse de
l'URI, refus des mots de passe intégrés à l'URI, simulation, etc.) n'émettent
volontairement *pas* cette ligne : il n'y a rien à affirmer au sujet d'une
information d'identification qui n'a jamais été obtenue. Utilisez la présence ou
l'absence de la ligne avec `auth.password_source` pour déterminer si la gestion
des informations d'identification a été exercée lors d'une exécution donnée.

**L'audit est émis sur les chemins opérationnels de réussite et d'échec**, y
compris les erreurs d'analyse de la ligne de commande après le démarrage. Les
sorties d'aide/version et les échecs antérieurs au chargement du contrat de
localisation intégré ne produisent pas d'audit complet. Un échec ultérieur est
tout de même écrit sur stderr et dans `--audit-log PATH` s'il est indiqué, sous la forme `outcome: error: <stage>`.
Exemple de ligne de résultat d'échec :

```
outcome:             error: parsing --connect URI (value redacted to avoid logging embedded credentials)
```

La sortie du terminal inclut également un résumé opérateur codé, tel que
`DBP1001E` ou `DBP0001E`, accompagné de la chaîne causale. Le résultat de
l'audit est limité en taille et peut tronquer un texte long ; utilisez la sortie
du terminal avec le code de message pour le triage du support. Consultez
`docs/MESSAGES.md`.

Les sondes facultatives de RTT, compression et style de texte peuvent échouer
sans invalider la capture principale du catalogue. Ces cas sont affichés et
conservés sous `warnings:` avec les codes `DBP1405W` à `DBP1408W`, afin qu'un
résultat Tier 2 réussi mais partiel puisse être distingué d'un résultat complet.
Les avertissements identiques répétés sont dédupliqués et les détails de pilote
sur plusieurs lignes sont aplatis, afin que l'audit reste limité et analysable
par une machine.

## Lectures des artefacts hors tables

La collecte des artefacts est indépendante de l'échantillonnage de lignes du Tier 2 :

- `--artifact-detail none` ignore les catalogues d'artefacts et les définitions.
- `summary` lit les catalogues d'objets modélisés, mais pas le texte des définitions.
- `graph` lit aussi les catalogues de dépendances, mais pas le texte des définitions.
- `analyzed` lit en plus les définitions SQL/procédurales disponibles dans une mémoire de processus bornée pour l'analyse lexicale.

L'audit enregistre le détail demandé, la visibilité, les comptages d'objets, dépendances et prérequis externes, ainsi que tous les indicateurs de complétude. Chaque opération de catalogue apparaît dans `database_operations_observed`. L'échec d'un catalogue facultatif émet `DBP1410W`, apparaît dans `warnings` et empêche une affirmation de complétude inexacte.

En mode analysé, les définitions sont détenues par un propriétaire qui les efface, puis réduites à des bandes bornées et des jetons de caractéristiques fermés. Le texte des définitions, les noms d'objets source, les points de terminaison externes, les principaux d'artefacts, les informations d'identification, les clés/certificats, les noms de paquets/bibliothèques et les binaires ne sont jamais écrits dans le Blueprint ou le journal d'audit. Les seuls noms de principaux exacts conservés sont les trois identités de session SQL Server du bloc d'audit `auth` explicite ci-dessus ; ils ne sont jamais écrits dans le Blueprint, la présentation ou les artefacts publiés. Les modes graph et analyzed exigent `--yes`, car une topologie anonyme peut identifier une application.

L'audit distingue les postures de confidentialité par l'une de ces assertions de confiance :

- summary : comptages bornés uniquement, sans identité d'objet ni définition ;
- graph : graphe de dépendances anonyme, sans définition ;
- analyzed : définitions lues transitoirement, seules des bandes bornées sont conservées.

Consultez [`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) pour la couverture des familles et l'interprétation de la complétude.

## Ajouts du Tier 2

Lorsque la mesure de compression est acceptée interactivement, ou sans interaction avec `--measure-compression --yes`, l'outil effectue également les opérations suivantes :

- Pour chaque table qui n'est pas prouvée vide, il suit un chemin
  d'échantillonnage borné propre au moteur. PostgreSQL commence par
  `TABLESAMPLE SYSTEM(0.1) LIMIT N` et se rabat sur `LIMIT N` si nécessaire ;
  MySQL utilise `LIMIT N` et SQL Server `TOP N`. Les chemins biaisés définissent
  `sampled_with_bias = true` dans la sortie.
- Il lit les lignes échantillonnées dans un tampon local en mémoire.
- Les lectures de base de données restent séquentielles. L'option
  `--compression-workers N` peut exécuter de 1 à 32 workers locaux bornés
  (1 par défaut afin de limiter l'impact sur l'hôte source). Augmentez-la
  explicitement pour utiliser davantage de CPU locale. Chaque worker possède
  ses contextes zstd et ne dépend d'aucun verrou zstd partagé.
- Il compresse avec zstd au niveau 3.
- Il enregistre les ratios obtenus et l'écart type.
- Il **supprime chaque tampon lorsque son travail local borné est terminé**.
  Les octets ne sont ni écrits sur disque ni transmis. Le pool conserve au
  plus N échantillons en attente et N échantillons en cours de compression.

`local_sample_processing.encoded_rowframe_bytes` indique les octets encodés
localement pour la compression, et non les octets réseau de la base. Les octets
non exposés par le pilote restent `unknown`. Le bloc `[compression]` contient
les ratios. `--max-wall-secs` est une échéance stricte pour toute la collecte
active, connexion, catalogues, RTT et Tier 2 compris. PostgreSQL définit aussi
le `statement_timeout` local à la session, MySQL le `max_execution_time` local
à la session pour les instructions `SELECT` en lecture seule, et SQL Server le
`LOCK_TIMEOUT` local à la session, car il ne possède pas de limite de session
équivalente pour la durée totale d’une instruction. À l’expiration de
l’échéance globale, le client ferme la connexion. L’audit ne considère pas
cette fermeture comme une preuve que SQL Server a accusé réception de
l’annulation ; un opérateur doit vérifier que le travail du serveur s’est
arrêté avant de réessayer.

`sampling_work` constitue une preuve opérationnelle sans identifiant. Cette
section comptabilise les bornes des workers et de la file locale, le plafond de
16 MiB de charge utile projetée par table, les travaux soumis et terminés, les
tentatives de compression et les tables non
échantillonnées parce que le catalogue du moteur les prouvait vides au moment
de sa lecture. `compression_worker_ms` est le temps mural cumulé des workers,
pas le temps CPU du processus, et peut dépasser
`compression_pipeline_wall_ms` lorsque des workers se chevauchent. Le temps
mural du pipeline peut chevaucher les lectures de base de données, qui restent
séquentielles. Ces compteurs décrivent le travail effectué ; ce ne sont ni des
nombres de lignes, ni des mesures d'octets réseau, ni des affirmations sur
l'exactitude de la source.

## Protocole de vérification

Si vous souhaitez *prouver* que l'outil ne fait que ce qui est documenté :

1. **Audit des sources** : clonez le dépôt, lisez `src/secret.rs`, puis recherchez
   `\.expose\(\)` en dehors de ce fichier :
   ```
   $ rg -n '\.expose\(\)' src --glob '!secret.rs'
   ```
   Les sites d'appel de production remettent immédiatement le `&str` exposé au
   constructeur de connexion d'un pilote. MySQL appelle en outre `.to_string()`, car l'API de `mysql_async`
   exige une `String` ; cette copie n'est pas remise à zéro et persiste jusqu'à
   la suppression de l'`OptsBuilder`. Tier 1 et Tier 2 réutilisent la même
   connexion MySQL. Consultez SECURITY.md §2 pour la
   discussion complète.
2. **Compilation depuis les sources** : `./build.sh`. La CI de publication effectue une reconstruction indépendante sur le même runner, dans un répertoire cible Cargo distinct, et rejette toute différence d'octets. Une comparaison locale n'est pertinente qu'avec la même révision source, la même cible, les mêmes fonctionnalités, la même chaîne Rust épinglée, le même éditeur de liens et les mêmes options de compilation.
3. **Comparaison à la version** : `./verify.sh release/dbwarp-blueprint-X.Y.Z-...`
4. **Trace d'exécution** : exécutez `strace -f -e trace=open,connect,read,write`
   dans un bac à sable. Comparez avec les listes ci-dessus.
5. **Trace réseau** : utilisez `tcpdump` sur l'hôte. Pour une exécution active authentifiée par mot de passe, vérifiez la session de base de données et le trafic DNS attendu. Pour l'authentification intégrée, tenez également compte du trafic attendu vers le KDC ou le contrôleur de domaine. En mode batch, rapprochez une session de base de données par source de base de données.

Si l'un de ces éléments ne correspond pas à ce qui est documenté ici, ouvrez un ticket avec votre trace ; nous l'examinerons dans les 72 heures.
