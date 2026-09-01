# バッチ収集とBlueprintバンドル

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../BATCH_AND_BUNDLES.md)を参照してください。

**言語:** [English](../BATCH_AND_BUNDLES.md) | [Deutsch](../de/BATCH_AND_BUNDLES.md) | [Français](../fr/BATCH_AND_BUNDLES.md) | [Español](../es/BATCH_AND_BUNDLES.md) | [Polski](../pl/BATCH_AND_BUNDLES.md) | **日本語** | [简体中文](../zh/BATCH_AND_BUNDLES.md)

`dbwarp-blueprint` は、単一ソースのBlueprintファイルと複数ソースのバンドルディレクトリの両方をサポートします。

顧客が 1 つのデータベース、1 つのテーブルサブセット、1 つの Parquet ファイル、または 1 つの Avro ファイルを共有する場合は、単一の `blueprint.toml` を使用します。顧客が複数のデータベース、複数の構造化ファイルデータセットを所有する場合、または環境全体を 1 つのレビューパッケージにまとめたい場合は、バンドルを使用します。

## バンドルのレイアウト

バッチ実行はディレクトリを書き込みます:

```text
customer-blueprint-bundle/
  bundle.toml
  blueprints/
    erp_pg.blueprint.toml
    billing_mysql.blueprint.toml
    orders_parquet.blueprint.toml
  audits/
    erp_pg.audit.txt
    billing_mysql.audit.txt
    orders_parquet.audit.txt
```

`bundle.toml` には、ソース単位のメタデータと子Blueprintファイルへの相対パスが含まれます。各ソースを個別にレビュー、監査、再実行できるため、これが推奨される作業形式です。

別途レビューした引き渡し用には、ディレクトリを 1 つの埋め込み TOML にパックします:

```bash
dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

パック形式は、各子 Blueprint をそのソースエントリ配下に埋め込みます。
オペレーターが指定したソース ID、タグ、データセットグループ ID、監査パスの
メタデータも保持されるため、匿名のマニフェスト値を使用し、転送前にパック済み
ファイルを確認してください。作業ディレクトリはレビューしやすい一方、詳細な監査と
`errors.txt` がある場合はそれも含むため、既定ではディレクトリ全体を転送しないでください。

## バンドル契約

現在のバンドルは `schema_version = 3` と
`kind = "dbwarp-blueprint-bundle"` を使用します。ディレクトリバンドルは
`blueprint_path` で各子 Blueprint を参照し、パック済みバンドルは
`blueprint` に埋め込みます。Writer はこれらの正規識別子だけを出力します。

Reader はバンドルスキーマ v1 と v2 も受け入れます。これらは入力互換専用です。
受け入れた従来バンドルは v3 に正規化され、旧識別子で再出力されません。従来の
バンドルは source が独立、replica、shard のどれかを宣言しないため、その関係は
`unknown` となり、source 間 aggregate total は抑止されます。子 path は相対で、
正規化後もバンドルディレクトリ内に留まる必要があります。

Bundle v3 は物理 capture source と論理 dataset を分離します。各 source は
`dataset_relationship`、`dataset_group`、`dataset_scope_completeness` を持ち、
トップレベルの `dataset_groups` table が関係、membership、宣言された member set の
完全性を記録します。

aggregation は fail-closed です。

- `independent`: group に source が 1 つだけあり、total は 1 回だけ加算されます。
- `replica`: 一致する copy は 1 回だけ数えます。相違する場合は deterministic
  representative を 1 つ保持し、平均せず、結果を不完全とします。
- `shard`: `members_complete = true` で、宣言したすべての member が成功した場合だけ
  加算します。不完全な shard group は total に寄与しません。
- `unknown`: source 間の table、row、byte total をすべて抑止します。
- source の `[dataset_scope]` が不完全または不明なら、関係が既知でも aggregate
  evidence は不完全になります。

source ごとの total は常に保持されます。抑止は source 間 aggregate だけに適用され、
replica set の重複加算や、一部の shard を dataset 全体として表示することを防ぎます。

## バッチマニフェスト

顧客が所有するマニフェストを作成します:

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
dataset_relationship = "independent"
tags = ["critical", "erp"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
dataset_relationship = "independent"
tags = ["billing"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
dataset_relationship = "independent"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
dataset_relationship = "independent"
tags = ["lake"]
```

関係を省略すると既定値は `unknown` です。run は成功しますが `DBP1414W` と
`DBP1417W` を出力し、aggregate total を抑止します。2 つの endpoint を 2 つの
独立 dataset と仮定するより安全です。

replica member は共通 group で宣言します。

