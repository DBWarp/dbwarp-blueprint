<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../.github/assets/dbwarp-logo-dark.png">
    <img src="../../.github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

# dbwarp-blueprint

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../../README.md)を参照してください。

**言語:** [English](../../README.md) | [Deutsch](../de/README.md) | [Français](../fr/README.md) | [Español](../es/README.md) | [Polski](../pl/README.md) | **日本語** | [简体中文](../zh/README.md)

ローカライズされたドキュメントの管理方針については、[`docs/TRANSLATIONS.md`](../TRANSLATIONS.md)を参照してください。

英語が正本です。機械支援による翻訳は補足資料であり、誤りを含む可能性があります。
[`MACHINE_TRANSLATIONS.md`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/MACHINE_TRANSLATIONS.md).

## 概要

DBWarp Blueprint は、信頼を最優先に設計されたデータベース Blueprint コレクターです。PostgreSQL、MySQL、または SQL Server に対して、お客様自身の環境内で実行します。カタログメタデータを読み取り、圧縮測定を指定した場合は上限付きの行サンプルも読み取ります。その後、テーブルサイズ、行数、型ファミリー、インデックスと外部キーの構造を含む、データベースの匿名化された構造 Blueprint を書き出します。

識別子はキー付きの匿名ラベルに置き換えられ、Blueprintには行の値は一切書き込まれません。デフォルトでは、新しいプロセスローカルキーにより、オフライン辞書チェックは行われません。`--anonymization-key-file`を使用すると、承認された比較実行間でラベルを保持できます。出力の共有前に、[`SECURITY.md`](SECURITY.md)を必ずお読みください。各モードで何が公開されるか、およびどのオプションがそれを拡大するかについて詳細に説明されています。

出力はプレーンテキストファイルです。共有するかどうかを決める前に、すべての行を確認できます。

DBWarp Blueprint は無料のオープンソースソフトウェアであり、すべてお客様の環境内で動作します。データベースそのものを渡さずに、データベースに関する事実を当社へ提供できるようにするためのツールです。

## 実行する理由

Blueprint の出力を当社と共有していただければ、DBWarp がデータをどの程度高速に移動できるか、またそれによって移行、CI/CD テストデータ、分析の各スケジュールがどう変わるかをご説明できます。

距離が最も重要です。データの移動距離が長いほど、DBWarp が示せる改善幅は大きくなります。

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; スイス、チューリッヒ

---

`dbwarp-blueprint`を独自の環境で実行し、DBWarpがデータベースへのアクセス、ダンプ、スキーマ名、または行データを受け取ることなく、移行の規模決定と計画に使用できる、制限付きで匿名化された、レビュー可能な`blueprint.toml`ファイルを作成します。

PostgreSQL、MySQL、または SQL Server に接続してカタログメタデータを読み取り、必要に応じて制限された行サンプルからローカル圧縮率を測定し、プレーンテキストの TOML を書き込みます。入力がライブデータベースではなく、すでに構造化データファイルである場合は、オフラインモードでローカルの Parquet または Avro ファイルからBlueprintを導出することもできます。出力を開いてすべての行をレビューし、共有するかどうかを判断できます。

任意で `--deck blueprint.pptx` を指定すると、同じ匿名化Blueprintの PowerPoint サマリーも書き込みます。デッキはライブデータベース実行時に書き込めるほか、レビュー済み TOML ファイルから `--from-toml blueprint.toml --deck blueprint.pptx` を使って後から書き込めます。デッキ作成機能は Rust バイナリに組み込まれており、ネットワーク接続を行いません。

## 用途

DBWarp が転送を推定し計画するには、十分な構造情報が必要です:

