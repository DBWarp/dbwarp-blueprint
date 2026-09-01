# クックブック

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../COOKBOOK.md)を参照してください。

**言語:** [English](../COOKBOOK.md) | [Deutsch](../de/COOKBOOK.md) | [Français](../fr/COOKBOOK.md) | [Español](../es/COOKBOOK.md) | [Polski](../pl/COOKBOOK.md) | **日本語** | [简体中文](../zh/COOKBOOK.md)

一般的な `dbwarp-blueprint` ワークフローのための、タスク指向のレシピです。

## レシピ: ローカライズされたオペレーターセッション

コマンド、値、識別子、出力スキーマを正規の形式に保ったまま、
完全な組み込み言語カタログの 1 つを選択します:

```bash
./dbwarp-blueprint --lang de --help
./dbwarp-blueprint --lang ja \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

無人実行では、`DBWARP_BLUEPRINT_LANG=fr` または標準のプロセスロケールを設定します。
明示的な `--lang` が常に優先されます。DBP コードと低レベルのプロバイダー詳細は
正規の形式を維持するため、ローカライズされた失敗も検索してサポートと共有できます。

## レシピ: 内部 CA を使用する PostgreSQL

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out pg-appdb.blueprint.toml \
  --audit-log pg-appdb.audit.txt
```

通常の本番 PostgreSQL レビューにはこれを使用します。ホスト名検証が失敗した場合は、サーバー証明書を修正するか、正しい DNS 名を使用してください。loopback テスト以外では `--tls-skip-verify` を使用しないでください。

## レシピ: ユーザー名ファイルを使用する MySQL

ユーザー名に URI エンコードしにくい文字が含まれる場合に便利です。

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

性能を代表する合成再構築には、既定の balanced ポリシーを使用します。これは、
正確な MySQL 宣言/インデックスメタデータと、細かく丸められたサンプル幅を使用します:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

`declared_length_fidelity = "exact"`、
`index_length_fidelity = "exact"`、および
`observed_length_fidelity = "relative-rounded-v2"` を確認してください。
顧客が正確なサンプル長統計の共有を承認した後に限り、
`--length-fidelity exact --yes` を使用してください。名前と値は引き続き除外されます。

数千のテーブルがある環境では、必要に応じて `--max-wall-secs` を既定の 300 秒より
大きくしてください。fidelity マーカーはポリシーを証明しますが、後段の
estimator は、フィクスチャをベンチマーク対応と判定する前に、空でない可変幅の
すべてのインデックス列について observed average/p95 length を別途要求します。

## レシピ: SQL Server SQL 認証

```bash
./dbwarp-blueprint \
  --connect sqlserver://sql-blueprint@sql-primary.internal,1433/appdb \
  --password-file /etc/dbwarp/sql-blueprint.pass \
  --auth-mode sql-auth \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-appdb.blueprint.toml \
  --audit-log mssql-appdb.audit.txt
```

SQL Server で証明書を検証する TLS モードは、`--tls-ca` を省略すると
オペレーティングシステムのトラストストアを使用します。指定する `.pem` または
`.crt` ファイルには CA 証明書を正確に 1 つだけ含める必要があり、そのルートを
置き換えます。`verify-ca` と `verify-full` はどちらも接続先のホスト名を検証します。

## レシピ: SQL Server Entra ID トークン

ツールの外部でトークンを生成し、ファイルで渡します:

```bash
install -d -m 700 "$HOME/.cache/dbwarp-blueprint"
TOKEN_FILE="$HOME/.cache/dbwarp-blueprint/sql-token"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"
chmod 600 "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

## レシピ: カタログのみのセキュリティレビュー

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

これは、最も手間の少ないレビューモードです。行サンプリングを回避しますが、後段の圧縮とエグレスの推定精度は低下します。

## 非テーブル移行の複雑度を評価する

定義を読まずに件数と外部前提条件を収集するには、既定のサマリーから始めます:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail summary \
  --out appdb-summary.blueprint.toml \
  --audit-log appdb-summary.audit.txt \
  --yes
```


セキュリティ承認後に、匿名依存関係と有界な言語複雑度の証拠を収集します:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail analyzed \
  --out appdb-analyzed.blueprint.toml \
  --audit-log appdb-analyzed.audit.txt \
  --yes
```


`visibility`、3 つの完全性フラグ、`catalogs_unreadable`、`families_not_inventoried`、`counts_by_external_class` を確認してください。各外部クラスを明示的な移行タスクとして扱います。インベントリ済みオブジェクトは DBWarp が再作成または翻訳できる証明ではありません。移行機能マトリクスと比較してください。[`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)を参照してください。

## レシピ: RTT プローブを無効にする

