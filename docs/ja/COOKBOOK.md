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
  --artifact-detail none \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

無人実行では、`DBWARP_BLUEPRINT_LANG=fr` または標準のプロセスロケールを設定します。
明示的な `--lang` が常に優先されます。DBP コードと低レベルのドライバー詳細は
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

上記のレシピでは、既定のバランスの取れたポリシーがすでに使用されています。具体的には、正確なMySQL declaration/index メタデータと、厳密に丸められたサンプルデータ幅が使用されています。

`declared_length_fidelity = "exact"`、`index_length_fidelity = "exact"`、および`observed_length_fidelity = "relative-rounded-v2"`を確認してください。 `--length-fidelity exact --yes`は、貴組織が正確なサンプルデータの長さの共有を承認した場合にのみ使用してください。 名前と値は引き続き除外されます。

数千のテーブルを持つデータベースでは、必要に応じて`--max-wall-secs`を既定値である300秒よりも高く設定してください。フィデリティマーカーはポリシーを記述するものであり、サンプリングがすべてのテーブルに到達したことを示すものではありません。

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
install -m 600 /dev/null "$TOKEN_FILE"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

Azure SQL はパブリック CA の証明書を提示するため、このレシピでは `--tls-ca` を
設定せず、オペレーティングシステムのトラストストアを使用します。指定した
`--tls-ca` ファイルは、そのストアを 1 つの証明書で置き換えます。
[TLS](TLS.md)を参照してください。

## レシピ: カタログのみのセキュリティレビュー

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

これは最も負荷の少ないレビューモードです。行のサンプリングは行いませんが、圧縮率やデータ転送量の推定精度は低くなります。

## レシピ: 非テーブル移行の複雑度を評価する

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


`visibility` を確認し、すべての完全性フラグ、`catalogs_unreadable`、`families_not_inventoried`、および `counts_by_external_class` を確認してください。各外部クラスを、明示的な移行タスクとして扱ってください。インベントリに登録されたオブジェクトが、DBWarp がそれを再作成または変換できることの証明であるとはみなさないでください。DBWarp に、移行でサポートされているオブジェクトの種類を問い合わせてください。[`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) を参照してください。

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

## レシピ：1つのパッケージに複数のデータベース。

複数のデータベースに対して、レビュー可能な1つのパッケージを使用したい場合は、バッチマニフェストを使用してください。

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

## レシピ：混合データベースとデータレイクファイル。

ParquetまたはAvroの抽出データがある場合、ライブデータベースと一緒に構造化ファイル形式のソースを同じバッチで使用してください。

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
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` は、`merge_same_schema` のようなファイルを結合し、宣言されたモードをバンドルに記録します。 関連性のないスキーマは、別のソースに保持してください。

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

この機能は、バンドルの一部のみが共有される場合に利用します。

## レシピ：レビュー済みのバンドルを共有用にパッケージ化する。

作業用のバンドルディレクトリには、子Blueprintとアクセス制御された監査が含まれています。これを丸ごと転送しないでください。マニフェストの値と子Blueprintを確認した後、共有するための単一のファイルを作成してください。

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

パックされたファイルには、オペレーターが指定したソース ID、タグ、データセットグループ ID、監査パスのメタデータが残ります。匿名の値を使用し、パック済み TOML を検査して、承認済みチャネルだけで転送してください。

## レシピ：共有用のバッチパッケージ。

[review-and-share]に関する[ガイドライン](QUICKSTART.md#review-and-share)に従ってください。作業中のマニフェスト、監査ログ、およびコマンド記録はローカルに保持し、レビュー済みのパッケージ化されたBlueprintのみを、この別のディレクトリに作成してください。

```text
blueprint-share/
  customer-blueprint-bundle.packed.toml
```

## レシピ: レビュー済み TOML からのオフラインデッキ

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

このモードは TOML ファイルだけを読み取り、デッキを書き込みます。ライブデータベース用フラグを暗黙に無視せず、拒否します。

## レシピ: バイト単位で同一の再現性

タイムスタンプを固定し、お手元で管理する同じ保護された匿名化キーを再利用します:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --anonymization-key-file /etc/dbwarp/anonymization.key \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

キーファイルは、正確に32バイトの生データ、または64文字の16進数で構成されている必要があります。Unix環境では、group/world-readableであってはならず、決して共有してはなりません。このオプションを指定しない場合、新しいオペレーティングシステム乱数キーが、意図的に毎回匿名ラベルの順序を変更します。`--generated-at`のみを指定しても不十分です。承認されたフォレンジックのスナップショットには、完全な手順を使用してください。同じレビュー済みのBlueprintから2回生成されたデータは、タイムスタンプと言語が変更されない限り、バイト単位で完全に同一になります。

## レシピ：DBWarpで共有するためのパッケージ。

[review-and-share]に関する[ガイドライン](QUICKSTART.md)を参照してください。 既定のパッケージには、承認されたBlueprintのみが含まれています。

```text
blueprint-share/
  blueprint.toml
```

`blueprint.pptx` は、別途レビューと承認を行った後でのみ追加してください。監査ログ、コマンド記録、および credential/key に関連する資料は、共有ディレクトリに含めないでください。監査ログは、特定のサポートニーズがある場合に限り、承認された安全なチャネルを通じて送信してください。
