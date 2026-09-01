<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../.github/assets/dbwarp-logo-dark.png">
    <img src="../../.github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

# dbwarp-blueprint

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../README.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../README.md) | [Deutsch](../de/README.md) | **Français** | [Español](../es/README.md) | [Polski](../pl/README.md) | [日本語](../ja/README.md) | [简体中文](../zh/README.md)

## Présentation

DBWarp Blueprint est un collecteur de Blueprint de base de données axé sur la confiance. Vous l’exécutez dans votre propre environnement sur PostgreSQL, MySQL ou SQL Server. Il lit les métadonnées du catalogue et, si vous demandez une mesure de compression, un échantillon borné de lignes. Il écrit ensuite un Blueprint structurel anonymisé de votre base de données : tailles des tables, nombres de lignes, familles de types, ainsi que structure des index et des clés étrangères.

Les identifiants sont remplacés par des libellés anonymes associés à une clé et
aucune valeur de ligne n’est écrite dans le Blueprint. Par défaut, une nouvelle
clé locale au processus empêche les vérifications hors ligne par dictionnaire ;
`--anonymization-key-file` permet au client de conserver les libellés entre des
exécutions de comparaison approuvées. Lisez [`SECURITY.md`](SECURITY.md) avant
de partager une sortie : ce document décrit précisément ce que chaque mode
divulgue et les options qui élargissent ce périmètre.

La sortie est un fichier en texte brut. Vous pouvez en lire chaque ligne avant de décider de la partager ou non.

DBWarp Blueprint est gratuit et open source, et s’exécute entièrement dans votre environnement. Il vous permet de nous communiquer des faits sur votre base de données sans nous communiquer votre base de données elle-même.

## Pourquoi l’exécuter

Partagez votre sortie Blueprint avec nous et nous pourrons vous indiquer dans quelle mesure DBWarp accélérerait le déplacement de vos données, ainsi que l’incidence sur les calendriers de migration, de données de test CI/CD et d’analyse.

La distance est le facteur déterminant. Plus vos données doivent voyager loin, plus l’amélioration que DBWarp peut vous montrer est importante.

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; Zurich, Suisse

---

`dbwarp-blueprint` est le collecteur Blueprint DBWarp exécuté chez le client. Exécutez-le dans l'environnement du client pour produire un fichier `blueprint.toml` assaini et vérifiable que DBWarp peut utiliser pour dimensionner une migration, générer des fixtures synthétiques et préparer l'opération, sans recevoir d'accès à la base de données, de dumps, de noms de schémas ou de données de lignes.

Il se connecte à PostgreSQL, MySQL ou SQL Server, lit les métadonnées du catalogue, mesure éventuellement la compression locale à partir d'un échantillon de lignes de taille limitée et écrit du TOML en texte brut. Il peut aussi dériver un Blueprint depuis des fichiers Parquet ou Avro locaux en mode hors ligne lorsque l'entrée est déjà un fichier de données structurées plutôt qu'une base de données active. Vous pouvez ouvrir la sortie, en vérifier chaque ligne et décider de la partager ou non.

Facultativement, `--deck blueprint.pptx` écrit également une synthèse PowerPoint du même Blueprint anonymisé. La présentation peut être générée pendant une exécution sur une base de données active ou ultérieurement à partir d'un fichier TOML vérifié avec `--from-toml blueprint.toml --deck blueprint.pptx`. Le générateur de présentations est intégré au binaire Rust et n'établit aucune connexion réseau.

## Utilité de l'outil

DBWarp a besoin d'informations structurelles suffisantes pour estimer et planifier un transfert :

- nombre de tables ;
- nombre approximatif de lignes ;
- taille des tables et des index ;
- familles de types des colonnes, capacités structurelles/préfixes d'index exacts et,
  par défaut, largeurs observées arrondies pour préserver la confidentialité ;
- structure des index et des clés étrangères ;
- comptages respectueux de la confidentialité des artefacts hors tables et prérequis de déploiement externes ;
- synthèses facultatives de compression par table et colonne à partir d'un petit échantillon local ;
- mesures facultatives du RTT de la base de données côté client.

Ces éléments suffisent pour estimer la taille du transfert, choisir un plan de départ pour le transfert en masse DBWarp et générer une fixture de benchmark synthétique représentative. Ils ne suffisent pas pour reconstruire le schéma ou les données du client.

