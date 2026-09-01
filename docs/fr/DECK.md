# Présentation visuelle de synthèse

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../DECK.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../DECK.md) | [Deutsch](../de/DECK.md) | **Français** | [Español](../es/DECK.md) | [Polski](../pl/DECK.md) | [日本語](../ja/DECK.md) | [简体中文](../zh/DECK.md)

`dbwarp-blueprint --deck blueprint.pptx` écrit une synthèse PowerPoint (`.pptx`) facultative du Blueprint, à côté du fichier TOML indiqué par `--out`. `dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx` crée ultérieurement la même présentation depuis un fichier Blueprint existant et vérifié, sans se connecter à une base de données. Il s'agit d'une présentation des mêmes données anonymisées : rien de plus n'est lu, envoyé ou calculé au sujet de votre base de données.

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

## Pied de page et confidentialité

Chaque diapositive de contenu utilise le pied de page DBWarp : un petit logo à
gauche, un séparateur et un niveau de confidentialité facultatifs, un numéro de
diapositive seul et centré, puis `DBWarp.com` à droite. La diapositive de titre
reste sans numéro.

Utilisez `--deck-confidentiality public|internal|confidential|restricted` pour
ajouter l'une des étiquettes de classification intégrées et localisées. Toute
autre valeur sûre et non vide devient une étiquette personnalisée affichée à
l'identique ; placez entre guillemets les valeurs contenant des espaces, par
exemple `--deck-confidentiality "CLIENT // SENSITIVE"`. Une étiquette ne peut
comporter ni espace initial ou final, ni caractère de contrôle ou de formatage
bidirectionnel, et ne peut dépasser 48 unités de largeur d'affichage. Omettez
cette option pour ne pas afficher d'étiquette. Ce réglage modifie uniquement la
présentation ; il ne change ni le fichier Blueprint ni les données résumées dans
le diaporama, et la sortie reste déterministe lorsque `--generated-at` est fixé.

## Propriétés de confiance

- **Créée localement, depuis la mémoire.** La présentation est rendue à partir du même Blueprint en mémoire qui produit `blueprint.toml`. Il n'y a ni requête supplémentaire à la base de données ni second parcours du catalogue. En mode `--from-toml`, le Blueprint en mémoire est chargé depuis le fichier TOML vérifié.
- **Aucun réseau.** La génération de la présentation n'établit aucune connexion sortante.
- **Aucune bibliothèque tierce.** Le format OOXML est produit directement dans [`src/deck.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/deck.rs) ; le fichier `.pptx` est une simple archive ZIP de parties XML que vous pouvez ouvrir avec `unzip` et lire. Aucune automatisation PowerPoint, aucun service de rendu, aucune dépendance supplémentaire dans le graphe de dépendances. Les images de logo DBWarp approuvées et les polices statiques DM Sans sont intégrées au binaire Rust et écrites comme parties média et police OOXML ; la génération ne lit aucun chemin d'actif à l'exécution.
- **Aucun identifiant réel, aucune donnée de ligne.** Les tables, colonnes et index apparaissent avec les mêmes identifiants anonymes que dans le fichier Blueprint (`table-001`, `col-1`, `idx-1`, `schema-A`), et chaque nombre utilise la même précision documentée. La présentation ne contient aucun fait propre au client au-delà de ceux du fichier Blueprint.
- **Déterministe.** Avec une valeur `--generated-at` figée, un même Blueprint produit un fichier `.pptx` identique octet pour octet pour une même langue sélectionnée (ordre des parties fixe, horodatages fixes).

## Contenu

La présentation s'adapte à la taille du schéma :

- **Titre** : logo et slogan DBWarp, moteur, version, nature de la source, nombre de tables et horodatage de génération.
- **Synthèse exécutive** : signaux destinés au management sur l'ampleur de la migration, la concentration des données, la complexité relationnelle et les preuves prêtes à être partagées.
- **Vue d'ensemble** : totaux des tables, lignes, tailles de données et tailles d'index, ainsi que nombres de colonnes, d'index, de clés étrangères et de schémas.
- **Petits schémas** (quelques tables) : un panneau dimensionné par table (lignes, octets, types de colonnes, index) et un diagramme des clés étrangères.
- **Grands schémas** : caractérisation plutôt qu'énumération :
  - *Tables les plus volumineuses* : principales tables par taille, avec un reste `+ N more`.
  - *Composition du schéma* : distribution des types de colonnes et statistiques sur les index et les totaux.
  - *Relations* : nombre de clés étrangères, tables connectées ou autonomes et tables les plus référencées (hubs).
- **Compression mesurée** (Tier 2 uniquement) : nombre de tables échantillonnées, ratio zstd-3 pondéré, empreinte compressée projetée et tables échantillonnées les plus compressibles.
- **Modèle de confiance** : diapositive finale résumant les garanties ci-dessus.

## Vérifier la sortie

Le fichier `.pptx` est un paquet OOXML standard. Pour auditer exactement son contenu :

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

Ouvrez-le dans PowerPoint, LibreOffice Impress ou Google Slides. Le générateur se trouve dans [`src/deck.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/deck.rs) et est intégré au binaire Rust. Il n'existe aucun générateur de présentation distinct à installer, auditer ou maintenir synchronisé.