既定では、接続確立後に 5 回の `SELECT 1` プローブを実行し、`[network]` ブロックを出力します。DBA がカタログ外のクエリを禁止している場合は、無効にします:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --no-rtt-probe \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

RTT プローブは行データを読み取りません。各クエリは定数の整数 `1` を返します。

## レシピ: 圧縮サンプリングの時間を制限する

大規模な本番システムでは、最初の実行を保守的にします:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal/appdb \
  --password-file /etc/dbwarp/mysql.pass \
  --measure-compression --yes \
  --sample-rows 500 \
  --max-wall-secs 120 \
  --out blueprint.toml \
  --audit-log audit.txt
```

出力で多数のサンプルが biased または missing と記録された場合は、より大きな時間予算を設定し、リードレプリカから再実行してください。

## レシピ: 1 顧客、複数データベース

複数のデータベースについて 1 つのレビューパッケージを顧客が必要とする場合は、バッチマニフェストを使用します。

`customer.batch.toml`:

```toml
[defaults]
measure_compression = true
sample_rows = 1000
max_wall_secs = 300
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["erp", "critical"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
tags = ["billing"]

[[source]]
id = "warehouse_sql"
kind = "sqlserver"
connect_env = "WAREHOUSE_SQL_URI"
password_file = "/etc/dbwarp/warehouse.pass"
auth_mode = "sql-auth"
tags = ["warehouse"]
```

ドライラン:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

実行:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

これにより、`bundle.toml`、ソースごとの子Blueprint 1 つ、およびソースごとの監査 1 つが書き込まれます。
各子Blueprintは独立してレビューできます。

## レシピ: 1 顧客、データベースとデータレイクファイルの混在

顧客がライブデータベースのほかに Parquet または Avro 抽出を持つ場合は、同じバッチ内で構造化ファイルのソースを使用します。

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["database"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/mnt/customer/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/mnt/customer/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` は現在、`merge_same_schema` と同様にファイルを統合しますが、顧客の意図はバンドル内で確認できるよう保持されます。関連のないスキーマは別々のソースにしてください。

## レシピ: バンドルから 1 つのソースまたはテーブルだけを抽出する

バッチ実行後、ソースを一覧表示します:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

1 つのソースを抽出します:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

1 つのソースから 1 つのテーブルを抽出します:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

顧客が環境の一部だけをベンチマーク用に承認する場合、または大規模なバンドルから小規模で対象を絞ったフィクスチャを生成する場合に使用してください。

## レシピ: 個別にレビュー済みのバンドルを引き渡し用にパックする

作業用バンドルディレクトリには、子 Blueprint とアクセス制御された監査が含まれます。ディレクトリ全体を転送しないでください。マニフェストの値と子 Blueprint をレビューした後、単一ファイルの引き渡し用ファイルを作成します:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

パックされたファイルには、オペレーターが指定したソース ID、タグ、データセットグループ ID、監査パスのメタデータが残ります。匿名の値を使用し、パック済み TOML を検査して、承認済みチャネルだけで転送してください。

## レシピ: バッチ引き渡しパッケージ

次のようなディレクトリを作成します:

```text
customer-blueprint-handoff/
  customer-blueprint-bundle.packed.toml
  customer.batch.toml.redacted
  reviewer-notes.md       # optional
```

レビュー済みのコピーから、この別ディレクトリを構築してください。作業用の `bundle.toml`、`blueprints/`、`audits/`、および `errors.txt` はローカルに置き、アクセス制御してください。`customer.batch.toml.redacted` に含めるのは、承認済みのソース ID、種別、タグ、データセットモードだけです。シークレット、プライベートホスト名、パスワードファイル、トークンファイル、秘密鍵、データベースログ、デコードされた行サンプルを含めないでください。

## レシピ: レビュー済み TOML からのオフラインデッキ

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

このモードは TOML ファイルだけを読み取り、デッキを書き込みます。ライブデータベース用フラグを暗黙に無視せず、拒否します。

## レシピ: バイト単位で同一の再現性

タイムスタンプを固定します:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

フォレンジックレビュー、スナップショット比較、または決定論的なデッキ生成に使用してください。

## レシピ: DBWarp への引き渡しパッケージ

次のようなディレクトリを作成します:

```text
customer-blueprint-handoff/
  blueprint.toml
  blueprint.pptx              # optional
  command-used.redacted.txt
  reviewer-notes.md           # optional
```

`command-used.redacted.txt` には、承認済みのフラグとサンプリング予算を記録できますが、認証情報、トークン、プライベートホスト名、ローカルパスは削除してください。`audit.txt` はアクセス制御された運用証拠としてローカルに保持してください。特定のサポート上の必要性がある場合に限り、承認済みの安全なチャネルを通じて含めます。パスワードファイル、トークンファイル、秘密鍵、データベースログを含めないでください。
