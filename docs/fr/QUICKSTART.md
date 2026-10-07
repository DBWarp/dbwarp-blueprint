# Démarrage rapide

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../QUICKSTART.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | **Français** | [Español](../es/QUICKSTART.md) | [Polski](../pl/QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | [简体中文](../zh/QUICKSTART.md)

Ce guide de démarrage rapide est destiné à un administrateur de base de données ou à un responsable de la sécurité qui a besoin de créer un fichier DBWarp Blueprint partageable sans exposer de données.

## 1. Choisir comment exécuter l'outil

Utilisez l'une des méthodes suivantes :

- Téléchargez un binaire de version et vérifiez sa somme de contrôle.
- Compilez depuis les sources avec `./build.sh`.
- Compilez depuis le bundle de version contenant les dépendances pour une revue stricte et hors ligne de celles-ci.

Consultez [`../BUILD.md`](BUILD.md) et [`../binaries/README.md`](BINARIES.md).

Sélectionnez explicitement une langue de présentation lorsque cela est nécessaire :

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --schema app --dry-run
```

Les valeurs prises en charge sont `en`, `de`, `fr`, `es`, `pl`, `ja` et `zh`.
La langue de présentation modifie l'aide, les demandes, les diagnostics, le
texte de progression et le texte de la présentation. Elle ne modifie jamais les
noms d'options, les valeurs acceptées, les schémas d'URI, les sélecteurs, les
codes DBP, les clés d'audit ou le TOML Blueprint. Consultez
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Provisionner un compte dédié avec le minimum de privilèges

Effectuez cette étape avant toute connexion réelle, y compris avant les
exemples `--dry-run` destinés à devenir ensuite des captures. Ne commencez pas
avec un compte propriétaire de l'application, administrateur, superutilisateur,
`root`, `sa` ou `db_owner`.

1. Identifiez précisément le moteur et sa version, la base de données et les
   schémas approuvés.
2. Choisissez le niveau de capture : `basic` pour les catalogues de tables uniquement, `standard` pour ajouter un échantillon de lignes limité, ou `enhanced` pour l'analyse des objets non-table également.
3. Demandez au DBA de copier le script correspondant sous
   `sql/grants/<engine>/`, de modifier toutes les valeurs marquées de base de
   données, schéma, principal, mot de passe et option de rôle, puis de
   l'exécuter selon le processus normal de gestion des changements.
4. Utilisez le compte dédié créé par le script et passez la même portée
   approuvée avec une option `--schema NAME` par schéma dans chaque commande
   réelle.
5. Une fois la collecte examinée, demandez à l'administrateur de base de données (DBA) de vérifier et d'exécuter, sous `sql/revoke/`, le script de révocation correspondant au moteur sélectionné afin de supprimer le compte et les autorisations.

Les scripts distinguent volontairement les autorisations à portée précise des
rôles intégrés plus pratiques et expliquent lorsqu'un rôle est plus large.
Consultez [`../../sql/grants/README.md`](../../sql/grants/README.md) pour les
scripts exécutables et
[`../../sql/grants/DATABASE_PERMISSIONS.md`](../../sql/grants/DATABASE_PERMISSIONS.md)
pour la justification tenant compte des versions destinée au DBA et à la
sécurité. Le collecteur ne crée, n'élargit ni ne supprime lui-même aucun
principal de base de données.

## 3. Préparer les informations d'identification en toute sécurité

Ne placez pas de mot de passe dans l'URI de connexion. L'outil refuse les mots de passe intégrés à l'URI pour éviter leur divulgation dans la liste des processus et l'historique du shell.

Modèle recommandé avec fichier de mot de passe (le secret est saisi sans écho et n'apparaît pas dans l'historique du shell) :

```bash
sudo install -d -m 700 -o "$USER" -g "$(id -gn)" /etc/dbwarp
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

Si le nom d'utilisateur est difficile à encoder dans une URI, placez-le également dans un fichier :

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\migration_user' > /etc/dbwarp/db.user
```

Utilisez ensuite `--user-file /etc/dbwarp/db.user`.

## 4. Commencer par une simulation

Une simulation valide les arguments et affiche l'action prévue sans établir de connexion :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

Pour le mode de présentation `--from-toml`, la simulation est une vérification préalable locale et ne lit pas la base de données.

Pour plusieurs sources, effectuez un test préliminaire du manifeste par lots au lieu de l'exécuter directement :

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 5. Exécuter le mode catalogue uniquement

Le mode catalogue uniquement lit les métadonnées et les statistiques, mais aucun échantillon de ligne :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

Utilisez ce mode lorsqu'une politique interdit l'échantillonnage de lignes ou lorsque vous souhaitez effectuer une première revue de sécurité.

## 6. Choisir le niveau de détail des artefacts hors tables

Par défaut, `--artifact-detail summary` lit les catalogues hors tables mais pas les définitions d'objets. Il émet des comptages bornés et des classes de prérequis externes. Utilisez `--artifact-detail none` si la politique interdit ces catalogues. La sonde de topologie limitée au comptage est néanmoins exécutée ; consultez la [référence des privilèges](../../sql/grants/README.md#topology-evidence).

Pour une topologie de dépendances anonyme, utilisez `graph`. Pour des bandes bornées de caractéristiques du langage et de complexité, utilisez `analyzed`. Tous deux exigent un consentement explicite :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


La sortie ne contient jamais de noms d'objets, texte de définition, points de terminaison, secrets, clés, certificats ou binaires. Consultez [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) avant d'approuver le mode graph ou analyzed.

## 7. Exécuter la mesure de compression Tier 2

Le Tier 2 lit en mémoire des échantillons de lignes de taille limitée, calcule
des mesures agrégées de compression, densité de valeurs NULL,
cardinalité/fréquence, longueur et style, puis supprime les valeurs échantillonnées :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
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

Utilisez le niveau 2 lorsque cela est possible. Il fournit des estimations plus précises de la taille du transfert et des coûts de sortie.

## 8. Générer une présentation

Pendant l'exécution sur la base active :

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
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

## 9. Vérifier avant de partager

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
- des mesures agrégées facultatives de compression, densité de valeurs NULL,
  cardinalité/fréquence, longueur et style, jamais les valeurs échantillonnées.

## 10. Partagez avec DBWarp.

Minimum à partager :

```text
blueprint.toml
```

Pour plusieurs sources, créez et examinez un ensemble compressé plutôt que de partager le répertoire de travail :

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Les métadonnées du bundle conservent les identifiants de source, les tags et les identifiants de groupe de jeux de données choisis dans le manifeste batch. Utilisez des valeurs anonymes et vérifiez-les avant le transfert.

Consultez [les lots de collecte et les ensembles de plans](BATCH_AND_BUNDLES.md) si vous avez plusieurs bases de données ou plusieurs ensembles de données Parquet ou Avro, ou si vous souhaitez partager uniquement certaines sources ou tables.

### Vérifier et partager

Par défaut, partagez uniquement le `blueprint.toml` vérifié ou le bundle empaqueté. Une présentation ne peut l’accompagner qu’après vérification de son contenu et de son niveau de confidentialité, puis approbation distincte conformément à la politique de votre organisation.

Conservez les audits, les enregistrements de commandes et les présentations non approuvées localement et avec un accès contrôlé. Ils peuvent contenir des points de terminaison, des principaux authentifiés, des chemins locaux, des données de synchronisation et des identifiants de manifeste. Ne les envoyez que pour un besoin de support spécifique via un canal sécurisé approuvé. N'incluez jamais de fichiers de mot de passe ou de jetons, de clés d'anonymisation, de clés privées de CA, de sauvegardes de bases de données ou de journaux de bases de données avec un manifeste partagé.
