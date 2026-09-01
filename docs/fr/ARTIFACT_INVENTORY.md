# Inventaire des artefacts hors table

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../ARTIFACT_INVENTORY.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../ARTIFACT_INVENTORY.md) | [Deutsch](../de/ARTIFACT_INVENTORY.md) |
**Français** | [Español](../es/ARTIFACT_INVENTORY.md) |
[Polski](../pl/ARTIFACT_INVENTORY.md) | [日本語](../ja/ARTIFACT_INVENTORY.md) |
[简体中文](../zh/ARTIFACT_INVENTORY.md)

Depuis le schéma v4, les Blueprints décrivent les objets de base de données hors
table et les prérequis de déploiement sans publier leurs noms source,
définitions, chaînes de point de terminaison, secrets, certificats, clés ou
binaires. Cet inventaire aide DBWarp
à estimer la complexité d'une migration et à identifier les travaux nécessitant
des paquets, de l'infrastructure, une approbation de sécurité ou une conversion
assistée.

L'inventaire n'est pas une promesse de capacité. Le signalement d'un objet ne
signifie pas que DBWarp sait le recréer ou le traduire automatiquement. La
capacité de migration doit être vérifiée séparément dans la matrice des routes
et artefacts de DBWarp.

## Niveaux de détail

Utilisez `--artifact-detail` pour choisir le compromis entre confidentialité et
planification :

| Valeur | Lectures en base | Sortie Blueprint | Consentement |
|---|---|---|---|
| `none` | Aucun catalogue ni définition d'artefact | Aucun compteur ni graphe | Aucun consentement supplémentaire |
| `summary` | Catalogues d'artefacts, sans définitions | Compteurs par type et classe de prérequis externe | Valeur par défaut ; aucun consentement supplémentaire |
| `graph` | Catalogues et métadonnées de dépendance, sans définitions | Compteurs, objets anonymes stables et arêtes | Nécessite `--yes` |
| `analyzed` | Catalogues, dépendances et définitions disponibles | Graphe et classes bornées de fonctions linguistiques et de complexité | Nécessite `--yes` |

La valeur par défaut est `summary`. Utilisez `none` si la politique autorise la
structure des tables mais interdit les catalogues hors table. Utilisez `graph` pour
une planification par dépendances sans lire les définitions, et `analyzed`
uniquement après approbation de leur lecture transitoire.

```bash
./dbwarp-blueprint \
  --connect postgresql://blueprint_user@db.internal/appdb \
  --password-file /etc/dbwarp/blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --artifact-detail analyzed \
  --out appdb.blueprint.toml \
  --audit-log appdb.blueprint.audit.txt \
  --yes
```

## Contrat de confidentialité

La sortie d'artefacts contient uniquement des métadonnées bornées issues d'un
vocabulaire fermé :

- des identifiants anonymes stables comme `view-001`, `function-002` et `schema-A` ;
- des jetons fermés pour le type, sous-type, niveau, visibilité et mode de sécurité ;
- des dépendances exprimées uniquement par des identifiants anonymes d'artefact ou de table ;
- des compteurs et classes bornées plutôt que des descriptions libres ;
- des noms de catalogues standard tels que `pg_proc`, `information_schema.views` ou `sys.objects` ;
- des classes de prérequis externes, jamais leurs noms ni leur contenu.

La sortie ne contient pas les noms d'objets source, le texte source SQL ou
procédural, les noms de schéma, les principaux, les chaînes de point de
terminaison, les chaînes de fournisseur, les informations d'identification,
les clés, les corps de certificats, les fichiers d'assembly, les noms de paquets
d'extension ou les noms de bibliothèques chargeables.

En mode `analyzed`, les définitions ne restent en mémoire que le temps de
supprimer commentaires et littéraux et de produire des agrégats lexicaux
bornés. Elles sont détenues par un conteneur effacé à la libération et ne sont
ni sérialisées, ni journalisées, ni envoyées à un autre service. Il s'agit d'une
réduction de l'exposition mémoire, pas d'une garantie contre la pagination du
système ou un débogueur privilégié.

Même un graphe anonyme peut caractériser une application par ses comptes et sa
topologie. C'est pourquoi `graph` et `analyzed` échouent avec `DBP1014E` sans
`--yes` explicite.

## Preuves d'exhaustivité

Le bloc `[artifact_inventory]` est volontairement auto-auditable :

| Champ | Signification |
|---|---|
| `contract` | Contrat versionné indépendamment, actuellement `dbwarp-blueprint-artifacts/v1` |
| `detail` | Niveau de détail demandé |
| `visibility` | `full`, `privilege_filtered` ou `unknown` |
| `inventory_complete` | Vrai uniquement avec visibilité totale, aucun catalogue illisible et aucune famille non modélisée déclarée |
| `dependencies_complete` | Vrai uniquement si les sources de dépendances étaient lisibles et si les familles modélisées sont couvertes |
| `analysis_complete` | Vrai uniquement avec `analyzed` et une analyse complète de toutes les définitions disponibles |
| `catalogs_read` | Familles de catalogues standard inspectées avec succès |
| `catalogs_unreadable` | Familles de catalogues en échec ou indisponibles |
| `families_not_inventoried` | Familles connues hors du contrat actuel |

