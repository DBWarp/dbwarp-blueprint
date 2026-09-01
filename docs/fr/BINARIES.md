# Télécharger dbwarp-blueprint

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../binaries/README.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../binaries/README.md) | [Deutsch](../de/BINARIES.md) | **Français** | [Español](../es/BINARIES.md) | [Polski](../pl/BINARIES.md) | [日本語](../ja/BINARIES.md) | [简体中文](../zh/BINARIES.md)

Les binaires précompilés de `dbwarp-blueprint` sont publiés sur la page GitHub Releases :

<https://github.com/DBWarp/dbwarp-blueprint/releases>

Vous pouvez télécharger un binaire, vérifier sa somme de contrôle, l'exécuter localement et examiner le fichier `blueprint.toml` généré avant de partager quoi que ce soit avec DBWarp.

Choisissez une balise de version exacte, par exemple `https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0`, puis téléchargez l'archive et `SHA256SUMS.txt` depuis cette même balise. N'utilisez pas une URL mutable `releases/latest` pour une exécution reproductible ou auditée.

## Fichiers

| Plateforme | Fichier |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |
| Bundle de sources hors ligne pour audit | `dbwarp-blueprint-source-vendored.tar.gz` |
| Sommes de contrôle | `SHA256SUMS.txt` |

Chaque version inclut également `SHA256SUMS.txt`.

## Vérifier le téléchargement

Linux/macOS :

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell :

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Comparez le hachage affiché avec la ligne correspondante dans `SHA256SUMS.txt`.

## Binaire téléchargé ou compilation locale ?

Le binaire téléchargeable est fourni par commodité. La voie offrant la confiance la plus forte reste la compilation depuis les sources :

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
git checkout <release-tag>
./build.sh
```

Ce clone normal des sources est volontairement petit et utilise `Cargo.lock` pour figer les versions des dépendances.

Si votre politique exige d'examiner chaque fichier source des dépendances avant la compilation, téléchargez `dbwarp-blueprint-source-vendored.tar.gz` depuis la même version et compilez dans l'arborescence extraite :

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Consultez [`BUILD.md`](BUILD.md).

## Fonctionnement de l'outil

`dbwarp-blueprint` lit les métadonnées de la base de données et mesure éventuellement la compression sur un petit échantillon local. Il écrit un fichier texte anonymisé destiné à l'estimation d'une migration DBWarp. Il ne téléverse pas le fichier et n'envoie aucune télémétrie.
