# Compiler dbwarp-blueprint depuis les sources

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../BUILD.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../BUILD.md) | [Deutsch](../de/BUILD.md) | **Français** | [Español](../es/BUILD.md) | [Polski](../pl/BUILD.md) | [日本語](../ja/BUILD.md) | [简体中文](../zh/BUILD.md)

Utilisez ce guide si vous préférez compiler vous-même l'outil avant de l'exécuter sur une base de données.

## Compilation rapide

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
./build.sh
```

Le binaire est écrit dans :

```text
target/release/dbwarp-blueprint
```

Tous les autres exemples utilisent `./dbwarp-blueprint`. Après une compilation
depuis les sources, exécutez directement `target/release/dbwarp-blueprint` ou
copiez ce fichier vers `./dbwarp-blueprint` avant de les suivre.

Si la chaîne Rust épinglée n'est pas installée et qu'un accès réseau examiné est
autorisé, donnez votre accord explicitement :

```bash
ALLOW_NETWORK=1 ./build.sh
```

## Fonctionnement du script de compilation

`build.sh` est volontairement prudent :

- il lit la version de Rust figée dans `rust-toolchain.toml` ;
- il utilise votre `rustc` existant s'il correspond à la version figée ;
- il refuse de télécharger Rust sauf si `ALLOW_NETWORK=1` est défini ;
- il fige la version d'amorçage de rustup et vérifie son SHA-256 officiel avant utilisation ;
- il conserve l'état de la chaîne d'outils sous `./build/` ;
- il utilise Cargo.lock pour obtenir des versions de dépendances reproductibles ;
- il compile par défaut avec `cargo build --release --locked` ;
- il passe automatiquement à `--frozen --offline --locked` lorsqu'il est exécuté depuis un bundle de sources contenant les dépendances ;
- il refuse `DBWARP_BLUEPRINT_OFFLINE=1` si `vendor-crates/` est absent ;
- il affiche le SHA256 du binaire obtenu ;
- il inscrit dans l'audit la révision source exacte et l'état de l'arbre de travail.

Il n'utilise pas `sudo` et ne modifie pas votre installation système de Rust.

## Binaires téléchargeables

Des binaires précompilés sont disponibles sur la page Releases :

<https://github.com/DBWarp/dbwarp-blueprint/releases>

Ils sont fournis par commodité. Figez un tag de version exact et vérifiez son SHA-256 avant utilisation ; n'utilisez pas d'URL de téléchargement modifiable pour une exécution reproductible. Si votre politique impose une revue des sources, compilez localement depuis le même tag.

Les archives binaires de plateforme sont des bundles opérateur, pas des
arborescences de sources, et ne peuvent pas être reconstruites sur place. Leur
copie de ce guide et `verify.sh` décrit le parcours de vérification avec les
sources correspondantes. Utilisez une copie du tag de version exact ou
l'archive de sources avec dépendances de la version lorsque vous avez besoin de
`build.sh`, des sources Cargo ou d'une compilation locale de comparaison.

Fichiers de version :

| Plateforme | Fichier |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## Vérifier une archive téléchargée

Linux :

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS :

```bash
shasum -a 256 dbwarp-blueprint-macos-arm64.tar.gz
```

Comparez la valeur affichée à la ligne correspondante de `SHA256SUMS.txt`.

Windows PowerShell :

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Chaque version publie également un fichier
`dbwarp-blueprint-<platform>.binary.sha256` pour l’exécutable extrait. Consultez
[Télécharger les binaires](BINARIES.md) pour la commande de vérification.

## Compilations propres aux modes d'authentification

La compilation par défaut prend en charge les flux par mot de passe, fichier de
jeton, variable d'environnement de jeton et TLS ; le mTLS par certificat client
est disponible pour PostgreSQL et MySQL.

L'authentification intégrée SQL Server est prise en charge selon la plateforme :

| Plateforme | Commande de compilation | Rôle |
|---|---|---|
| Linux | Binaire Linux de la version GitHub, ou `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Authentification par mot de passe, jeton et TLS, plus Kerberos / GSSAPI lorsqu'il est sélectionné |
| Windows | Binaire Windows de la version GitHub, ou `cargo build --release --locked --features winauth` | Windows Integrated Auth / SSPI |

Les binaires de publication Linux ne nécessitent pas les bibliothèques Kerberos
au démarrage. Ils chargent l'environnement d'exécution GSSAPI de la plateforme
uniquement lorsque `--auth-mode integrated` est sélectionné. Si `kinit`
fonctionne, les composants d'exécution requis sont généralement déjà présents.
Les compilations à partir du code source activent Kerberos/GSSAPI avec
`integrated-auth-gssapi`, comme indiqué ci-dessus.

## Compiler sans le script

Si votre politique préfère les commandes Cargo directes :

```bash
cargo build --release --locked
```

Compilation SSPI sous Windows :

```powershell
cargo build --release --locked --features winauth
```

Compilation Kerberos sous Linux :

```bash
cargo build --release --locked --features integrated-auth-gssapi
```

## Reproduire un binaire de release

`./build.sh` prouve que la source analysée est correctement compilée ; l'identité des octets nécessite en outre les entrées de compilation natives complètes de la version. Consultez la révision exacte de la source enregistrée dans `PROVENANCE.json`, publiée avec chaque version, utilisez sa liste de cibles et de fonctionnalités, la chaîne d'outils Rust spécifiée, les entrées natives enregistrées compiler/linker, l'horodatage du commit `SOURCE_DATE_EPOCH`, ainsi que les rémappages de chemin et les indicateurs du linker du flux de travail de la version. Les versions Windows utilisent également `clang-cl` et `/Brepro`.

Après avoir reproduit ces paramètres, comparez le binaire extrait au résultat local :

```bash
SOURCE_BIN=target/release/dbwarp-blueprint \
  ./verify.sh /path/to/extracted/dbwarp-blueprint
```

Si les hachages sont différents, ne considérez pas les binaires comme équivalents. Chaque version est construite deux fois, et une différence de byte entraîne l'échec de la version. `PROVENANCE.json` enregistre la révision source, la cible, les fonctionnalités, la chaîne d'outils, l'époque de la date source, le compilateur natif, la taille du binaire et le hachage nécessaires pour évaluer une reproduction locale.

## Dépendances embarquées

Le dépôt contient des dépendances corrigées sous `vendor/`. Elles préservent les règles de confiance restrictives de `--tls-ca` pour MySQL et SQL Server. L'authentification intégrée Linux charge GSSAPI uniquement lorsqu'elle est demandée. L'authentification intégrée Windows utilise une dépendance maintenue pour la génération de nombres aléatoires. Les versions de toutes les autres dépendances sont figées par `Cargo.lock`.

Chaque version GitHub publie un bundle `dbwarp-blueprint-source-vendored.tar.gz` distinct pour les équipes de sécurité qui souhaitent examiner et compiler hors ligne tous les fichiers source des dépendances.

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Ce bundle contient les dépendances corrigées sous `vendor/`, une arborescence `vendor-crates/` générée pour toutes les autres dépendances et un fichier `.cargo/config.toml` généré qui redirige crates.io vers l'arborescence locale des dépendances. Dans ce mode, `build.sh` utilise `cargo build --release --frozen --offline --locked`.
