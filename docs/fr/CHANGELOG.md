# Historique des modifications

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../CHANGELOG.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../CHANGELOG.md) | [Deutsch](../de/CHANGELOG.md) | **Français** | [Español](../es/CHANGELOG.md) | [Polski](../pl/CHANGELOG.md) | [日本語](../ja/CHANGELOG.md) | [简体中文](../zh/CHANGELOG.md)

Les numéros de version identifient le collecteur. La version du schéma
Blueprint et l’encodage des échantillons de compression sont des contrats de
compatibilité distincts ; consultez [FORMAT.md](FORMAT.md) et la
[mesure de la compression](COMPRESSION_MEASUREMENT.md).

## 1.5.1

### Capture et fidélité

- Maintien du schéma Blueprint v6 et distinction entre estimations du
  catalogue, observations échantillonnées et preuves de fraîcheur des
  statistiques indisponibles.
- Amélioration de la mesure des charges utiles binaires, du profilage des
  charges utiles déjà compressées et des sondes de compression bornées. Les
  ratios associés à des valeurs `sample_encoding` différentes ne sont pas
  interchangeables ; les consommateurs doivent reconnaître l’encodage avant
  d’utiliser une mesure.
- Nouvelles tentatives adaptatives d’échantillonnage MySQL et SQL Server
  bornées, y compris pour les valeurs surdimensionnées et l’expansion liée au
  jeu de caractères. Le biais de préfixe restant est consigné tout en
  conservant les métadonnées de longueur d’origine des valeurs échantillonnées.
- Correction de la détection de troncature MySQL lorsque le jeu de caractères
  de la connexion modifie la longueur en octets renvoyée.
- Amélioration de la gestion des longueurs de valeurs synthétiques, de la
  cardinalité, de la distribution et de la localité dans le cœur Blueprint
  partagé.

### Exploitation et revue

- Clarification de la configuration des comptes dédiés à privilèges minimaux,
  des instructions de compilation depuis les sources, de la vérification des
  compilations comparatives et de la matrice des versions de bases qualifiées.
- Actualisation des documents traduits automatiquement et des formulations à
  l’exécution ; l’anglais reste la référence et les traductions restent
  complémentaires.
- Renforcement des contrôles des droits d’exécution des sources publiques et
  des archives de version.
- Ajout de l’historique des versions et des consignes d’assistance et de
  contribution aux distributions de sources et de binaires.
- Suppression des illustrations ASCII inutilisées sans modification des modes
  de bannière pris en charge.

### Compatibilité et déploiement

Les Blueprints existants restent lisibles par le nouveau collecteur. L’inverse
n’est pas garanti : le lecteur de la version 1.5.0 rejette le nouveau champ
facultatif `sample_layout`, et les consommateurs plus anciens peuvent rejeter
les nouveaux encodages d’échantillons de compression. Un parseur du schéma v6
ne prouve donc pas à lui seul la compatibilité avec les versions futures.
Mettez à niveau et validez les outils consommateurs en même temps que le
collecteur avant d’utiliser de nouvelles captures pour des présentations, la
génération de données ou la planification fondée sur la compression.

Figez l’artefact de version exact et sa somme de contrôle. La qualification
d’une version précédente ne prouve pas qu’un autre binaire produit des
résultats identiques.

## 1.5.0

La version précédente fournit la capture en schéma v6 pour PostgreSQL, MySQL et
SQL Server, l’inspection de fichiers structurés, la sortie locale de Blueprints
et de présentations, ainsi que des scripts d’octroi de droits tenant compte
des versions. Consultez le
[tag de version](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0)
pour les sources et artefacts exacts.

Pour signaler un problème, consultez [SUPPORT.md](SUPPORT.md). Pour les
consignes de contribution, consultez [CONTRIBUTING.md](CONTRIBUTING.md).