## Ce que l'outil ne fait pas

`dbwarp-blueprint` ne fait pas ce qui suit :

- envoyer de la télémétrie ;
- appeler des serveurs DBWarp ;
- téléverser le fichier Blueprint ;
- lire `~/.pgpass`, `~/.my.cnf`, des informations d'identification cloud ou des clés SSH ;
- lire les variables d'environnement de mot de passe par défaut telles que `PGPASSWORD` ou `MYSQL_PWD` ;
- écrire autre chose que les sorties sélectionnées pour le mode actif ; le mode batch écrit un répertoire de bundle contenant les Blueprints enfants, les audits enfants et d'éventuelles preuves d'échec ;
- inclure dans la sortie les noms réels de tables, colonnes, index ou schémas, les noms d'objets hors tables, les définitions SQL, les points de terminaison externes, les informations d'identification, les clés, les certificats, les binaires ou les valeurs de lignes.

Les exécutions Blueprint actives ouvrent une session de base de données vers le point de terminaison indiqué. La résolution DNS peut utiliser le résolveur configuré, et l'authentification Kerberos/SSPI intégrée peut contacter l'infrastructure d'identité. Le mode batch répète cette limite pour chaque source de base de données. Les opérations TOML, Parquet, Avro et bundle locales n'ouvrent aucune connexion réseau initiée par l'application.

## Télécharger ou compiler

| Méthode | Usage recommandé | Lien |
|---|---|---|
| Télécharger un binaire | essai rapide, appel avec un ingénieur commercial, hôte de test isolé | [`binaries/README.md`](BINARIES.md) |
| Compiler depuis un petit clone des sources | revue de sécurité, politique de production, contrôle de reproductibilité | [`BUILD.md`](BUILD.md) |
| Compiler depuis un bundle de sources contenant les dépendances | audit strict et hors ligne des dépendances | GitHub Releases |

La méthode privilégiant la confiance consiste à compiler depuis les sources. Le dépôt normal reste petit et utilise `Cargo.lock` pour figer les versions des dépendances. Pour les audits hors ligne plus stricts, chaque version publie également un bundle de sources contenant le code source de toutes les dépendances. Des binaires de version sont fournis par commodité, accompagnés de sommes de contrôle SHA256.

## Démarrage rapide

Choisissez une langue de présentation lorsque cela est utile. L'anglais est la
langue par défaut ; des catalogues complets sont intégrés pour l'allemand, le
français, l'espagnol, le polonais, le japonais et le chinois simplifié :

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --dry-run
```

Seuls l'aide destinée aux humains, les demandes, les diagnostics, la progression
et les libellés des présentations PowerPoint sont traduits. Les noms de commandes
et d'options, les valeurs acceptées, les schémas d'URI, les noms de variables
d'environnement, les sélecteurs, les codes DBP, les clés d'audit et le TOML
généré restent des jetons anglais canoniques. Les procédures d'automatisation et
de support sont donc identiques dans toutes les langues. Consultez
[`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

Commencez par une simulation. Elle affiche le plan sans établir de connexion :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --dry-run
```

Exécution recommandée de type production avec TLS, journal d'audit et mesure de compression :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Avec `--measure-compression --yes`, la sortie inclut les ratios zstd au niveau
des tables et les projections de compression par colonne. Les blocs par colonne
sont calculés à partir du même échantillon limité que le ratio au niveau de la
table ; ils servent à l'estimation des fixtures DBWarp et n'écrivent pas les
valeurs échantillonnées sur disque. Le schéma v3 et les versions ultérieures émettent aussi des agrégats de
cardinalité et de distribution par colonne respectueux de la confidentialité,
ainsi que des résumés inférés des préfixes d'index et des relations. Les
empreintes temporaires sont bornées en mémoire puis supprimées ; aucune valeur
ni empreinte n'apparaît dans le fichier TOML Blueprint.

Depuis le schéma v4, les Blueprints inventorient également les objets hors tables. Par défaut,
`--artifact-detail summary` conserve des comptages bornés par classe d'objet et
de prérequis externe sans lire les définitions. `graph` ajoute une topologie de
dépendances anonyme et `analyzed` des bandes bornées de caractéristiques du
langage et de complexité ; tous deux exigent `--yes`, car même un graphe anonyme
peut identifier une application :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```