- テーブル数
- 概算行数
- テーブルとインデックスのサイズ
- 列の型ファミリー、正確な構造上の容量/インデックスプレフィックス、および既定でプライバシーに配慮して丸められる観測幅
- インデックスと外部キーの形状
- 境界付きで名前を含まない非テーブル成果物の件数と外部デプロイ前提条件
- 小さなローカルサンプルから得られる、任意のテーブルおよび列の圧縮サマリー
- オプションで、データ収集元からデータベースまでのラウンドトリップ時間を測定できます。

それらの事実は、データ転送のサイズを推定し、転送を計画するのに十分です。ソース名と行の値は省略されていますが、特徴的な構造と統計情報は依然としてワークロードを特定する可能性があります。匿名化はリスク軽減であり、不可逆性の保証ではありません。

## 行わないこと

`dbwarp-blueprint` は次を行いません:

- テレメトリの送信
- DBWarp サーバーの呼び出し
- Blueprintファイルのアップロード
- `~/.pgpass`、`~/.my.cnf`、クラウド資格情報、SSH 鍵の読み取り
- `PGPASSWORD` や `MYSQL_PWD` など、既定のパスワード環境変数の読み取り
- 暗黙のシステム一時、キャッシュ、設定ディレクトリの使用。明示的に選択された
  出力ファイルを書き込み、バッチモードではアトミック公開のため `--out-dir` の隣に
  ステージングまたは復旧ディレクトリも使用します
- 実際のテーブル名、列名、インデックス名、スキーマ名、非テーブルオブジェクト名、SQL 定義、外部エンドポイント、資格情報、鍵、証明書、バイナリ、行の値を出力に含めること

ライブBlueprint実行では、指定されたエンドポイントへのデータベースセッションを
開きます。DNS は設定済みのリゾルバーを使用する場合があり、統合
Kerberos/SSPI 認証は認証基盤へ接続する場合があります。バッチモードでは、
データベースソースごとにこの境界が繰り返されます。ローカル TOML、Parquet、
Avro、およびバンドル操作は、アプリケーションからネットワーク接続を開始しません。

## ダウンロードまたはビルド

| 方法 | 最適な用途 | リンク |
|---|---|---|
| バイナリファイルをダウンロードしてください。 | クイックトライアル、隔離されたテストホスト。 | [`binaries/README.md`](BINARIES.md) |
| 小規模なソースクローンからビルド | セキュリティレビュー、本番ポリシー、再現性確認 | [`BUILD.md`](BUILD.md) |
| vendored ソースバンドルからビルド | 厳格なオフライン依存関係監査 | GitHub Releases |
| SQL フォールバックをレビューして実行 | DBA ポリシーがサードパーティーのバイナリを拒否 | [`sql/blueprint.pg.sql`](../../sql/blueprint.pg.sql)、[`sql/blueprint.mysql.sql`](../../sql/blueprint.mysql.sql)、[`sql/blueprint.sqlserver.sql`](../../sql/blueprint.sqlserver.sql)、[`blueprint_format.py`](../../blueprint_format.py) |

### SQL フォールバックの境界

SQL フォールバックはレビュー可能な最低限のカタログ経路であり、Rust コレクターと同等機能の代替ではありません。各 SQL スクリプトは実際のスキーマ、テーブル、列、インデックス名を含む中間 JSON を書きます。MySQL の `COLUMN_TYPE` には宣言された enum/set メンバーも含まれる場合があります。この JSON を機密性のあるスキーマ素材として扱い、ソース環境内に保持し、そこで `blueprint_format.py` により正規化して、レビュー済み TOML 出力だけを共有してください。

フォールバックには `--schema` セレクターがありません。PostgreSQL は接続先データベースのすべての非システムスキーマにある通常テーブル、MySQL と SQL Server は選択したデータベースの通常のローカルユーザーテーブルを対象とします。一部だけが承認されている場合は使用しないでください。その TOML には通常テーブルのサブセットについてテーブル/列/インデックス/FK 構造と概算ローカルサイズが含まれますが、v7 のすべてのテーブル種別をインベントリ化するものではありません。すべての構造ファミリーを不完全として扱い、MySQL FEDERATED テーブルと SQL Server 外部テーブルについては、リモートデータをローカルと誤表示せず除外します。また、行サンプリング、RTT 証拠、非テーブル成果物インベントリ、ライブトポロジプローブは含まれません。トポロジとデータセット完全性は明示的に `unknown` です。