L'échec d'un catalogue optionnel ne supprime pas silencieusement des objets. Le
programme émet `DBP1410W`, enregistre le catalogue concerné et force les
indicateurs d'exhaustivité correspondants à faux. Un compte peu privilégié peut
donc fournir un inventaire partiel utile sans présenter l'absence comme preuve.

## Couverture par moteur

Le collecteur v1 modélise les familles suivantes :

| Moteur | Familles d'objets modélisées |
|---|---|
| PostgreSQL | vues, vues matérialisées, séquences, routines, agrégats, types enum/domain/composite/range, déclencheurs, valeurs par défaut, contrôles, politiques, règles, déclencheurs d'événements, extensions, tables/serveurs étrangers, publications, abonnements, espaces de tables et fonctions natives |
| MySQL | vues, fonctions et procédures stockées, déclencheurs, événements planifiés, dépendances de vues, tables FEDERATED et enregistrements UDF chargeables |
| SQL Server | vues, procédures stockées, fonctions scalaires/tabulaires, modules CLR, déclencheurs, valeurs par défaut, contrôles, règles, synonymes, séquences, types utilisateur, assemblies CLR, objets de données externes, catalogues de texte intégral, objets de partitionnement, groupes de fichiers non PRIMARY, certificats, clés, informations d'identification limitées à la base, serveurs liés et tâches SQL Server Agent |

Chaque Blueprint énumère les familles connues non modélisées. Un compteur nul ne
prouve pas l'absence tant que `visibility`, les indicateurs d'exhaustivité et la
liste des familles non inventoriées ne le permettent pas.

## Prérequis externes

Les objets dépendant de plus qu'un DDL de table portable portent une classe
anonyme de prérequis externe :

| Classe | Éléments à résoudre par l'opérateur |
|---|---|
| `postgresql_extension` | Paquet d'extension compatible et version cible |
| `postgresql_native_function` | Bibliothèque native et compatibilité ABI |
| `mysql_loadable_udf` | Binaire UDF chargeable et hypothèses ABI du serveur source |
| `sqlserver_clr_assembly` | Activation CLR, assembly, environnement d'exécution et politique de confiance |
| `foreign_endpoint` | Réseau, fournisseur, base distante et authentification |
| `replication_topology` | Topologie publication/abonnement et politique cible |
| `physical_storage` | Conception des groupes de fichiers ou du placement physique |
| `server_feature` | Disponibilité d'une fonction serveur ou de service géré |
| `certificate_material` | Émission ou import de certificat selon la politique cible |
| `encryption_or_credential_material` | Clés, informations d'identification, magasin externe et gestion des secrets |
| `sqlserver_agent` | Disponibilité de l'agent, environnement et gouvernance des tâches |

Le Blueprint indique si un binaire, un secret ou un point de terminaison est requis
mais non capturé. Les objets externes doivent devenir des tâches explicites de
migration, jamais des omissions au mieux.

## Recensement des fonctions linguistiques

Le détail `analyzed` ajoute des blocs `dbwarp-language-feature-census/v1` aux
définitions SQL et procédurales disponibles. Le premier analyseur est
`lexical-v1` et indique `status = "partial"` ; ce n'est ni un analyseur
syntaxique, ni un compilateur, ni un lieur sémantique, ni une garantie de
traduction.

Il enregistre des classes bornées de taille, nombre d'instructions et de jetons,
imbrication, complexité cyclomatique et régions opaques/dynamiques. Un
vocabulaire fermé décrit flux de contrôle, jointures, sous-requêtes, CTE,
agrégats, fenêtres, DML, DDL, objets temporaires, SQL dynamique, JSON, XML,
spatial, vecteur et modes de sécurité. Le contexte moteur comprend le profil de
grammaire normalisé, les modes SQL MySQL et, pour SQL Server, la compatibilité,
`ANSI_NULLS` et `QUOTED_IDENTIFIER`.

L'analyseur lexical retire commentaires, littéraux et identifiants entre
délimiteurs. Des règles contextuelles couvrent les événements de déclencheur,
`EXECUTE FUNCTION` de PostgreSQL et les options de module SQL Server. Les
résultats restent des preuves grossières de planification. Un futur analyseur
fondé sur une grammaire pourra changer de version sans modifier le contrat
d'artefact externe.

## Processus de revue recommandé

1. Exécuter `summary` avec la revue de catalogue normale.
2. Examiner les compteurs, classes externes, visibilité, catalogues illisibles et familles non modélisées.
3. Approuver `graph` uniquement si la topologie anonyme est acceptable.
4. Approuver `analyzed` uniquement si la lecture transitoire des définitions est acceptable.
5. Conserver le journal d'audit localement comme preuve à accès contrôlé. Ne le partager que si un destinataire nommé a besoin des détails sur le point de terminaison, l'identité, les chemins et les dégradations via un canal sécurisé approuvé.
6. Comparer l'inventaire à la matrice de capacité DBWarp avant toute promesse de recréation ou traduction automatique.

Pour les champs sérialisés exacts, consultez la [Référence du format](FORMAT.md).
Pour les lectures, écritures, avertissements et assertions de confiance,
consultez la [Référence d'audit](AUDIT.md).