La présence d'un artefact constitue une preuve de planification, pas une
promesse que DBWarp peut le recréer ou le traduire automatiquement. Consultez
[`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

### Fidélité des longueurs MySQL

La politique `balanced` par défaut préserve exactement les capacités déclarées
en caractères/octet et les longueurs de préfixe d'index. Les longueurs
moyenne/p95 des valeurs échantillonnées utilisent des intervalles à erreur
relative (environ 3,2 % d'erreur maximale, avec conservation exacte des valeurs
jusqu'à 32 octets). Ainsi, une clé `VARCHAR(3000)` dont les valeurs contiennent
normalement 9 caractères reste proche de 9 caractères dans les données générées,
tout en conservant des limites DDL/d'index source valides :

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

N'utilisez les statistiques échantillonnées exactes que lorsque la politique autorise cette précision supplémentaire :

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

Utilisez `--length-fidelity strict` pour conserver l'ancien regroupement
grossier, partageable en toute sécurité, des longueurs déclarées, observées et
de préfixe. Le mode strict sacrifie volontairement la fidélité des fixtures/index
et n'est pas prêt pour les benchmarks client. L'ancienne syntaxe
`--preserve-exact-lengths --yes` reste un alias de compatibilité pour
`--length-fidelity exact --yes`.

Les nouveaux Blueprints enregistrent séparément les champs
`declared_length_fidelity`, `index_length_fidelity` et
`observed_length_fidelity`. Le champ historique `length_metadata` est conservé
pour assurer une compatibilité prudente avec les anciens consommateurs. Les
capacités de caractères PostgreSQL sont des valeurs exactes du catalogue ; les
limites en octets dépendant de l’encodage et les longueurs de préfixe d’index
restent indisponibles.

Pour un benchmark généré représentatif du client, `--measure-compression` n'est
pas facultatif : cette option fournit les longueurs moyenne/p95 observées, afin
qu'une clé déclarée sur plusieurs kilo-octets mais dont les valeurs réelles ne
comptent que quelques caractères ne soit pas générée à sa capacité maximale. Le
budget temporel d'échantillonnage par défaut est de 300 secondes. Augmentez
`--max-wall-secs` pour les très grands schémas. Les outils de planification en
aval doivent refuser le Blueprint si une colonne indexée non vide et de largeur
variable reste non échantillonnée. Une génération de vérification ou de
compatibilité nécessite alors une dérogation explicite en aval et doit être
marquée comme non représentative.

Vérifiez ensuite les fichiers :

```bash
less blueprint.toml
less audit.txt
```

Si votre politique l'autorise, partagez `blueprint.toml` avec DBWarp. Une présentation peut également être partagée après vérification. Conservez le journal d'audit comme preuve opérationnelle à accès contrôlé, sauf si un cas de support précis l'exige via un canal sécurisé approuvé ; il contient des détails sur le point de terminaison, l'identité, les chemins et les temps.

## Mode fichier structuré

Si la source est déjà un fichier structuré local, générez le TOML Blueprint sans informations d'identification de base de données :

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

```bash
./dbwarp-blueprint \
  --from-avro /data/sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Le mode Parquet lit le pied de fichier et les métadonnées des groupes de lignes. Les conteneurs d'objets Avro ne possèdent pas de nombre de lignes équivalent dans un pied de fichier ; le mode Avro parcourt donc le conteneur pour compter les enregistrements et utilise le schéma d'écriture pour déterminer la structure des colonnes. Aucun de ces modes ne se connecte à une base de données ni ne lit d'options d'informations d'identification.

Si votre politique permet l'échantillonnage décodé, le mode fichier peut aussi
estimer la compression de type transport DBWarp à partir d'échantillons locaux
de taille limitée :

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Les mêmes options fonctionnent avec `--from-avro`. Les valeurs échantillonnées
sont encodées en mémoire sous la forme `dbwarp-blueprint-rowframe-v1` ; seuls les
ratios globaux de compression zstd sont écrits dans le TOML Blueprint.

## Mode lot et bundle

Pour plusieurs bases de données, plusieurs tables/jeux de données ou une revue
du parc d'un client, utilisez un manifeste de lot et écrivez un répertoire de
bundle :

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Le répertoire de travail contient `bundle.toml`, des fichiers Blueprint enfants
par source et des journaux d'audit par source à accès contrôlé. Ne transférez
pas l'ensemble du répertoire de travail par défaut. Vous pouvez en afficher le
contenu, en extraire des éléments ou créer un bundle Blueprint empaqueté et vérifié séparément :

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
./dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 --out table-042.blueprint.toml
./dbwarp-blueprint --bundle-pack customer-blueprint-bundle --out customer-blueprint-bundle.packed.toml
```

Consultez [`docs/BATCH_AND_BUNDLES.md`](BATCH_AND_BUNDLES.md) pour la syntaxe
du manifeste, les modes de jeu de données de fichiers structurés et les règles
des sélecteurs.

## Commandes courantes par base de données

PostgreSQL :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

MySQL :

```bash
./dbwarp-blueprint \
  --connect mysql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

SQL Server :

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@db.internal,1433/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

Pour des exemples Kerberos, SSPI et Entra ID, consultez [`AUTH.md`](AUTH.md). Pour les autorités de certification internes, mTLS et la vérification du nom d'hôte, consultez [`TLS.md`](TLS.md).

## Mode catalogue uniquement

Si la politique interdit l'échantillonnage des lignes, omettez `--measure-compression` :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

Le mode catalogue uniquement lit exclusivement les métadonnées. DBWarp peut toujours effectuer une estimation à partir de la taille des tables, du nombre de lignes, des familles de types et de la structure des index/clés étrangères, mais la compression et le réalisme des fixtures synthétiques sont moins précis, car l'entropie du texte/des données binaires doit être déduite.

## Aperçu de la sortie

```toml
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 6
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 28
row_count = 12500000
table_bytes = 4200000000
index_bytes = 1100000000

[tables.table-001]
rows = 12500000
table_bytes = 4200000000
index_bytes = 1100000000
schema = "schema-A"
has_clustered_index = false

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false

[tables.table-001.idxs.idx-1]
type = "btree"
primary = true
unique = true
cols = [1]
```

Le contrat complet du fichier est documenté dans [`FORMAT.md`](FORMAT.md). Le journal d'audit est documenté dans [`AUDIT.md`](AUDIT.md).

## Présentation visuelle récapitulative

Générez une présentation pendant l'exécution active :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

Ou générez-la ultérieurement depuis un fichier Blueprint vérifié, sans connexion à la base de données :

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx
```

La présentation s'adapte à la taille du schéma : détails par table pour les petits schémas, diapositives de caractérisation pour les grands schémas, synthèse de compression lorsque des données Tier 2 sont présentes et diapositive sur le modèle de confiance. Consultez [`DECK.md`](DECK.md).

## Documentation

Pour commencer :

- [`docs/QUICKSTART.md`](QUICKSTART.md) : première exécution sûre et premier dossier de transmission.
- [`docs/COOKBOOK.md`](COOKBOOK.md) : recettes pratiques pour PostgreSQL, MySQL, SQL Server, TLS, les présentations et les flux de travail sans échantillonnage.
- [`docs/DBA_REVIEW_GUIDE.md`](DBA_REVIEW_GUIDE.md) : ce qu'un DBA ou responsable de la sécurité doit savoir avant d'exécuter l'outil.
- [`sql/grants/README.md`](../../sql/grants/README.md) : scripts d’octroi de privilèges minimaux tenant compte des versions et suppression du compte après la capture.
- [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md) : échecs courants et corrections.
- [`docs/MESSAGES.md`](MESSAGES.md) : codes de message opérateur stables `DBPnnnnS`.
- [`docs/COMPRESSION_MEASUREMENT.md`](COMPRESSION_MEASUREMENT.md) : fonctionnement de l'échantillonnage de compression Tier 2.
- [`docs/INDEX.md`](INDEX.md) : carte complète de la documentation.

Points de départ de la revue de sécurité :

- [`SECURITY.md`](SECURITY.md) : modèle de sécurité et gestion des informations d'identification.
- [`AUDIT.md`](AUDIT.md) : éléments lus, écrits, interrogés et journalisés.
- [`FORMAT.md`](FORMAT.md) : champs de sortie et règles d'arrondi.
- [`TLS.md`](TLS.md) : comportement TLS et mTLS.
- [`AUTH.md`](AUTH.md) : modes d'authentification pris en charge.
- [`BUILD.md`](BUILD.md) : compilation depuis les sources et vérification des versions.
- [`DECK.md`](DECK.md) : présentation PowerPoint facultative de synthèse.

## Licence

Apache-2.0 OR MIT.
