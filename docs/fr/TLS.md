# TLS et certificats

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../../TLS.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../../TLS.md) | [Deutsch](../de/TLS.md) | **Français** | [Español](../es/TLS.md) | [Polski](../pl/TLS.md) | [日本語](../ja/TLS.md) | [简体中文](../zh/TLS.md)

Utilisez TLS dès que la connexion à la base de données franchit une limite réseau.
`verify-full` est le mode par défaut : la chaîne de certificats et le nom d'hôte du serveur sont validés, sauf choix explicite d'un autre mode.

## Options courantes

```bash
--tls-mode disable|prefer|require|verify-ca|verify-full
--tls-ca /path/to/ca-bundle.pem
--tls-cert /path/to/client-cert.pem
--tls-key /path/to/client-key.pem
```

Réglage recommandé en production :

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

## Autorité de certification interne

Si le certificat de votre base de données est signé par une autorité de certification interne :

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml
```

## Incompatibilité du nom d'hôte

Avec `--tls-mode verify-full`, utilisez dans `--connect` un nom d'hôte qui
correspond au certificat. Cette version ne permet pas de remplacer le nom du
serveur TLS ; fournir `--tls-server-name` provoque un échec explicite plutôt
qu'un affaiblissement silencieux de la vérification. Si votre politique autorise
la validation de l'autorité de certification sans validation du nom d'hôte,
utilisez `--tls-mode verify-ca`.

Les sources de confiance par défaut dépendent du moteur :

- PostgreSQL et MySQL utilisent les certificats racines Mozilla intégrés au
  binaire lorsque `--tls-ca` est omis. Un bundle PEM fourni remplace ces
  certificats racines.
- SQL Server utilise le magasin de confiance du système d'exploitation lorsque
  `--tls-ca` est omis. Un fichier `.pem` ou `.crt` fourni doit contenir
  exactement un certificat d'autorité de certification et remplace les
  certificats racines du système d'exploitation.

Le pilote SQL Server valide le nom d'hôte de la connexion avec `verify-ca`
comme avec `verify-full` ; pour ce moteur, `verify-ca` n'est délibérément pas
moins strict que `verify-full`.

## Modes en clair et de compatibilité

`prefer` n'est accepté que pour les cibles de bouclage. PostgreSQL peut alors revenir au texte en clair local et émet `DBP1404W` ; les autres moteurs tentent toujours TLS. À distance, `disable` et `require` nécessitent `--i-know-what-im-doing`, car ils autorisent le texte en clair ou chiffrent sans authentifier le serveur. Cette confirmation ne rend pas ces modes adaptés à la production.

## mTLS

PostgreSQL et MySQL prennent en charge l'authentification par certificat client.
Si l'une de ces bases de données exige un certificat client :

```bash
--tls-cert /etc/dbwarp/client.crt \
--tls-key /etc/dbwarp/client.key
```

Les fichiers de clé privée ne doivent pas être lisibles par le groupe ou par tous les utilisateurs sur les systèmes de type Unix.
L'authentification par certificat client n'est pas implémentée pour SQL Server ;
avec ce moteur, fournir `--tls-cert`/`--tls-key` provoque un échec `DBP1015E`
au lieu d'ignorer silencieusement les fichiers.

## Ignorer la vérification

`--tls-skip-verify` est réservé au diagnostic. Ne l'utilisez pas pour collecter le Blueprint d'une base de données de production, sauf approbation explicite de votre équipe de sécurité.

## Journal d'audit

Le journal d'audit enregistre le mode TLS demandé, les chemins de CA et de certificat client, ainsi que l'éventuel contournement de la vérification. Après connexion, il enregistre si TLS a été négocié ; les pilotes actuels n'exposant pas de version fiable, celle-ci est indiquée comme indisponible. Les clés privées ne sont pas enregistrées.
