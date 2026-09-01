# Internationalisation

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../INTERNATIONALISATION.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../INTERNATIONALISATION.md) | [Deutsch](../de/INTERNATIONALISATION.md) | **Français** | [Español](../es/INTERNATIONALISATION.md) | [Polski](../pl/INTERNATIONALISATION.md) | [日本語](../ja/INTERNATIONALISATION.md) | [简体中文](../zh/INTERNATIONALISATION.md)

`dbwarp-blueprint` sépare la présentation destinée aux humains de la syntaxe
opérationnelle. Il s'agit d'une limite de sécurité et d'automatisation, et non
d'une simple préférence d'affichage.

## Langues prises en charge

Le texte source anglais fait foi. Les catalogues de présentation non anglais sont assistés par machine et peuvent contenir des erreurs même si leur couverture des clés et des tokens est validée. Comparez les décisions de sécurité, contractuelles, réglementaires et de moindre privilège au texte anglais. Consultez [`TRANSLATIONS.md`](../TRANSLATIONS.md) pour le processus distinct de publication des documents traduits.

| Valeur | Langue | Balise de paramètres régionaux utilisée dans les présentations générées |
|---|---|---|
| `en` | Anglais | `en-US` |
| `de` | Allemand | `de-DE` |
| `fr` | Français | `fr-FR` |
| `es` | Espagnol | `es-ES` |
| `pl` | Polonais | `pl-PL` |
| `ja` | Japonais | `ja-JP` |
| `zh` | Chinois simplifié | `zh-CN` |

Sélectionnez explicitement une langue :

```bash
dbwarp-blueprint --lang de --help
dbwarp-blueprint --lang ja --connect postgresql://db.internal/app --dry-run
```

Lorsque `--lang` est absent, l'ordre de résolution est le suivant :

1. `DBWARP_BLUEPRINT_LANG` ;
2. `LC_ALL` ;
3. `LC_MESSAGES` ;
4. `LANG` ;
5. anglais.

Les suffixes de région et d'encodage sont acceptés pour les balises de
paramètres régionaux d'environnement ; ainsi, `de_CH.UTF-8`, `pl_PL.UTF-8` et
`ja-JP` sont ramenés à leur langue de base. Les valeurs explicites de `--lang`
sont volontairement limitées aux sept jetons canoniques du tableau.

Sous Windows, `LC_ALL`, `LC_MESSAGES` et `LANG` sont normalement absents. L'outil utilise donc l'anglais sauf si `--lang` ou `DBWARP_BLUEPRINT_LANG` est défini, par exemple `$env:DBWARP_BLUEPRINT_LANG = "de"` dans PowerShell ou `set DBWARP_BLUEPRINT_LANG=de` dans cmd. Les noms de variables ne sont pas sensibles à la casse sous Windows, mais le sont sous Linux et macOS ; utilisez toujours les noms canoniques en majuscules.

## Éléments traduits

- descriptions de l'aide de premier niveau et des options ;
- éléments de structure de l'aide, tels que les libellés d'utilisation et de valeurs possibles ;
- plans de pré-exécution et demandes de consentement ;
- résumé, cause et action corrective des messages DBP ;
- textes de progression et d'avertissement ;
- titres, libellés, explications et métadonnées de paramètres régionaux des présentations PowerPoint.

Les détails techniques fatals peuvent rester inchangés sous le message DBP localisé lorsqu'ils sont nécessaires au diagnostic. Les avertissements non fatals masquent les détails bruts du pilote lorsqu'ils pourraient contenir des identifiants source ; le code DBP stable et la cible Blueprint anonyme restent disponibles.

## Éléments qui ne changent jamais

Les éléments suivants restent des jetons anglais canoniques dans chaque langue de présentation :

- la commande `dbwarp-blueprint` et les noms d'options tels que `--measure-compression` ;
- les valeurs acceptées telles que `verify-full`, `balanced` et `exact` ;
- les schémas d'URI tels que `postgresql://`, `mysql://` et `sqlserver://` ;
- les noms de variables d'environnement et les chemins de fichiers ;
- les sélecteurs tels que `source=ID` et `table=ID` ;
- les identifiants DBP tels que `DBP1001E` ;
- les identifiants anonymisés tels que `table-001`, `col-1` et `schema-A` ;
- les clés d'audit, clés TOML, clés de bundle, noms de types de base de données et méthodes d'indexation.

Par conséquent, les scripts n'ont pas besoin de gérer des options ou valeurs
propres à chaque langue, et un Blueprint généré avec `--lang ja` est identique
octet pour octet à un Blueprint généré avec `--lang en` lorsque toutes les autres
entrées déterministes sont identiques.

## Comportement strict des catalogues

Tous les catalogues sont compilés dans le binaire. Au démarrage, le programme
vérifie que chaque langue non anglaise annoncée couvre exactement :

- l'arborescence d'aide Clap active ;
- chaque code DBP stable et ses trois champs de diagnostic ;
- chaque clé stable de demande, de progression, d'avertissement et de présentation ;
- chaque espace réservé requis et chaque jeton opérationnel protégé.

Les entrées manquantes ou en trop, les modifications d'espaces réservés, les
jetons opérationnels altérés, le JSON non valide ou les caractères de contrôle
invisibles/bidirectionnels provoquent un refus d'exécution (`fail closed`) avec
`DBP1010E`. Le programme ne remplace pas silencieusement une traduction
manquante par l'anglais.

## Flux de travail des responsables de maintenance

La source canonique est constituée de l'aide Rust en anglais et des définitions
de messages/interface utilisateur dans `src/i18n.rs`. Lorsqu'une expression
visible par le client change :

1. mettez à jour chaque catalogue de paramètres régionaux sous `locales/` dans le même commit ;
2. conservez exactement tous les espaces réservés et les jetons opérationnels canoniques ;
3. exécutez le test ciblé de couverture exacte ;
4. ajoutez ou mettez à jour le cas correspondant à la limite opérateur dans
   `tests/cli_errors.rs` lorsqu'un échec ou un avertissement change ;
5. exécutez l'intégralité de la suite de tests et inspectez un échantillon représentatif de l'aide et des présentations ;
6. obtenez une relecture technique native avant de considérer une nouvelle formulation comme définitive pour un contrat client, un dépôt réglementaire ou un support marketing public.

Validation ciblée :

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

Les tests d'intégration prouvent également que les jetons d'option sont
identiques dans toutes les langues, que les codes DBP localisés restent stables,
que le TOML émis ne dépend pas de la langue et que le texte des présentations
générées porte les paramètres régionaux sélectionnés.
