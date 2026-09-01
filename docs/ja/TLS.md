# TLS と証明書

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../../TLS.md)を参照してください。

**言語:** [English](../../TLS.md) | [Deutsch](../de/TLS.md) | [Français](../fr/TLS.md) | [Español](../es/TLS.md) | [Polski](../pl/TLS.md) | **日本語** | [简体中文](../zh/TLS.md)

データベース接続がネットワーク境界を越える場合は、常に TLS を使用してください。
既定値は `verify-full` です。オペレーターが別のモードを明示しない限り、証明書チェーンとサーバーホスト名を検証します。

## 共通オプション

```bash
--tls-mode disable|prefer|require|verify-ca|verify-full
--tls-ca /path/to/ca-bundle.pem
--tls-cert /path/to/client-cert.pem
--tls-key /path/to/client-key.pem
```

本番環境での推奨設定:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

## 内部 CA

データベース証明書が内部 CA によって署名されている場合:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml
```

## ホスト名の不一致

`--tls-mode verify-full` を使用する場合は、証明書と一致するホスト名を
`--connect` に指定してください。このリリースでは TLS サーバー名の上書きは
サポートしていません。`--tls-server-name` を渡すと、検証を暗黙に弱めることなく
明示的に失敗します。PostgreSQL と MySQL で、ポリシー上ホスト名検証なしの
CA 検証が許可されている場合は、`--tls-mode verify-ca` を使用してください。

既定の信頼元はエンジンごとに異なります:

- PostgreSQL と MySQL は、`--tls-ca` を省略すると、バイナリに組み込まれた
  Mozilla ルートを使用します。PEM バンドルを指定すると、そのルートを置き換えます。
- SQL Server は、`--tls-ca` を省略するとオペレーティングシステムの
  トラストストアを使用します。指定する `.pem` または `.crt` ファイルには
  CA 証明書を正確に 1 つだけ含める必要があり、OS のルートを置き換えます。

SQL Server ドライバーは `verify-ca` と `verify-full` の両方で接続先の
ホスト名を検証します。このエンジンでは、`verify-ca` は意図的に
`verify-full` より弱くありません。

## 平文および互換モード

`prefer` はループバック宛先でのみ許可されます。PostgreSQL はそこでローカル平文へフォールバックして `DBP1404W` を出力する場合があります。他のエンジンは TLS を試行します。リモートの `disable` と `require` には `--i-know-what-im-doing` が必要です。前者は平文を許可し、後者はサーバーを認証せずに暗号化するためです。この確認によって本番向けになるわけではありません。

## mTLS

PostgreSQL と MySQL はクライアント証明書認証をサポートします。いずれかの
データベースがクライアント証明書を要求する場合:

```bash
--tls-cert /etc/dbwarp/client.crt \
--tls-key /etc/dbwarp/client.key
```

Unix 系システムでは、秘密鍵ファイルをグループまたは全ユーザーから読み取り可能にしてはいけません。
SQL Server のクライアント証明書認証は実装されていません。このエンジンで
`--tls-cert`/`--tls-key` を指定すると、ファイルを暗黙に無視せず
`DBP1015E` で失敗します。

## 検証のスキップ

`--tls-skip-verify` は診断専用です。セキュリティチームが明示的に承認していない限り、本番データベースのBlueprint収集には使用しないでください。

## 監査ログ

監査ログには要求した TLS モード、CA とクライアント証明書のパス、検証をスキップしたかどうかが記録されます。接続成功後は TLS がネゴシエートされたかを記録します。現在のドライバーは信頼できるプロトコルバージョンを公開しないため、バージョンは利用不可と記録されます。秘密鍵は記録されません。
