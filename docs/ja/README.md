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

英語文書が正本です。機械翻訳された文書セットは、複数の独立したレビューを
経た後に別途提供される場合がありますが、それでも誤りが含まれる可能性があります。

## 概要

DBWarp Blueprint は、信頼を最優先に設計されたデータベース Blueprint コレクターです。PostgreSQL、MySQL、または SQL Server に対して、お客様自身の環境内で実行します。カタログメタデータを読み取り、圧縮測定を指定した場合は上限付きの行サンプルも読み取ります。その後、テーブルサイズ、行数、型ファミリー、インデックスと外部キーの構造を含む、データベースの匿名化された構造 Blueprint を書き出します。

識別子はキー付き匿名ラベルに置き換えられ、行の値が Blueprint に書き込まれることはありません。
既定では、実行ごとに新たに生成されるプロセス固有キーによって、オフラインの辞書照合を防ぎます。
`--anonymization-key-file` を使うと、顧客は承認済みの複数の比較実行でラベルを維持できます。
出力を共有する前に [`SECURITY.md`](SECURITY.md) をお読みください。各モードが開示する情報と、その範囲を拡大するオプションが正確に記載されています。

出力はプレーンテキストファイルです。共有するかどうかを決める前に、すべての行を確認できます。

DBWarp Blueprint は無料のオープンソースソフトウェアであり、すべてお客様の環境内で動作します。データベースそのものを渡さずに、データベースに関する事実を当社へ提供できるようにするためのツールです。

## 実行する理由

Blueprint の出力を当社と共有していただければ、DBWarp がデータをどの程度高速に移動できるか、またそれによって移行、CI/CD テストデータ、分析の各スケジュールがどう変わるかをご説明できます。

距離が最も重要です。データの移動距離が長いほど、DBWarp が示せる改善幅は大きくなります。

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; スイス、チューリッヒ

---

`dbwarp-blueprint` は、DBWarp の顧客側Blueprintコレクターです。顧客自身の環境内で実行し、サニタイズされレビュー可能な `blueprint.toml` ファイルを生成します。DBWarp はこのファイルを使用して、データベースアクセス、ダンプ、スキーマ名、行データを受け取ることなく、移行規模の算定、合成フィクスチャの生成、事前計画を行えます。

PostgreSQL、MySQL、または SQL Server に接続してカタログメタデータを読み取り、必要に応じて制限された行サンプルからローカル圧縮率を測定し、プレーンテキストの TOML を書き込みます。入力がライブデータベースではなく、すでに構造化データファイルである場合は、オフラインモードでローカルの Parquet または Avro ファイルからBlueprintを導出することもできます。出力を開いてすべての行をレビューし、共有するかどうかを判断できます。

任意で `--deck blueprint.pptx` を指定すると、同じ匿名化Blueprintの PowerPoint サマリーも書き込みます。デッキはライブデータベース実行時に生成できるほか、レビュー済み TOML ファイルから `--from-toml blueprint.toml --deck blueprint.pptx` を使って後から生成できます。デッキジェネレーターは Rust バイナリに組み込まれており、ネットワーク接続を行いません。

## 用途

DBWarp が転送を推定し計画するには、十分な構造情報が必要です:

- テーブル数
- 概算行数
- テーブルとインデックスのサイズ
- 列の型ファミリー、正確な構造上の容量/インデックスプレフィックス、および既定でプライバシーに配慮して丸められる観測幅
- インデックスと外部キーの形状
- プライバシーを保護した非テーブル成果物の件数と外部デプロイ前提条件
- 小さなローカルサンプルから得られる、任意のテーブルおよび列の圧縮サマリー
- 任意の顧客側データベース RTT の根拠

これらの事実は、転送サイズを推定し、DBWarp の一括処理開始プランを選択し、代表的な合成ベンチマークフィクスチャを生成するには十分です。顧客のスキーマやデータを復元するには不十分です。

