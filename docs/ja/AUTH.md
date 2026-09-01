# 認証

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../../AUTH.md)を参照してください。

**言語:** [English](../../AUTH.md) | [Deutsch](../de/AUTH.md) | [Français](../fr/AUTH.md) | [Español](../es/AUTH.md) | [Polski](../pl/AUTH.md) | **日本語** | [简体中文](../zh/AUTH.md)

`dbwarp-blueprint` は、PostgreSQL、MySQL、および SQL Server のBlueprint収集で一般的に必要となる認証モードをサポートします。

## ユーザー名

ユーザー名は URI 内または別途指定できます:

```bash
--connect postgresql://app@db.internal/payments
```

または:

```bash
--connect postgresql://db.internal/payments --user app
```

URI エンコードが難しい文字を含むユーザー名には、次を使用してください:

```bash
--user-file /path/to/user.txt
--user-env DB_USER
```

## パスワード

推奨:

```bash
--password-file /path/to/password.txt
```

代替手段:

```bash
--password-env DB_PASSWORD
```

パスワードソースを指定しない場合、可能であればツールが対話形式で入力を求めます。

接続 URI に埋め込まれたパスワードは拒否されます。

## SQL Server Entra ID トークン

Microsoft Entra ID を使用する Azure SQL Database または Managed Instance では、通常使用しているツールでトークンを生成し、シークレットとして `dbwarp-blueprint` に渡してください。

トークンファイル:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-file /secure/path/token.txt \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out blueprint.toml
```

名前を指定した環境変数:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-env AZURE_SQL_TOKEN \
  --tls-mode verify-full \
  --out blueprint.toml
```

本ツールは Azure CLI を呼び出さず、トークンを更新せず、トークンをディスクに書き込みません。

## SQL Server 統合認証

統合認証では、ホスト上にすでに存在するオペレーティングシステムの資格情報を使用します。

Linux Kerberos / GSSAPI:

```bash
kinit user@EXAMPLE.COM
DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh
./target/release/dbwarp-blueprint \
  --connect sqlserver://db.internal,1433/payments \
  --auth-mode integrated \
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' \
  --tls-mode verify-full \
  --out blueprint.toml
```

Windows SSPI:

```powershell
.\dbwarp-blueprint.exe `
  --connect sqlserver://db.internal,1433/payments `
  --auth-mode integrated `
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' `
  --tls-mode verify-full `
  --out blueprint.toml
```

統合モードでは、`dbwarp-blueprint` はパスワードを読み取りません。オペレーティングシステムが SQL Server ドライバーに認証トークンを提供します。

統合認証を利用できるのは SQL Server のみです。PostgreSQL と MySQL では `--auth-mode integrated` が `DBP1005E` で拒否されます。

