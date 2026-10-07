# 構造化ファイルのBlueprintソース

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../STRUCTURED_FILES.md)を参照してください。

**言語:** [English](../STRUCTURED_FILES.md) | [Deutsch](../de/STRUCTURED_FILES.md) | [Français](../fr/STRUCTURED_FILES.md) | [Español](../es/STRUCTURED_FILES.md) | [Polski](../pl/STRUCTURED_FILES.md) | **日本語** | [简体中文](../zh/STRUCTURED_FILES.md)

`dbwarp-blueprint` は、ソースがライブデータベースではなく既存のファイルである場合、
ローカルの Parquet および Avro 入力から上限付きの匿名化されたBlueprint TOML を構築できます。

これはオフラインモードです:

- データベース接続なし。
- 資格情報なし。
- テレメトリなし。
- 出力に行の値を書き込まない。
- table と column の identifier は `table-NNN` と `col-N` だけを出力する。
- 監査にはローカルの入力/出力パス、出力ハッシュ、モード、時間、サンプリング作業、
  警告などの通常の運用証跡を記録する。このモードではデータベースエンドポイントを記録しない。

## Parquet

```bash
dbwarp-blueprint \
  --from-parquet /data/orders.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

Parquet モードは footer および row-group metadata を読み取ります。次を導出します:

- ファイルメタデータからの行数。
- Parquet physical/logical type からの列型ラベル。
- definition level からの nullability。
- 完全な列統計が利用できる場合の観測 NULL 比率。
- column-chunk metadata からの粗いエンコード済み平均幅と列ごとのソースストレージ比率。
- ソースオブジェクトのバイト数、row-group 数、partition 数、codec 来歴。

メタデータのみの Parquet 取得では、デコード済み p95 幅を推測しません。任意のデコードサンプリングは、エンコード幅のヒントを、デコードされた `len_avg`、`len_p95`、`null_fraction`、論理 `table_bytes` の観測値で置き換えます。

デコードサンプリングを行わない Parquet では、非圧縮 column-chunk bytes を
論理 `table_bytes` 推定値に使用します。table 単位の `ratio_storage` はその値と
オブジェクトの実サイズを比較し、column 単位の `ratio_storage` は非圧縮/圧縮 chunk bytes を
比較します。これはファイル計画用の信号であり、DBWarp transport compression
ではなく、`ratio_zstd_3` として出力されることはありません。

## Avro

```bash
dbwarp-blueprint \
  --from-avro /data/events.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Avro object container は、Parquet 形式の footer row count を公開しません。そのため、Avro モードは container を一度走査し、record 数を数え、論理 `table_bytes` を導出し、列ごとの `len_avg`、`len_p95`、`null_fraction` を観測します。writer schema は論理型メタデータを提供します。`storage_bytes` と `ratio_storage` は Avro container を表すもので、DBWarp の転送見積もりではありません。

## 論理型の忠実度

構造化されたファイルからのデータ取得は、サイズ決定に必要な、以下の範囲に限定された論理メタデータを保持します。小数点precision/scale、日付と時刻の種類、タイムスタンプの精度、UTC/localの意味、UUID、固定サイズのバイナリ幅、UTF-8文字列、および生のバイト。NULL値のみを含むフィールドは、合成テキストに変換されるのではなく、`type = "null"`のままになります。

ネストされたParquetのデータや、Avroの配列、マップ、レコード、または複数のデータ型を持つユニオンは、単一の正確なSQLのスカラー値として表現できません。Blueprintは、正規化された`json`型と、`source_semantics`のような要素（例：`"repeated-leaf"`、`"nested-json"`、または`"multi-type-union"`）を記録します。これらの列はJSONとしてサイズ設定され、ネストされたスキーマは正確に再現されません。

ソースファイルの stem、Parquet path、Avro field name、batch の `logical_table` label は Blueprint identifier として書き込まれません。複数ファイルの dataset は秘密キーで保護された `table-NNN` identifier を出力し、object bytes、partition、row group、codec、幅、NULL 比率、互換な compression provenance を集約し、構造化された論理 column contract が異なるファイルを拒否します。

## デコード済み圧縮サンプリング

構造化ファイルモードは、任意のデコード済み圧縮サンプリングをサポートします:

```bash
dbwarp-blueprint \
  --from-parquet /data/orders.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

同じフラグを `--from-avro` でも使用できます。

有効にすると、`dbwarp-blueprint` は次を行います:

- ファイルから最大 `--sample-rows` record をデコードする。
- ライブデータベースの Blueprint 取得と同じ一時的な
  `blueprint-compression-probe-v2` 表現を使用して、サンプリングした値を
  エンコードする。
- テーブル単位および列単位の zstd-3 圧縮サマリーを出力する。
- 生成した TOML に `sample_encoding = "blueprint-compression-probe-v2"` を記録する。
- サンプリングしたバイトはメモリ内だけに保持し、行の値をディスクへ決して書き込まない。

`--measure-compression` は、デコードされたデータ値を読み取るため、`--yes` が必要です。これは、集計圧縮、NULL値の密度、cardinality/frequency、長さ、およびスタイルに関する測定値を永続化しますが、サンプリングされた値は永続化しません。

現在のサンプリング機能は、決定的な最初のN件のサンプルを使用します。これは再現性が高く、安価ですが、ファイルがソートまたはクラスタリングされている場合、偏りが生じる可能性があります。重要な見積もりを行う場合は、代表的なファイルを使用するか、異なるシャードから複数のBlueprintファイルを生成することを推奨します。

## 適用範囲

構造化ファイルのBlueprintモードは、次の用途に有用です:

- DBWarp 実行前に Parquet/Avro import のサイズを見積もる。
- Parquet/Avroへのデータベース移行の計画。

実際のソースがサポート対象データベース（PostgreSQL、MySQL、または SQL Server）である場合、これはライブデータベースの Blueprint 取得を置き換えるものではありません。データベースカタログには、汎用ファイルメタデータには存在しないインデックス、キー、FK、statistics-freshness、engine-layout の詳細があります。
