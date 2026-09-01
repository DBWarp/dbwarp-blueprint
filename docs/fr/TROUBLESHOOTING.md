# Dépannage

> **Traduction assistée par machine :** cette traduction attend une relecture technique par un spécialiste de langue maternelle française. La [version anglaise canonique](../TROUBLESHOOTING.md) fait foi et cette page ne doit pas être considérée comme une formulation contractuelle.

**Langues :** [English](../TROUBLESHOOTING.md) | [Deutsch](../de/TROUBLESHOOTING.md) | **Français** | [Español](../es/TROUBLESHOOTING.md) | [Polski](../pl/TROUBLESHOOTING.md) | [日本語](../ja/TROUBLESHOOTING.md) | [简体中文](../zh/TROUBLESHOOTING.md)

Échecs courants de `dbwarp-blueprint` et actions à entreprendre.

Les échecs relevant de l'opérateur commencent désormais par un code de message stable `DBPnnnnS`, par exemple `DBP1001E`.
Utilisez ce code pour rechercher dans la documentation ou ouvrir un ticket de support. Consultez les [codes de message opérateur](MESSAGES.md).

## La langue demandée n'est pas utilisée

Pour diagnostiquer la sélection des paramètres régionaux, utilisez explicitement une valeur prise en charge :

```bash
dbwarp-blueprint --lang pl --help
```

Les valeurs prises en charge sont `en`, `de`, `fr`, `es`, `pl`, `ja` et `zh`.
Sans `--lang`, l'outil consulte `DBWARP_BLUEPRINT_LANG`, `LC_ALL`, `LC_MESSAGES` et
`LANG`, dans cet ordre. Une valeur explicite non prise en charge est refusée
avec `DBP1011E` ; un catalogue intégré incomplet fait échouer le démarrage avec
`DBP1010E` au lieu de provoquer un retour à l'anglais.

Sous Windows, les variables de langue sont généralement absentes ; passez `--lang` ou définissez `DBWARP_BLUEPRINT_LANG`.

## Largeur ou couleurs de bannière incorrectes

La largeur vient de `COLUMNS` lorsqu'elle est définie, sinon de la console sous Linux et macOS, sinon de 80 colonnes. La capacité couleur vient de `NO_COLOR`, `TERM` et `COLORTERM` ; si `TERM` est absent, ce qui est normal sous Windows, 16 couleurs sont utilisées. Remplacez ce choix avec `--color always`, `--color never` ou définissez `COLUMNS`.

## Le mot de passe dans l'URI est refusé

Symptôme :

```text
DBP1001E refusing to use URI-embedded password
```

Correction : retirez le mot de passe de l'URI et utilisez l'une des options suivantes :

```bash
--password-file /path/to/pass
--password-env DBWARP_BLUEPRINT_PASSWORD
```

Sous Unix, le mode du fichier ne doit pas autoriser sa lecture par le groupe ou les autres utilisateurs.

## Erreur d'autorisation du fichier de mot de passe

Symptôme : l'outil refuse `--password-file` ou `--tls-key`, car les autorisations sont trop larges.

Correction :

```bash
chmod 600 /etc/dbwarp/db.pass
chmod 600 /etc/dbwarp/client.key
```

Cela évite une divulgation accidentelle à d'autres utilisateurs locaux du même hôte.

## Échec de la vérification TLS

Utilisez `--tls-mode verify-full` avec le bon bundle d'autorité de certification et le bon nom d'hôte :

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

Si le nom d'hôte du certificat ne correspond pas, corrigez le nom DNS ou le certificat. `--tls-skip-verify` est refusé sur les hôtes qui ne sont pas en boucle locale, sauf si `--i-know-what-im-doing` est également fourni ; ne l'utilisez pas en production.

## Racines de confiance TLS de SQL Server

Pour SQL Server, les modes de vérification des certificats utilisent le magasin
de confiance du système d'exploitation lorsque `--tls-ca` est omis. Un fichier
`.pem` ou `.crt` fourni doit contenir exactement un certificat d'autorité de
certification et remplace ces certificats racines. Le pilote vérifie le nom
d'hôte de la connexion avec `verify-ca` comme avec `verify-full`.

## Le Tier 2 nécessite un consentement

Symptôme :

```text
--measure-compression requires --yes
```

Correction :

```bash
--measure-compression --yes
```

Ce choix est volontairement explicite, car le Tier 2 lit en mémoire des échantillons de lignes de taille limitée avant de les supprimer.

## L'échantillonnage prend trop de temps

Réduisez l'une ou les deux valeurs suivantes :

```bash
--sample-rows 500
--max-wall-secs 120
```

Pour la première revue de production, il vaut mieux un échantillon Tier 2 plus petit que l'absence de mesure de compression. Si les résultats sont biaisés ou incomplets, recommencez depuis une réplique avec un budget plus élevé.

## Le DBA interdit la sonde SELECT 1 hors catalogue

Désactivez la sonde RTT :

```bash
--no-rtt-probe
```

La sonde RTT par défaut consiste en cinq requêtes `SELECT 1` et ne lit aucune donnée de ligne, mais certaines politiques considèrent toute requête hors catalogue comme hors périmètre.

## La sortie ne contient aucune section de compression

Les sections de compression n'apparaissent que lorsque les deux options sont présentes :

```bash
--measure-compression --yes
```

Les Blueprints catalogue uniquement sont valides, mais les estimations de compression en aval seront déduites.

## Certains échantillons de compression sont marqués comme biaisés

Certains moteurs ne permettent pas un échantillonnage uniforme des tables dans tous les cas, et les petites tables peuvent nécessiter un repli sur `LIMIT`. Le fichier Blueprint enregistre `sampled_with_bias` et `bias_reason` afin que l'estimateur et la personne chargée de la revue puissent en tenir compte.

Les échantillons biaisés restent utiles ; ils sont simplement moins probants que les échantillons uniformes.

## Échec de la génération d'une présentation depuis TOML

`--from-toml` doit être associé à `--deck` :

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

N'incluez pas d'options de base de données active avec `--from-toml`. L'outil refuse le mélange des modes actif et hors ligne afin de conserver une limite d'audit simple.

## Le fichier Blueprint paraît trop petit

Un fichier Blueprint normal est compact. Il contient des métadonnées structurelles, des nombres arrondis, des index, la structure du graphe de clés étrangères et des résumés de compression facultatifs. Il ne doit contenir ni valeurs de ligne ni identifiants réels.

Si vous avez besoin d'une base de données de benchmark représentative, transmettez le fichier `blueprint.toml` approuvé aux outils en aval, examinés séparément et autorisés pour cette mission.

## Prouver qu'aucun téléversement n'a eu lieu

Utilisez le journal d'audit et les outils réseau :

```bash
./dbwarp-blueprint ... --audit-log audit.txt
strace -f -e trace=connect ./dbwarp-blueprint ...
tcpdump host db.internal
```

Le comportement réseau attendu à l'exécution dépend du mode actif. Une
exécution en direct avec `--connect` ouvre la session de base de données
demandée ; DNS peut contacter le résolveur configuré et l'authentification
Kerberos/SSPI intégrée peut contacter un KDC ou un contrôleur de domaine. Le
mode par lot ouvre une session de base de données par source de base de données.
Les opérations locales sur les fichiers TOML, Parquet et Avro, ainsi que sur les
bundles, n'établissent aucune connexion réseau applicative, même si les chemins
montés sur le réseau restent soumis à la pile de stockage de l'hôte.
