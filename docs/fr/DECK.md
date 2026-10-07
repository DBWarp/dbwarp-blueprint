# Présentation visuelle de synthèse

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../DECK.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../DECK.md) | [Deutsch](../de/DECK.md) | **Français** | [Español](../es/DECK.md) | [Polski](../pl/DECK.md) | [日本語](../ja/DECK.md) | [简体中文](../zh/DECK.md)

`dbwarp-blueprint --deck blueprint.pptx` écrit une synthèse PowerPoint (`.pptx`) facultative du Blueprint, à côté du fichier TOML indiqué par `--out`. `dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx` crée ultérieurement la même présentation depuis un fichier Blueprint existant et vérifié, sans se connecter à une base de données. Il s'agit d'une présentation des mêmes données anonymisées : rien de plus n'est lu depuis votre base de données ni envoyé vers celle-ci. La présentation calcule uniquement les synthèses et projections locales documentées à partir de champs déjà présents dans le Blueprint.

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

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx \
  --lang ja
```

`--lang en|de|fr|es|pl|ja|zh` localise le texte de la présentation destiné aux humains ainsi que les métadonnées de langue PowerPoint. Les identifiants anonymes, les noms de types de base de données, les méthodes d'index, les mesures et le TOML source restent canoniques et indépendants de la langue. Si une phrase de la présentation manque, la validation du catalogue refuse de poursuivre (`fail closed`) au lieu de lui substituer silencieusement l'anglais. Consultez [`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

Chaque diapositive comporte également des notes pour le présentateur, adaptées à la langue locale. Ces notes transforment les informations présentées en un résumé concis et en langage naturel, plutôt que de répéter chaque étiquette et chaque valeur affichées sur la diapositive. Elles utilisent uniquement les mesures provenant de Blueprint et omettent le pied de page répété, de sorte que la vue du présentateur et les notes distribuées ajoutent une interprétation sans introduire de nouvelles informations.

## Pied de page et confidentialité

Chaque diapositive de contenu comporte le même pied de page : un petit logo à gauche, un séparateur facultatif et le niveau de confidentialité, un numéro de diapositive centré et `DBWarp.com` à droite. La diapositive de titre ne comporte pas de numéro.

Utilisez `--deck-confidentiality public|internal|confidential|restricted` pour
ajouter l'une des étiquettes de classification intégrées et localisées. Toute
autre valeur sûre et non vide devient une étiquette personnalisée affichée à
l'identique ; placez entre guillemets les valeurs contenant des espaces, par
exemple `--deck-confidentiality "CLIENT // SENSITIVE"`. Une étiquette ne peut
comporter ni espace initial ou final, ni caractère de contrôle ou de formatage
bidirectionnel, et ne peut dépasser 48 unités de largeur d'affichage. Omettez
cette option pour ne pas afficher d'étiquette. Ce réglage modifie uniquement la
présentation ; il ne change ni le fichier Blueprint ni les données résumées dans
le diaporama. Avec exactement le même Blueprint vérifié, la même langue, la
même étiquette et le même horodatage, les octets de la présentation sont
reproductibles.

## Propriétés de confiance

- **Créée localement, depuis la mémoire.** La présentation est rendue à partir du même Blueprint en mémoire qui produit `blueprint.toml`. Il n'y a ni requête supplémentaire à la base de données ni second parcours du catalogue. En mode `--from-toml`, le Blueprint en mémoire est chargé depuis le fichier TOML vérifié.
- **Aucun réseau applicatif.** La génération de la présentation n'ouvre aucune connexion réseau ; la lecture d'un Blueprint sur un chemin monté en réseau reste soumise à la pile de stockage de l'hôte.
- **Aucune bibliothèque tierce.** Le générateur OOXML est implémenté dans
  `src/deck.rs` et ses modules `deck_*`. Le fichier `.pptx` est une archive ZIP
  composée de parties XML que vous pouvez examiner avec `unzip`. Il n'utilise
  ni automatisation PowerPoint, ni service de rendu, ni dépendance
  supplémentaire. Les images du logo DBWarp et les polices DM Sans statiques
  sont intégrées dans le binaire Rust et écrites sous forme de parties OOXML
  media/font ; la génération ne lit pas de chemin d'accès à un module
  d'exécution.