```toml
[[source]]
id = "orders_primary"
kind = "postgresql"
connect_env = "ORDERS_PRIMARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true

[[source]]
id = "orders_secondary"
kind = "postgresql"
connect_env = "ORDERS_SECONDARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true
```

sharded system では、既知のすべての shard を共通 group に列挙し、manifest が完全な
論理 dataset を列挙するときだけ `dataset_group_complete = true` を設定します。
member が失敗すると、その run では group が不完全になります。

最初にドライランします:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

バッチを実行します:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

非ドライランのバッチでは、複数のデータベースへ接続したり、構造化ファイルのサンプルをデコードしたりする可能性があるため、`--yes` が必要です。各子ソースには、それぞれ専用の監査ファイルが作成されます。

`continue_on_error = true` の場合、残りのソースを処理し、`errors.txt` を含む診断バンドルをアトミックに公開します。それでもコマンドは非ゼロで終了します。全ソース失敗は `DBP1115E`、一部失敗は `DBP1116E` です。部分バンドルはレビューと再実行の証拠であり、完全な収集成功ではありません。

ドライランと実行のどちらでも、ソースにアクセスする前にマニフェスト全体を検証します。不明なフィールド、重複する ID、安全なファイル名正規化後に衝突する ID、ソース種別と互換性のないフィールド、曖昧なデータベース接続ソース、無効なデータセットモード、およびゼロの圧縮サンプリング予算は拒否されます。各 `source.id` は一意で、前後に空白がなく、正規化後に 120 ASCII バイト以下でなければなりません。

## 構造化ファイルのデータセットモード

Parquet および Avro ソースの場合:

- `single_file` は解決後のファイルが正確に 1 つであることを要求し、それを 1 つの論理テーブルとして維持します。
- `one_table_per_file` は、各ファイルを 1 つの子Blueprintファイル内の別々のサニタイズ済みテーブルへマッピングします。
- `merge_same_schema` は、列数が一致する場合、多数のファイルを 1 つの論理テーブルにマージします。
- `partitioned_dataset` は現在、`merge_same_schema` と同じマージ動作を使用します。Hive 形式のパーティション検出との意味上の区別を将来のために確保しています。

マージ検査は意図的に保守的です。匿名化された列配置、正規型とネイティブ型、NULL 許容性、宣言された幅、精度とスケール、符号なしと `BIT(n)` の意味、タイムスタンプ精度、文字セットと照合順序、および構造化ソースの意味が一致することを要求します。重要度の高いデータレイク計画では、この構造検査に合格しても、既知のスキーマごとにデータセットをグループ化してください。

## バンドル操作

ソースを一覧表示します:

```bash
dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

先頭行は `aggregation`、物理 `sources`、`logical_datasets`、aggregate total、
`limitations` を表示します。group 行は `relationship`、`members_complete`、source ID、
source 行は `dataset_relationship`、`dataset_group`、`dataset_scope` を表示します。
`aggregation=suppressed` は size zero ではなく、manifest を確認または修正する指示です。

タグ付きソースのサブセットを 1 つ一覧表示します:

```bash
dbwarp-blueprint \
  --bundle-list customer-blueprint-bundle/bundle.toml \
  --select tag=erp
```

1 つのソースを抽出します:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

1 つのソースから 1 つのテーブルを抽出します:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

サポートされるセレクターキーは次のとおりです:

- `source=ID`
- `table=ID`
- `engine=postgresql|mysql|sqlserver|parquet|avro`
- `tag=NAME`

セレクターは、カンマ区切りの 1 つの文字列として、または繰り返し指定する `--select` フラグとして渡せます。同じキーに対する競合する値は拒否されます。

## 後続ツールへの引き渡し

バンドルは、持ち運び可能でレビュー可能なBlueprint入力です。後続の consumer は受け入れる前に、バンドル契約とスキーマバージョンを検証し、記録されたセレクターを適用し、複数の子を結合する際にソース ID を保持して、テーブル ID の衝突を防ぐ必要があります。他の DBWarp 製品のコマンドと互換性規則は、別途レビューされた各製品の文書に属するため、ここでは意図的に重複させません。

## プライバシーとレビューの境界

バンドルによってプライバシーモデルが緩和されることはありません:

- ライブ DB ソースは、引き続きサニタイズされたテーブル/列/インデックス ID を出力する。
- 構造化ファイルの値は、`--measure-compression --yes` が有効な場合にのみデコードされる。
- デコードされたサンプルはメモリ内にとどまる。
- バンドルメタデータは、顧客が選択したソース ID とタグを使用する。
- バンドルコマンドはテレメトリの送信もファイルのアップロードも行わない。

顧客は、バンドルを共有する前に、任意の子Blueprintまたはソースエントリを削除できます。