## 行わないこと

`dbwarp-blueprint` は次を行いません:

- テレメトリの送信
- DBWarp サーバーの呼び出し
- Blueprintファイルのアップロード
- `~/.pgpass`、`~/.my.cnf`、クラウド資格情報、SSH 鍵の読み取り
- `PGPASSWORD` や `MYSQL_PWD` など、既定のパスワード環境変数の読み取り
- アクティブなモードで選択された出力以外の書き込み。バッチモードでは、
  子Blueprint、子監査、任意の障害証拠を含むバンドルディレクトリを書き込みます
- 実際のテーブル名、列名、インデックス名、スキーマ名、非テーブルオブジェクト名、SQL 定義、外部エンドポイント、資格情報、鍵、証明書、バイナリ、行の値を出力に含めること

ライブBlueprint実行では、指定されたエンドポイントへのデータベースセッションを
開きます。DNS は設定済みのリゾルバーを使用する場合があり、統合
Kerberos/SSPI 認証は認証基盤へ接続する場合があります。バッチモードでは、
データベースソースごとにこの境界が繰り返されます。ローカル TOML、Parquet、
Avro、およびバンドル操作は、アプリケーションからネットワーク接続を開始しません。

## ダウンロードまたはビルド

| 方法 | 最適な用途 | リンク |
|---|---|---|
| バイナリをダウンロード | 簡単な試用、セールスエンジニアリングの打ち合わせ、隔離されたテストホスト | [`binaries/README.md`](BINARIES.md) |
| 小規模なソースクローンからビルド | セキュリティレビュー、本番ポリシー、再現性確認 | [`BUILD.md`](BUILD.md) |
| vendored ソースバンドルからビルド | 厳格なオフライン依存関係監査 | GitHub Releases |

信頼を優先する方法は、ソースからビルドすることです。通常のリポジトリは小さく保たれ、`Cargo.lock` で依存関係のバージョンを固定します。より厳格なオフライン監査向けに、各リリースではすべての依存関係ソースファイルを含む vendored ソースバンドルも公開します。利便性のために SHA256 チェックサム付きのリリースバイナリも提供します。

## クイックスタート

必要に応じて表示言語を選択してください。英語が既定で、ドイツ語、フランス語、
スペイン語、ポーランド語、日本語、簡体字中国語の完全なカタログが組み込まれています:

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --dry-run
```

翻訳されるのは、人向けのヘルプ、プロンプト、診断、進行状況、PowerPoint デッキの
ラベルだけです。コマンド名とオプション名、受け付ける値、URI スキーム、
環境変数名、セレクター、DBP コード、監査キー、生成される TOML は
正規の英語トークンを維持します。これにより、すべての言語で自動化とサポート手順を
同一に保てます。[`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md)を参照してください。