信頼を優先する方法は、ソースからビルドすることです。通常のリポジトリは小さく保たれ、`Cargo.lock` で依存関係のバージョンを固定します。より厳格なオフライン監査向けに、各リリースではすべての依存関係ソースファイルを含む vendored ソースバンドルも公開します。利便性のために SHA256 チェックサム付きのリリースバイナリも提供します。

## クイックスタート

データベースへ接続する前に、DBA は
[`sql/grants/`](../../sql/grants/) にあるエンジン、バージョン、階層に対応した
スクリプトで最小権限の専用コレクターアカウントを準備し、正確なスキーマ範囲を
承認してください。アプリケーション所有者や管理者のアカウントから開始しないで
ください。承認済みの各スキーマを `--schema` で指定し、キャプチャ後は
[`sql/revoke/`](../../sql/revoke/) の該当スクリプトで専用アカウントを削除します。
完全な初回実行手順は [`docs/QUICKSTART.md`](QUICKSTART.md) を参照してください。

必要に応じて表示言語を選択してください。英語が既定で、ドイツ語、フランス語、
スペイン語、ポーランド語、日本語、簡体字中国語の完全なカタログが組み込まれています:

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --schema app --dry-run
```

翻訳されるのは、人向けのヘルプ、プロンプト、診断、進行状況、PowerPoint デッキの
ラベルだけです。コマンド名とオプション名、受け付ける値、URI スキーム、
環境変数名、セレクター、DBP コード、監査キー、生成される TOML は
正規の英語トークンを維持します。これにより、すべての言語で自動化とサポート手順を
同一に保てます。[`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md)を参照してください。

データベースに接続する前に、[`samples/`](../../samples/) の下の例を確認してください。これらは通常の Blueprint TOML ファイルであり、確認するために特別な設定は必要ありません。バイナリを入手した後、データベースやネットワークにアクセスすることなく、オフラインで最初の実行を行うことで、それらをデッキとして表示することができます。

```bash
./dbwarp-blueprint --from-toml samples/sqlserver-v6-analyzed.toml --deck sample.pptx
```

[`samples/README.md`](../../samples/README.md)に記載されている、小さなBlueprintの例から始めます。これには、PostgreSQLのカタログのみ、MySQLのサンプル、および分析された成果物を含むSQL Serverのサンプルが含まれます。これらの例は、手作業で作成されたものであり、実際のデータ収集ではありません。より大きなschema-v1の例は、古い形式を示していますが、まだ読み取ることができます。キャプチャを承認する前に、[`FORMAT.md`](FORMAT.md)を使用して、完全な出力内容を確認してください。どの例にも、すべてのオプションフィールドが網羅されているわけではありません。
最初にドライランしてください。接続せずにプランを表示します:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --dry-run
```

TLS、監査ログ、圧縮測定を使用する本番スタイルの推奨実行:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

`--measure-compression --yes`を使用すると、出力にはテーブルレベルのzstd圧縮率と、各列ごとの圧縮予測が含まれます。各列のブロックは、テーブルレベルの圧縮率と同じ範囲のサンプルから計算されます。これらは転送量の見積もりを詳細化するものであり、サンプリングされた値をディスクに書き込むことはありません。明確に識別可能な標準的な圧縮コンテナのシグネチャを持つ主要なバイナリサンプルは、粗い`style = "precompressed"`ラベルのみを受け取り、ファイルタイプ、シグネチャ、またはサンプリングされた値はシリアル化されません。スキーマv3以降では、範囲に制限された、名前を含まない各列のcardinality/skew集計と、推測されたindex-prefix/relationshipサマリーも出力されます。一時的な各値のハッシュはメモリ内で範囲に制限され、破棄されます。サンプリングされた値と各値のハッシュは、ドキュメント化された集計とは異なり、Blueprint TOMLには表示されません。

スキーマ v4 以降、Blueprint は非テーブルオブジェクトも収集します。既定の
`--artifact-detail summary` は定義を読まず、オブジェクトクラスおよび外部前提条件
クラス別の有界件数を保存します。`graph` は匿名依存トポロジを、`analyzed` は
有界な言語機能および複雑度のバンドを追加します。匿名グラフでもアプリケーションを
識別できるため、どちらも `--yes` が必要です:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```


