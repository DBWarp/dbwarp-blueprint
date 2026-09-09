# Assistance et signalement des problèmes

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../SUPPORT.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../SUPPORT.md) | [Deutsch](../de/SUPPORT.md) | **Français** | [Español](../es/SUPPORT.md) | [Polski](../pl/SUPPORT.md) | [日本語](../ja/SUPPORT.md) | [简体中文](../zh/SUPPORT.md)

Utilisez le
[suivi des tickets](https://github.com/DBWarp/dbwarp-blueprint/issues) pour les
questions d’installation sans informations sensibles, les défauts
reproductibles et les demandes de fonctionnalités. Ce canal ne garantit ni
délai de réponse ni niveau de service d’assistance.

Signalez les vulnérabilités présumées en privé par le moyen indiqué dans
[SECURITY.md](SECURITY.md), et non dans un ticket public.

## Informations utiles

- Tag de version exact, somme de contrôle du binaire, système d’exploitation
  et architecture.
- Moteur et version de la base de données, ou format de fichier structuré,
  et indication du caractère autogéré ou géré de la source.
- Options de commande dont les informations d’identification, les points de
  terminaison, les chemins et les sélecteurs identifiants ont été supprimés ou
  remplacés par des exemples clairement signalés.
- Code de diagnostic `DBP`, comportement attendu et comportement réel.
- Une petite reproduction synthétique, si possible.

Ne téléversez pas de données de production, d’informations d’identification ni
d’audit, de bundle, de Blueprint ou de présentation non vérifiés. Les audits
peuvent contenir des identités et des points de terminaison ; les Blueprints
anonymes peuvent encore révéler une structure de charge de travail distinctive.
Commencez par la description minimale qui peut être communiquée sans risque et
vérifiez chaque pièce jointe avant de la partager.

## Configurations prises en charge

[STATUS.md](../../STATUS.md) décrit les capacités et la matrice des moteurs
qualifiés. [BUILD.md](BUILD.md) décrit les exigences de compilation propres à
la plateforme et à l’authentification. Rust est figé à la version exacte de
`rust-toolchain.toml` ; le champ `rust-version` du paquet ne garantit pas que
toute chaîne d’outils plus récente est qualifiée.

Les consignes d’autorisation des services gérés n’affirment pas que chaque
service ou configuration a été testé. Utilisez les
[exigences d’autorisation](../../sql/grants/DATABASE_PERMISSIONS.md)
correspondantes et qualifiez la configuration exacte avant toute utilisation
en production.

Pour les changements entre versions du collecteur, consultez
[CHANGELOG.md](CHANGELOG.md).
