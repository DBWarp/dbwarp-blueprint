# Contribuer

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../CONTRIBUTING.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../CONTRIBUTING.md) | [Deutsch](../de/CONTRIBUTING.md) | **Français** | [Español](../es/CONTRIBUTING.md) | [Polski](../pl/CONTRIBUTING.md) | [日本語](../ja/CONTRIBUTING.md) | [简体中文](../zh/CONTRIBUTING.md)

Commencez par un ticket sans informations sensibles décrivant le problème et
une petite reproduction synthétique. Pour les changements importants, discutez
de l’approche avant de préparer un correctif. Les responsables de maintenance
décident si un changement proposé correspond au produit et à ses limites de
sécurité ; ouvrir un ticket ou une pull request n’implique pas son acceptation.

Suivez [SUPPORT.md](SUPPORT.md) pour signaler un problème sans risque et
[SECURITY.md](SECURITY.md) pour signaler une vulnérabilité en privé. Gardez des
échanges respectueux et centrés sur un comportement reproductible.

## Préparer un changement

- Utilisez la chaîne d’outils figée et les dépendances verrouillées décrites
  dans [BUILD.md](BUILD.md).
- Limitez les changements au sujet traité et ajoutez des tests de
  non-régression pour le comportement modifié.
- N’incluez jamais de données client, d’informations d’identification, de
  détails d’infrastructure privée ni de preuves d’audit permettant une
  identification dans un correctif ou un jeu de données de test.
- Préservez le consentement explicite, l’impact borné sur la source, l’accès
  selon le moindre privilège et une présentation honnête des observations
  manquantes ou dégradées.
- Documentez tout changement du contrat Blueprint ou de l’encodage de
  compression. Les règles existantes relatives aux champs facultatifs et à la
  compatibilité font partie de l’interface.
- Conservez en anglais canonique les options CLI, les codes de diagnostic et
  les champs sérialisés. Les changements des textes d’exécution destinés aux
  utilisateurs doivent mettre à jour tous les catalogues d’exécution fournis.
  Le Markdown anglais fait foi ; le Markdown traduit est complémentaire.

## Vérifications locales

Depuis le dépôt source, avec la chaîne d’outils figée installée :

```bash
cargo fmt --all --check
cargo test --locked --all-targets
./tools/check_blueprint_core_sync.sh
python3 tools/check_public_tree.py
```

Le cœur partagé possède sa propre suite de tests unitaires :

```bash
cargo test --locked --manifest-path crates/dbwarp-blueprint-core/Cargo.toml --lib
```

La réussite des tests locaux ne qualifie pas une version de base de données ni
une plateforme. Décrivez ce qui a réellement été testé et indiquez explicitement
que les autres configurations n’ont pas été testées. Ne publiez pas d’artefacts,
ne modifiez pas les tags de version ni les paramètres de sécurité du dépôt dans
le cadre d’un correctif sans l’accord des responsables de maintenance.

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

Ce processus de couverture exacte s’applique aux catalogues d’exécution
intégrés au binaire. Les documents Markdown traduits sont complémentaires ;
consultez [`docs/TRANSLATIONS.md`](../TRANSLATIONS.md).

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