- **Aucun identifiant réel, aucune donnée de ligne.** Les tables, colonnes et index apparaissent avec les mêmes identifiants anonymes que dans le fichier Blueprint (`table-001`, `col-1`, `idx-1`, `schema-A`). Les mesures sources conservent leur précision documentée ; toute projection est calculée uniquement à partir des champs déjà présents dans le Blueprint. La présentation ne contient rien de spécifique à votre base de données au-delà de cette entrée.
- **Reproductible à partir d'une entrée figée.** Un même Blueprint vérifié produit un fichier `.pptx` identique octet pour octet pour la même langue, la même étiquette de confidentialité et le même horodatage figé (ordre des parties et horodatages fixes). Cela ne rend pas deux captures en direct identiques : elles exigent aussi le même fichier protégé `--anonymization-key-file`, le même état de la source et les mêmes options de capture.

## Contenu

La présentation s'adapte à la taille du schéma :

- **Titre** : logo et slogan DBWarp, moteur, version, nature de la source, nombre de tables et horodatage de génération.
- **Synthèse exécutive** : signaux destinés au management sur l'ampleur de la migration, la concentration des données, la complexité relationnelle et les éléments de preuve à examiner.
- **Vue d'ensemble** : totaux des tables, lignes, tailles de données et tailles d'index, ainsi que nombres de colonnes, d'index, de clés étrangères et de schémas.
- **Petits schémas** (quelques tables) : un panneau dimensionné par table (lignes, octets, types de colonnes, index) et un diagramme des clés étrangères.
- **Grands schémas** : caractérisation plutôt qu'énumération :
  - *Tables les plus volumineuses* : principales tables par taille, avec un reste `+ N more`.
  - *Composition du schéma* : distribution des types de colonnes et statistiques sur les index et les totaux.
  - *Relations* : nombre de clés étrangères, tables connectées ou autonomes et tables les plus référencées (hubs).
- **Compression mesurée** (Tier 2 uniquement) : nombre de tables échantillonnées, ratio zstd-3 pondéré, empreinte compressée projetée et tables échantillonnées les plus compressibles.
- **Logique de base de données au-delà des tables.** Les objets non tabulaires (`graph` et `analyzed` détaillent uniquement) : six groupes en langage clair expliquent la logique et les dépendances de la base de données en dehors des définitions de tables ordinaires : les couches de requêtes, la logique exécutable, les comportements automatiques, les types et les objets producteurs de valeurs, les dépendances externes et la configuration de la plateforme. Les nombres affichés proviennent de l'inventaire des artefacts Blueprint.
- **Complexité des artefacts** (détails `graph` et `analyzed` uniquement) : catégorie agrégée de complexité des objets non tabulaires, couverture de la population d’objets éligibles et catégories des sept dimensions. La diapositive définit ces dimensions comme la taille des définitions, les branchements, l’utilisation des fonctionnalités, les dépendances, les exigences de l’environnement, le code source masqué et les comportements propres au dialecte. Chaque dimension affiche sa couverture à côté de sa catégorie afin que des preuves partielles ou inconnues ne puissent pas être confondues avec une faible complexité. Un résultat global `unknown` signifie que les preuves sont incomplètes ; `not applicable` exige une population d’objets éligibles vide et dont l’exhaustivité est démontrée.
- **Modèle de confiance** : diapositive finale résumant les propriétés de confiance ci-dessus.

## Vérifier la sortie

Le fichier `.pptx` est un paquet OOXML standard. Pour auditer exactement son contenu :

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

Ouvrez-le dans PowerPoint, LibreOffice Impress ou Google Slides. Le module de création de présentations est [`src/deck.rs`](../../src/deck.rs) et est intégré au binaire Rust. Il n'existe pas d'outil distinct de création de présentations à installer ou à auditer.