最初にドライランしてください。接続せずにプランを表示します:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --dry-run
```

TLS、監査ログ、圧縮測定を使用する本番スタイルの推奨実行:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

`--measure-compression --yes` を指定すると、出力にはテーブル単位の
zstd 比率と列単位の圧縮予測が含まれます。列単位の
ブロックは、テーブル単位の比率と同じ制限付きサンプルから計算されます。
DBWarp のフィクスチャ推定を目的としており、サンプル値をディスクへ
書き込みません。スキーマ v3 以降は、プライバシーを保護した列単位のカーディナリティと
分布の集計、および推定されたインデックスプレフィックスとリレーションの要約も出力します。
一時的な fingerprint はメモリ内で上限が設定され、破棄されます。値や fingerprint が
Blueprint TOML に含まれることはありません。

スキーマ v4 以降、Blueprint は非テーブルオブジェクトも収集します。既定の
`--artifact-detail summary` は定義を読まず、オブジェクトクラスおよび外部前提条件
クラス別の有界件数を保存します。`graph` は匿名依存トポロジを、`analyzed` は
有界な言語機能および複雑度のバンドを追加します。匿名グラフでもアプリケーションを
識別できるため、どちらも `--yes` が必要です:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
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
これにより、通常 9 文字の `VARCHAR(3000)` キーを、生成データでは 9 文字付近に保ちながら、
有効なソース DDL/インデックス制限を維持できます:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

ポリシーが追加の精度を許可する場合に限り、正確なサンプル統計を使用してください:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

`--length-fidelity strict` を使用すると、宣言長、観測長、プレフィックス長に対して、
従来の粗い共有安全バケットを維持できます。strict モードは意図的に
フィクスチャ/インデックス忠実度を犠牲にするため、顧客ベンチマークには対応していません。
従来の `--preserve-exact-lengths --yes` という表記は、
`--length-fidelity exact --yes` の互換エイリアスとして残っています。

新しいBlueprintは、`declared_length_fidelity`、
`index_length_fidelity`、`observed_length_fidelity` の各フィールドを
個別に記録します。従来の `length_metadata` フィールドは、古い
consumer との保守的な互換性のために残ります。
PostgreSQL の文字型容量は正確なカタログ値ですが、エンコーディングに依存するバイト上限とインデックスプレフィックス長は引き続き利用できません。

顧客を代表する生成ベンチマークでは、`--measure-compression` は
省略できません。これは観測された average/p95 value length を提供するため、実際の値が数文字しかない
宣言上数キロバイトのキーを、その容量いっぱいで生成することを防ぎます。
既定のサンプリング時間予算は 300 秒です。非常に大きなスキーマでは
`--max-wall-secs` を増やしてください。空でない可変幅インデックス列が未サンプリングのままなら、
後続の計画ツールはBlueprintを拒否する必要があります。その場合、smoke または互換性のための生成には
後続ツールでの明示的な override が必要で、nonrepresentative と記録しなければなりません。

その後、ファイルをレビューします:

```bash
less blueprint.toml
less audit.txt
```

ポリシー上問題がなければ、`blueprint.toml` を DBWarp と共有してください。
デッキもレビュー後に共有できます。監査ログにはエンドポイント、識別情報、パス、
タイミングの詳細が含まれるため、特定のサポート案件で承認済みの安全な経路から
必要とされる場合を除き、アクセス制御された運用証跡として保管してください。

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
制限されたローカルサンプルから DBWarp transport-style compression を推定できます:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

同じフラグを `--from-avro` でも使用できます。サンプル値はメモリ内で
`dbwarp-blueprint-rowframe-v1` としてエンコードされ、集約された zstd 圧縮比だけが
Blueprint TOML に書き込まれます。

## バッチおよびバンドルモード

複数のデータベース、複数のテーブル/データセット、または顧客環境のレビューには、
バッチマニフェストを使用し、バンドルディレクトリを書き込みます:

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
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

Kerberos、SSPI、Entra ID の例については [`AUTH.md`](AUTH.md) を参照してください。内部 CA、mTLS、ホスト名検証については [`TLS.md`](TLS.md) を参照してください。

## カタログのみのモード

ポリシーで行のサンプリングが禁止されている場合は、`--measure-compression` を省略します:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

カタログのみのモードは、メタデータだけを読み取ります。DBWarp はテーブルサイズ、行数、型ファミリー、インデックス/FK の形状から引き続き推定できますが、テキスト/バイナリのエントロピーを推論する必要があるため、圧縮と合成フィクスチャのリアリティは低下します。

## 出力プレビュー

```toml
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 6
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 28
row_count = 12500000
table_bytes = 4200000000
index_bytes = 1100000000

[tables.table-001]
rows = 12500000
table_bytes = 4200000000
index_bytes = 1100000000
schema = "schema-A"
has_clustered_index = false

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false

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

- [`docs/QUICKSTART.md`](QUICKSTART.md): 最初の安全な実行と最初の引き渡しパッケージ。
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