成果物の存在は計画証拠であり、DBWarp が自動的に再作成または翻訳できるという
主張ではありません。[`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)を
参照してください。

### MySQL の長さ忠実度

既定の `balanced` ポリシーは、宣言された文字/バイト容量と
インデックスプレフィックス長を正確に保持します。サンプリングした average/p95 value length には
relative-error bucket（最大誤差は約 3.2%、32 バイト以下の値は正確に保持）を使用します。
これにより、通常 9 文字の `VARCHAR(3000)` キーを 9 文字付近に保ち、実際の値の幅を
サイズに反映しながら、有効なソース DDL/インデックス制限を維持できます:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

ポリシーが追加の精度を許可する場合に限り、正確なサンプル統計を使用してください:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

`--length-fidelity strict` を使用して、宣言された、観測された、およびプレフィックスの長さに対応する、粗いプライバシーバケットを適用します。厳密モードでは、結果の推定値の精度が低下します。`--preserve-exact-lengths --yes` という表記は、`--length-fidelity exact --yes` のエイリアスです。

新しいBlueprintでは、`declared_length_fidelity`、`index_length_fidelity`、および`observed_length_fidelity`の各フィールドが個別に記録されます。また、`length_metadata`フィールドも書き込まれるため、以前の形式を読み取るツールも引き続き動作します。PostgreSQLの文字容量は、正確なカタログ値です。エンコーディングに依存するバイト数の上限と、インデックスのプレフィックスの長さは、依然として利用できません。

最も正確な見積もりを得るには、`--measure-compression` オプションを指定して実行してください。これにより、観測された平均値と p95 値の長さが記録され、実際の値よりもはるかに広い幅で宣言された列が過大評価されるのを防ぎます。デフォルトのサンプリング時間制限は 300 秒です。非常に大規模なスキーマの場合は、`--max-wall-secs` の値を増やしてください。

その後、ファイルをレビューします:

```bash
less blueprint.toml
less audit.txt
```

[review-and-shareの手順](QUICKSTART.md#review-and-share) に従って、いかなる成果物も送信する前に確認・共有してください。承認されたBlueprintの内容のみを共有し、別途レビューおよび承認された場合は、プレゼンテーション資料（deck）を共有してください。運用に関する証拠は、原則としてローカルに保持してください。

## 構造化ファイルモード

ソースがすでにローカルの構造化ファイルである場合は、データベース資格情報なしでBlueprint TOML を生成します:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

```bash
./dbwarp-blueprint \
  --from-avro /data/sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Parquet モードは footer と row-group metadata を読み取ります。Avro object container には同等の footer row count がないため、Avro モードは container を走査して record 数を数え、writer schema を列構造に使用します。どちらのモードも、データベースへ接続せず、資格情報フラグも読み取りません。

ポリシーでデコード済みサンプリングが許可されている場合、ファイルモードでも
転送計画に使用する、制限付きのローカル圧縮可能性を測定できます:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

同じフラグを `--from-avro` でも使用できます。サンプル値はメモリ内で
`blueprint-compression-probe-v2` としてエンコードされ、Blueprint には圧縮、NULL 密度、
カーディナリティ/頻度、長さ、スタイルの集約測定値が保存されます。サンプリング値は
保存されません。

## バッチおよびバンドルモード

複数のデータベース、複数のtables/datasets、または一連のデータベース全体のレビューを行う場合は、バッチマニフェストを使用し、出力ファイルをまとめて格納するディレクトリを作成してください。

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

作業ディレクトリには `bundle.toml`、ソースごとの子Blueprintファイル、
アクセス制御されたソースごとの監査ログが含まれます。既定では作業ディレクトリ
全体を転送しないでください。一覧表示や抽出のほか、別途レビューするパック済み
Blueprintバンドルを作成できます:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
./dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 --out table-042.blueprint.toml
./dbwarp-blueprint --bundle-pack customer-blueprint-bundle --out customer-blueprint-bundle.packed.toml
```

マニフェスト構文、構造化ファイルの dataset mode、selector rule については、
[`docs/BATCH_AND_BUNDLES.md`](BATCH_AND_BUNDLES.md)を参照してください。

## 一般的なデータベースコマンド

PostgreSQL:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

MySQL:

```bash
./dbwarp-blueprint \
  --connect mysql://app@db.internal/payments \
  --schema payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

SQL Server:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@db.internal,1433/payments \
  --schema dbo \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

Kerberos、SSPI、Entra ID の例については [`AUTH.md`](AUTH.md) を参照してください。内部 CA、mTLS、ホスト名検証については [`TLS.md`](TLS.md) を参照してください。

## カタログのみのモード

ポリシーでテーブル/列/インデックス/FK カタログだけが許可される場合、`--measure-compression` を省略し、既定の非テーブルサマリーも明示的に無効化します:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

このカタログのみモードでは、テーブルのメタデータ、統計情報、およびカウントのみのトポロジー調査を読み取りますが、行の値やテーブル以外のオブジェクトのインベントリは読み取りません。DBWarpは、テーブルのサイズ、行数、データ型、およびindex/FKの形状から依然として推定できますが、圧縮の推定はtext/binaryのエントロピーを推測する必要があるため、精度が低くなります。`--artifact-detail none`がない場合、既定の要約では、テーブル以外のオブジェクトのカタログも読み取りますが、定義は読み取りません。

## 出力プレビュー

```toml
# dbwarp-blueprint v7
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 7
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 1
row_count = 12500000
table_bytes = 4194304000
index_bytes = 1048576000

[database_topology]
contract = "dbwarp-blueprint-topology/v2"
deployment = "unknown"
local_role = "unknown"
visibility = "unknown"
member_count = 0
member_count_scope = "unknown"
identifiers_redacted = true

[dataset_scope]
contract = "dbwarp-blueprint-dataset-scope/v1"
layout = "unknown"
table_inventory_completeness = "unknown"
row_count_completeness = "unknown"
size_completeness = "unknown"
row_count_method = "postgres-planner-estimate"
size_method = "postgres-local-relation-size"
limitations = ["topology-unobserved", "topology-visibility-unknown"]

[structure_scope]
contract = "dbwarp-blueprint-structure-scope/v1"
visibility = "unknown"
table_inventory_completeness = "unknown"
column_inventory_completeness = "unknown"
index_inventory_completeness = "unknown"
relationship_inventory_completeness = "unknown"
limitations = ["metadata-visibility-unknown"]

[source_environment]
contract = "dbwarp-blueprint-source-environment/v1"
evidence_origin = "database-endpoint"
hosting_model = "unknown"
infrastructure_location = "unknown"
capacity_scope = "connected-instance"
capacity_visibility = "partial"
cpu_capacity_band = "unknown"
cpu_capacity_basis = "unknown"
memory_capacity_band = "under-2-gib"
memory_capacity_basis = "database-buffer-cache"
collector_machine_excluded = true
catalogs_read = ["pg-capacity-settings"]

[statistics_evidence]
contract = "dbwarp-blueprint-statistics-evidence/v1"
visibility = "unknown"
table_count = 1
counts_by_statistics_state = { unknown = 1 }
counts_by_row_count_quality = { unknown = 1 }
counts_by_size_quality = { unknown = 1 }
limitations = ["statistics-provenance-unclassified"]

[artifact_inventory]
contract = "dbwarp-blueprint-artifacts/v2"
detail = "none"
scope = "all-visible-schemas"
visibility = "unknown"
inventory_complete = false
dependencies_complete = false
requirements_complete = false
analysis_complete = false
families_not_inventoried = ["non_table_objects"]

[tables.table-001]
rows = 12500000
table_bytes = 4194304000
index_bytes = 1048576000
schema = "schema-A"
has_clustered_index = false
object_kind = "ordinary-table"
storage_organization = "unknown"
partitioning = "none"
segment_state = "unknown"

[tables.table-001.statistics]
row_count_method = "postgres-planner-estimate"
row_count_quality = "unknown"
statistics_state = "unknown"
refresh_age_band = "unknown"
modification_ratio_band = "unknown"
sample_fraction_band = "unknown"
statistics_scope = "unknown"
size_method = "postgres-local-relation-size"
size_quality = "unknown"
size_scope = "unknown"
size_accounting = "unknown"
size_visibility = "unknown"

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false
numeric_model = "integer"
numeric_precision_radix = "decimal"

[tables.table-001.idxs.idx-1]
type = "btree"
primary = true
unique = true
cols = [1]
```

完全なファイル仕様は [`FORMAT.md`](FORMAT.md) に記載されています。監査ログについては [`AUDIT.md`](AUDIT.md) に記載されています。

## ビジュアルサマリーデッキ

ライブ実行中にデッキを生成します:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

または、レビュー済みBlueprintファイルから後で構築します。この場合、データベース接続はありません:

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx
```

デッキはスキーマサイズに適応します。小規模スキーマではテーブル単位の詳細、大規模スキーマでは特性評価スライド、Tier 2 データがある場合は圧縮サマリー、さらに信頼モデルのスライドを生成します。[`DECK.md`](DECK.md)を参照してください。

## ドキュメント

最初に読む文書:

- [`docs/QUICKSTART.md`](QUICKSTART.md): 最初の安全な実行と、共有すべき内容。
- [`docs/COOKBOOK.md`](COOKBOOK.md): PostgreSQL、MySQL、SQL Server、TLS、デッキ、サンプリングなしのワークフローに関する実用的なレシピ。
- [`docs/DBA_REVIEW_GUIDE.md`](DBA_REVIEW_GUIDE.md): ツール実行前に DBA/セキュリティレビュー担当者が知る必要のある事項。
- [`sql/grants/README.md`](../../sql/grants/README.md): バージョン対応の最小権限付与スクリプトと、取得後のアカウント削除。
- [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md): 一般的な失敗と対処方法。
- [`docs/MESSAGES.md`](MESSAGES.md): 安定した `DBPnnnnS` オペレーターメッセージコード。
- [`docs/COMPRESSION_MEASUREMENT.md`](COMPRESSION_MEASUREMENT.md): Tier 2 圧縮サンプリングの仕組み。
- [`docs/INDEX.md`](INDEX.md): 完全なドキュメントマップ。

セキュリティレビューの開始点:

- [`SECURITY.md`](SECURITY.md): セキュリティモデルと資格情報処理。
- [`AUDIT.md`](AUDIT.md): 読み取り、書き込み、クエリ、ログの内容。
- [`FORMAT.md`](FORMAT.md): 出力フィールドと丸め規則。
- [`TLS.md`](TLS.md): TLS と mTLS の動作。
- [`AUTH.md`](AUTH.md): サポートされる認証モード。
- [`BUILD.md`](BUILD.md): ソースからのビルドとリリース検証。
- [`DECK.md`](DECK.md): 任意の PowerPoint サマリーデッキ。

## ライセンス

Apache-2.0 OR MIT。
