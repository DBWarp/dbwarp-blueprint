# Démarrage rapide

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../QUICKSTART.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | **Français** | [Español](../es/QUICKSTART.md) | [Polski](../pl/QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | [简体中文](../zh/QUICKSTART.md)

Ce guide de démarrage rapide s'adresse aux ingénieurs commerciaux, DBA ou responsables de la sécurité qui doivent produire un fichier Blueprint DBWarp partageable sans exposer les données du client.

## 1. Choisir comment exécuter l'outil

Utilisez l'une des méthodes suivantes :

- Téléchargez un binaire de version et vérifiez sa somme de contrôle.
- Compilez depuis les sources avec `./build.sh`.
- Compilez depuis le bundle de version contenant les dépendances pour une revue stricte et hors ligne de celles-ci.

Consultez [`../BUILD.md`](BUILD.md) et [`../binaries/README.md`](BINARIES.md).

Sélectionnez explicitement une langue de présentation lorsque cela est nécessaire :

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --dry-run
```

Les valeurs prises en charge sont `en`, `de`, `fr`, `es`, `pl`, `ja` et `zh`.
La langue de présentation modifie l'aide, les demandes, les diagnostics, le
texte de progression et le texte de la présentation. Elle ne modifie jamais les
noms d'options, les valeurs acceptées, les schémas d'URI, les sélecteurs, les
codes DBP, les clés d'audit ou le TOML Blueprint. Consultez
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Préparer les informations d'identification en toute sécurité

Ne placez pas de mot de passe dans l'URI de connexion. L'outil refuse les mots de passe intégrés à l'URI pour éviter leur divulgation dans la liste des processus et l'historique du shell.

Modèle recommandé avec fichier de mot de passe (le secret est saisi sans écho et n'apparaît pas dans l'historique du shell) :

```bash
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

Si le nom d'utilisateur est difficile à encoder dans une URI, placez-le également dans un fichier :

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\\migration_user' > /etc/dbwarp/db.user
```

Utilisez ensuite `--user-file /etc/dbwarp/db.user`.

## 3. Commencer par une simulation

Une simulation valide les arguments et affiche l'action prévue sans établir de connexion :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

Pour le mode de présentation `--from-toml`, la simulation est une vérification préalable locale et ne lit pas la base de données.

Pour plusieurs sources client, simulez plutôt le manifeste de lot :

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 4. Exécuter le mode catalogue uniquement

Le mode catalogue uniquement lit les métadonnées et les statistiques, mais aucun échantillon de ligne :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

Utilisez ce mode lorsqu'une politique interdit l'échantillonnage de lignes ou lorsque vous souhaitez effectuer une première revue de sécurité.

## 5. Choisir le niveau de détail des artefacts hors tables

Par défaut, `--artifact-detail summary` lit les catalogues hors tables mais pas les définitions d'objets. Il émet des comptages bornés et des classes de prérequis externes. Utilisez `--artifact-detail none` si la politique interdit ces catalogues.

Pour une topologie de dépendances anonyme, utilisez `graph`. Pour des bandes bornées de caractéristiques du langage et de complexité, utilisez `analyzed`. Tous deux exigent un consentement explicite :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


La sortie ne contient jamais de noms d'objets, texte de définition, points de terminaison, secrets, clés, certificats ou binaires. Consultez [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) avant d'approuver le mode graph ou analyzed.

## 6. Exécuter la mesure de compression Tier 2

Le Tier 2 lit en mémoire des échantillons de lignes de taille limitée, les compresse localement, écrit uniquement des ratios récapitulatifs, puis supprime les octets échantillonnés :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log blueprint.audit.txt
```

Utilisez le Tier 2 lorsque cela est possible. Il permet à DBWarp de mieux estimer les octets transmis, le coût de sortie réseau et la génération de données texte/binaires synthétiques.

## 7. Générer une présentation

Pendant l'exécution sur la base active :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --audit-log blueprint.audit.txt \
  --yes
```

Ou après la revue, sans connexion à la base de données :

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

## 8. Vérifier avant de partager

Vérifiez :

```bash
less blueprint.toml
less blueprint.audit.txt
unzip -l blueprint.pptx  # optional deck package inspection
```

Propriétés attendues :

- aucun nom réel de table ;
- aucun nom réel de colonne ;
- aucune valeur de ligne ;
- aucun commentaire hormis l'en-tête fixe ;
- nombres et tailles en octets arrondis ;
- identifiants anonymisés tels que `table-001`, `col-1` et `schema-A` ;
- comptages d'artefacts bornés et, après approbation, identifiants d'artefacts anonymes ;
- preuves explicites d'artefacts incomplets ou illisibles plutôt qu'une omission silencieuse ;
- uniquement des ratios de compression facultatifs, et non les octets échantillonnés.

## 9. Transmettre à DBWarp

Transmission minimale :

```text
blueprint.toml
```

Pour la revue d'un client comportant plusieurs sources, créez et vérifiez un bundle empaqueté au lieu de transmettre le répertoire de travail :

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Les métadonnées du bundle conservent les identifiants de source, les tags et les identifiants de groupe de jeux de données choisis dans le manifeste batch. Utilisez des valeurs anonymes et vérifiez-les avant le transfert.

Utilisez `docs/BATCH_AND_BUNDLES.md` lorsque le client possède plusieurs bases de données, plusieurs jeux de données Parquet ou Avro, ou souhaite n'approuver que certaines sources/tables pour la génération du benchmark.

Conservez ces éléments comme preuves locales à accès contrôlé par défaut :

```text
blueprint.audit.txt
blueprint.pptx
command-used.txt
```

Les audits et les commandes enregistrées peuvent contenir des points de terminaison de base de données, des identités authentifiées, des chemins locaux, des données de temps et des identifiants de source du manifeste. Ne les envoyez que pour un besoin de support précis via un canal sécurisé approuvé. N'envoyez pas les fichiers de mot de passe, les clés privées d'autorité de certification, les dumps du client ou les journaux de base de données.