上記の例では、Windows プリンシパルが SQL Server ログインとしてすでに存在していることを前提としています。`sql/grants/` の各 Tier スクリプトはパスワード付き SQL ログインを作成するため、このモードには適しません。最初に `FROM WINDOWS` でログインを作成し、その後で Tier の権限を変更せずに適用してください。異なるのはログイン DDL だけです。具体的なステートメント、およびグループ、管理サービスアカウント、コンピューターアカウントの扱いについては、[統合認証用の Windows およびドメインプリンシパル](../../sql/grants/DATABASE_PERMISSIONS.md#windows-and-domain-principals-for-integrated-authentication)を参照してください。

このモードでは、`sql-auth` よりも重要な運用上の注意点が二つあります。SQL Server から見える識別情報は、Collector プロセスを実行しているアカウントです。`BUILTIN\Administrators` が `sysadmin` に属するホストで管理者が Collector を起動すると、そのセッションは `sysadmin` となり、取得が成功したまま Grant スクリプト内のすべての `DENY` を迂回します。`--expect-server-principal` を指定すると、カタログ読み取り前に `DBP1606E` で失敗させることができます。また、専用サービスアカウントは、起動したユーザーのファイルアクセス権を継承しません。使用する場合は自身の資格情報ファイルへの読み取り権限と、`--out` および `--audit-log` のパスへの書き込み権限が必要です。

すべての SQL Server 接続で、`ORIGINAL_LOGIN()`、`SUSER_SNAME()`、
`USER_NAME()` をローカル監査に記録します。`--expect-server-principal` は
任意であり、SQL 認証でも使用できます。確立したセッション上で SQL Server が
`ORIGINAL_LOGIN()` と期待するプリンシパルを比較します。不一致または
識別情報を取得できない場合は、カタログ取得前に `DBP1606E` で失敗します。
正確な識別情報はローカル監査証跡にのみ残り、Blueprint、プレゼンテーション、
公開アーティファクトには含まれません。

## クラウド管理データベースの認証

管理対象エンドポイントであること自体は、`dbwarp-blueprint` が必要とするデータベース権限を変更しません。ネイティブのデータベースユーザー名とパスワードは `sql-auth` を使用し、ネットワークとデータベースアカウントのプロビジョニング後はクラウド制御プレーンのロールを必要としません。

`dbwarp-blueprint` はクラウド CLI、メタデータサービス、シークレットマネージャー、トークン更新 API を呼び出しません。ラッパーが短期間有効な各トークンを生成または取得し、一つの保護されたシークレットソースから渡す必要があります。

### PostgreSQL と MySQL のクラウドトークン

AWS、Azure、または Google Cloud が生成した PostgreSQL/MySQL マネージドサービスの直接トークンには `cloud-token` を使用します。`--password-file` または `--password-env` のいずれか一つだけを指定してください。このモードには `verify-full` が必要です。プロバイダーまたはインスタンスの CA がバイナリに組み込まれた信頼セットに基づかない場合は、その CA バンドルを追加してください。

PostgreSQL の例:

```bash
./dbwarp-blueprint \
  --connect postgresql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

MySQL の例:

```bash
./dbwarp-blueprint \
  --connect mysql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

MySQL では、`cloud-token` は検証済み TLS 接続の内部でのみ `mysql_clear_password` の交換を有効にします。通常の `sql-auth` 接続では、このプラグインは無効のままです。PostgreSQL は同じ検証済み TLS 要件のもとで通常のパスワードプロトコルを使用します。

### クラウド側の実行時権限

これらの権限はログインまたは接続トンネルを許可しますが、データベースプリンシパルとその権限を置き換えるものではありません。

| 管理対象の接続経路 | バイナリのモード | データベース外の実行時権限 |
|---|---|---|
| RDS/Aurora PostgreSQL または MySQL の IAM ログイン | `cloud-token` | 正確なデータベースユーザー ARN に対する `rds-db:connect` |
| Azure Database for PostgreSQL/MySQL の Entra ログイン | `cloud-token` | データアクセス用の Azure リソース RBAC ロールは不要。データベース内で ID をマッピングする必要があります |
| Cloud SQL PostgreSQL/MySQL の直接 IAM ログイン | `cloud-token` | 正確な権限 `cloudsql.instances.login`。`roles/cloudsql.instanceUser` は、より広い定義済みロールの代替です |
| Cloud SQL Auth Proxy またはコネクタ | 通常は `sql-auth`。プロキシが自動 IAM 認証を行う場合があります | プロキシ ID には `roles/cloudsql.client` が必要。自動 IAM 認証にはログイン権限も必要です |
| Azure SQL Database または Managed Instance の Entra ログイン | `entra-token` | データアクセス用の Azure リソース RBAC ロールは不要。上記の SQL Server トークンオプションを使用します |
| ネイティブのデータベース資格情報を使用する対応済み管理対象データベース | `sql-auth` | なし |

デプロイ権限レビューには、バージョン対応のデータベース権限、正確なクラウドポリシー、組み込みロールの代替とスコープ上の注意事項を記録する必要があります。プロバイダー設定、プリンシパル作成、ネットワークアクセス、トークン生成、および任意のシークレット取得は、プロビジョニングまたはラッパーの責務です。エンドポイントが管理対象であるという理由だけでコレクターに付与する権限ではありません。
