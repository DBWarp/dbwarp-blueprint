# Compiler dbwarp-blueprint depuis les sources

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../BUILD.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../BUILD.md) | [Deutsch](../de/BUILD.md) | **Français** | [Español](../es/BUILD.md) | [Polski](../pl/BUILD.md) | [日本語](../ja/BUILD.md) | [简体中文](../zh/BUILD.md)

Ce guide s'adresse aux clients qui préfèrent compiler eux-mêmes l'outil avant de l'exécuter sur une base de données.

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

Fichiers de version :

| Plateforme | Fichier |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## Vérifier une archive téléchargée

Linux/macOS :

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell :

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

## Compilations propres aux modes d'authentification

La compilation par défaut prend en charge les flux par mot de passe, fichier de
jeton, variable d'environnement de jeton et TLS ; le mTLS par certificat client
est disponible pour PostgreSQL et MySQL.

L'authentification intégrée SQL Server dispose de compilations propres à chaque plateforme :

| Plateforme | Commande de compilation | Rôle |
|---|---|---|
| Linux | `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Kerberos / GSSAPI |
| Windows | Binaire Windows de la version GitHub, ou `cargo build --release --features winauth` | Windows Integrated Auth / SSPI |

Sous Linux, Kerberos nécessite les bibliothèques d'exécution MIT Kerberos habituelles. Si `kinit` fonctionne sur l'hôte, les composants d'exécution requis sont généralement déjà présents.

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

## Dépendances embarquées

Le dépôt normal contient une petite dépendance corrigée sous `vendor/mysql_async`, afin que l'option MySQL `--tls-ca` applique les mêmes règles restrictives de confiance que le reste de l'outil. Les versions de toutes les autres dépendances sont figées par `Cargo.lock`.

Chaque version GitHub publie un bundle `dbwarp-blueprint-source-vendored.tar.gz` distinct pour les équipes de sécurité qui souhaitent examiner et compiler hors ligne tous les fichiers source des dépendances.

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Ce bundle contient le correctif `vendor/mysql_async`, une arborescence `vendor-crates/` générée pour toutes les autres dépendances et un fichier `.cargo/config.toml` généré qui redirige crates.io vers l'arborescence locale des dépendances. Dans ce mode, `build.sh` utilise `cargo build --release --frozen --offline --locked`.
