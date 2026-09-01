# DBWarp Blueprint ファイル形式 v6

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../../FORMAT.md)を参照してください。

**言語:** [English](../../FORMAT.md) | [Deutsch](../de/FORMAT.md) | [Français](../fr/FORMAT.md) | [Español](../es/FORMAT.md) | [Polski](../pl/FORMAT.md) | **日本語** | [简体中文](../zh/FORMAT.md)

人が読めます。差分を取れます。フォレンジックレビューが可能です。

> **この形式は、境界付きスキーマ、決定論的な識別子、文書化された数値精度により、
> 隠れチャネルと直接開示のリスクを低減します。匿名グラフ構造や明示的に有効化した
> 正確なフィールドからワークロードを識別できる場合があるため、組織のデータ分類
> ポリシーに従ってファイルをレビューしてください。**

## ファイルヘッダー

逐語的かつバイト単位で同一:

```
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

空行も契約の一部です。ツールはこのヘッダーだけを正確に出力し、他のコメントは
出力しません。これにより予期しないコメント内容を容易に検出できますが、残りの
構造化フィールドから特徴的なスキーマや依存関係グラフを識別できないという主張では
ありません。

## トップレベルフィールド

| フィールド | 型 | 説明 |
|---|---|---|
| `schema_version` | int | 形式バージョン。現在は `6`。バージョン 1～5 も引き続き読み取り可能です。 |
| `generated_at` | ISO-8601 string | UTC タイムスタンプ。秒精度で小数部なし。バイト単位で同一の再現性実行では、CLI フラグ `--generated-at "2026-04-26T00:00:00Z"` により**固定可能**です。このフラグが設定されると監査ログに `generated_at_pin: ...` が記録されるため、固定がフォレンジック上可視になります。この値を固定する唯一の方法はこのフラグであり、README の「既定では環境変数を読み取らない」という信頼契約に従い、環境変数は一切読み取られません。 |
| `engine` | string | `"postgresql"`、`"mysql"`、または `"sqlserver"`。 |
| `engine_version` | string | データベースエンジンから返されたバージョン文字列。 |
| `source_kind` | string | `"production"`、`"staging"`、`"scrubbed-replica"`、`"synthetic"` のいずれか。顧客が宣言します。 |
| `length_metadata` | string | 従来の互換性マーカー: `"hybrid-v2"`、`"exact"`、`"rounded"`、または `"not-captured"`。新しい consumer は下記の 3 フィールドを使用する必要があります。 |
| `declared_length_fidelity` | string | PostgreSQL の宣言文字容量、および既定の balanced/exact MySQL モードでは `"exact"`、厳格な MySQL プライバシーでは `"coarse-rounded-v1"`、利用できない場合は `"not-captured"`。 |
| `index_length_fidelity` | string | 既定の balanced/exact MySQL index prefix では `"exact"`、厳格なプライバシーでは `"rounded-down-v1"`、利用できない場合は `"not-captured"`。 |
| `observed_length_fidelity` | string | サンプリング時の既定値は `"relative-rounded-v2"`、exact モードでは `"exact"`、strict モードでは `"coarse-rounded-v1"`、または `"not-sampled"`。サンプリングのカバレッジは、引き続き列ごとに独立した要件です。 |
| `[totals]` | inline table | 集約された件数（下記参照）。 |
| `[network]` | table | クライアントからデータベースへの接続とクエリ RTT の任意の証拠。 |
| `[database_topology]` | table | スキーマ v6 のデータベースソースでは必須。プライバシーを保護した deployment、local role、visibility、catalog evidence。構造化ファイルでは省略。 |
| `[dataset_scope]` | table | すべてのスキーマ v6 Blueprint で必須。合計値の範囲と、テーブル、行、バイトの網羅性を宣言します。 |
| `[tables.X]` | tables | テーブルごとに 1 つ。匿名化 ID。 |
| `[fk_edges]` | inline table | 匿名化テーブル間の FK グラフ。任意。 |
| `[artifact_inventory]` | table | プライバシーを保護した非テーブルオブジェクト件数、任意の匿名依存グラフ、外部前提条件、任意の有界言語調査。データベースソースのみ。 |

## `[totals]`

| フィールド | 型 | 精度 |
|---|---|---|
| `table_count` | int | exact |
| `row_count` | int | テーブル単位で丸められた `rows` の合計 |
| `table_bytes` | int | テーブル単位で丸められた `table_bytes` の合計 |
| `index_bytes` | int | テーブル単位で丸められた `index_bytes` の合計 |

これらの数値は自動的にクラスタ全体の合計になるわけではありません。
必ず `[dataset_scope]` と一緒に解釈してください。sharded gateway や
coordinator は、基盤の shard を保持していなくても完全に見える catalog を
公開できます。スキーマ v6 はローカル catalog 統計を暗黙にグローバルな事実と
みなさず、この不確実性を明示します。

## `[database_topology]`（スキーマ v6 データベースソース）

このブロックは、接続したデータベース endpoint から見える有界な事実だけを
記録します。node 名、hostname、IP address、cluster 名、replication channel 名、
server identifier、endpoint は決して保存しません。

| フィールド | 値 / ルール |
|---|---|
| `contract` | 常に `dbwarp-blueprint-topology/v1`。 |
| `deployment` | `single-node`、`replicated`、`sharded`、`distributed`、または `unknown`。 |
| `local_role` | `standalone`、`primary`、`secondary`、`coordinator`、`worker`、`member`、または `unknown`。 |
| `visibility` | `full`、`partial`、または `unknown`。データの正しさではなく topology evidence を示します。 |
| `member_count` | 成功した evidence query から見える member 数。`0` は不明を意味し、member がゼロという意味ではありません。 |
| `identifiers_redacted` | `true` でなければなりません。 |
| `role_counts` | closed role token ごとの任意の件数。full visibility では合計が `member_count` と一致する必要があります。 |
| `features` | `citus`、`mysql-group-replication`、`mysql-galera`、`mysql-ndb`、`postgresql-streaming-replication`、`sqlserver-availability-group`、`vitess` などのソート済み closed token。 |
| `catalogs_read` | 正常に読み取った topology catalog のソート済み closed label。 |
| `catalogs_unreadable` | 読み取れなかった topology catalog のソート済み closed label。1 件でもあれば full visibility を主張できません。 |

通常の endpoint は `deployment = "unknown"` を報告しながら、完全なローカル
full-copy 統計を提供することがあります。cluster feature が見えないという理由だけで、
Blueprint が通常の server を `single-node` と推測することはありません。

## `[dataset_scope]`（スキーマ v6）

このブロックは各 sizing total を独立に限定します。必要な completeness dimension が
`incomplete` または `unknown` の場合、consumer は dataset 全体の無条件な算術を
拒否しなければなりません。

| フィールド | 値 / ルール |
|---|---|
| `contract` | 常に `dbwarp-blueprint-dataset-scope/v1`。 |
| `layout` | `full-copy`、`sharded`、`distributed`、`structured-dataset`、または `unknown`。 |
| `table_inventory_completeness` | `complete`、`incomplete`、または `unknown`。 |
| `row_count_completeness` | `complete`、`incomplete`、または `unknown`。 |
| `size_completeness` | `complete`、`incomplete`、または `unknown`。 |
| `row_count_method` | `postgres-planner-estimate`、`mysql-table-statistics`、`sqlserver-partition-counter`、`distributed-aggregate` などの closed provenance token。 |
| `size_method` | `postgres-local-relation-size`、`mysql-information-schema`、`sqlserver-partition-pages`、`citus-distributed-relation-size`、`distributed-aggregate` などの closed provenance token。 |
| `limitations` | 不完全または不明な範囲を示すソート済み closed reason。すべての dimension が complete でない限り、少なくとも 1 件必要です。 |

`selection-limited` は、合計と完全性の表明が、反復可能なライブ `--schema` セレクターで要求されたスキーマだけを対象とし、接続先データベース全体を対象とするとは主張しないことを意味します。`--schema` を省略すると、表示可能なすべてのスキーマを取得する従来の動作が維持されます。

ネイティブ PostgreSQL、MySQL、SQL Server collector は、ローカル統計が論理 dataset を
表せるか判断する前に、対応する topology catalog を検査します。既知の distributed
gateway は、信頼できる aggregate がない場合に危険な total を抑止します。SQL fallback
formatter には topology probe がないため、有用なローカル推定値を残しつつ、すべての
scope dimension を `unknown` とし、`topology-unobserved` と
`topology-visibility-unknown` を limitation にします。

構造化 Parquet と Avro の Blueprint は `[database_topology]` を省略し、footer/container
provenance とともに `layout = "structured-dataset"` を使用します。

Blueprint は通常の capture 中に storage speed benchmark を実行せず、client を実行する
machine から database server hardware を推測しません。database byte total は指定された
catalog method による保存 data volume を示すだけで、disk type、IOPS、throughput、CPU、
RAM、target migration performance を主張しません。

## `[network]`（任意）

Blueprintツールからソースデータベースまでの、顧客側で観測された
ネットワーク往復統計です。移行元から移行先までの RTT では**ありません**。
これは、実行時にBlueprintツールが顧客のソース DB からどの程度離れていたかを示す
根拠にすぎません。下流の estimator は、オペレーターが指定した移行 RTT の
sanity-check としてのみ使用します（たとえば、顧客のローカルプローブが 0.4 ms
であったのに、オペレーターが移行 RTT を 200 ms と主張するのは不自然です。
Blueprintツールは、おそらくソース DB 自体で実行されていました）。

プローブは接続確立後、カタログクエリの前に実行されるため、query-cache warmup によって
タイミングが偏ることはありません。**5× `SELECT 1`** を実行し、レイテンシーの中央値を
出力します。各 `SELECT 1` は定数の整数 1 を返します。このプローブで行データが
読み取られることはありません。

顧客が `--no-rtt-probe` を渡した場合、またはプローブ自体が途中で失敗した場合は
存在しません（stderr と監査ログへ非致命的警告として記録され、Blueprintファイルは
このブロックなしで引き続き出力されます）。

| フィールド | 型 | 精度 |
|---|---|---|
| `sample_count` | int | exact（v1 では常に 5） |
| `connect_total_ms` | int | TCP 接続開始から認証済みセッションの準備完了までの総 wall-clock（ミリ秒）。TCP handshake + TLS handshake（該当する場合）+ auth challenge/response を含みます。最も近い ms に丸められます。通常は `query_rtt_ms_p50` の 3–6 倍です。 |
| `query_rtt_ms_p50` | int | 5 回の `SELECT 1` サンプルから得た単一往復レイテンシーの中央値（ミリ秒）。最も近い ms に丸められます。自然なネットワークノイズフロア（実際には 1 ms 以上）は丸め粒度より広いため、有用な精度を失うことなく low-bit hidden channel を排除します。sub-ms LAN 値は 0 または 1 になります。 |
| `query_rtt_ms_p95` | int | 5 回のサンプルに nearest-rank 法を適用して求めた 95 パーセンタイル（すなわち最も遅い観測値）をミリ秒で表します。最も近い ms に丸められます。短時間のレイテンシースパイクを把握するには p50 と併用してください。5 回のサンプルはあくまで目安であり、ワークロードのベンチマークではありません。 |

5 回のプローブクエリは、監査ログに `5x SELECT 1 (RTT probe;
constant integer 1, no row data)` というラベルの**単一の要約エントリ**として
記録されます（5 つの別々の行ではありません）。これは、行内容を読み取らないという
信頼姿勢に一致します。

## `[tables.<id>]`

識別子は `table-NNN` です。`NNN` は、スキーマ名とテーブル名をドメイン分離した
HMAC-SHA256 順に並べた 1 始まりの序数です。既定のキーは実行プロセスごとに新しく生成され、出力されません。
同じ顧客管理 `--anonymization-key-file` を渡すと、承認済みの比較実行間で順序が維持されます。

| フィールド | 型 | 精度 / 値 |
|---|---|---|
| `rows` | int | 丸め: 100 単位（≤10k）、1000 単位（≤1M）、10000 単位（>1M） |
| `table_bytes` | int | 大きさに応じて、最も近い 1KiB / 1MiB / 100MiB に丸める |
| `index_bytes` | int | `table_bytes` と同じ丸め |
| `schema` | string | 匿名化 ID `schema-A`、`schema-B`、...、`schema-AA` |
| `kind` | string | Schema v6 の任意の閉じたトークン: `partitioned`、`materialized-view`、`temporal-current`、`temporal-history`、`memory-optimized`、`external`、`graph-node`、`graph-edge`。通常テーブルまたは証拠が不明な場合は省略。 |
| `unlogged` | bool | Schema v6 の任意の PostgreSQL カタログ観測。未取得の場合は省略し、明示的な `false` はカタログが logged table を確認したことを示します。 |
| `partition_strategy` | string | Schema v6 の `partitioned` 用任意トークン: `range`、`list`、`hash`、`key`、`linear-hash`。 |
| `partition_count` | int | Schema v6 の正確な正の leaf partition 数。`kind = "partitioned"` の場合は必須。 |
| `partition_key_cols` | array of int | Schema v6 の単純な partition key の列序数。expression key またはカタログ証拠がない場合は省略し、式はシリアル化しません。 |
| `partition_rows_max` | int | Schema v6 の最大 leaf partition 行数についての任意の丸め推定。 |
| `temporal_history` | string | Schema v6 の対応する `temporal-history` テーブル ID。`temporal-current` では必須。 |
| `counted_in_totals` | bool | Schema v6。省略時はすべての集計に含めます。`external` は明示的な `false` が必須で、`table_count`、`row_count`、`table_bytes`、`index_bytes` から除外します。ほかの明示値は正規形式ではありません。 |
| `check_count` | int | Schema v6 の任意の正確な構造的 CHECK 制約数。省略は不明、`0` は該当カタログが制約なしを確認したことを示します。 |
| `has_clustered_index` | bool | PostgreSQL では常に `false` |
| `stats_freshness` | string | `"fresh"` / `"stale"` / `"never_analyzed"` (PG)。SQL fallback の場合は空 |
| `[tables.<id>.cols.<cid>]` | sub-tables | 列ごとに 1 つ |
| `[tables.<id>.idxs.<iid>]` | sub-tables | インデックスごとに 1 つ |
| `[tables.<id>.compression]` | sub-table | Tier 2 の場合のみ |

## `[tables.<id>.cols.<cid>]`

識別子は `col-N` です。`N` は列の自然な属性順序（1 始まりで、ディスク上の序数を保持）です。
実行間で安定しています。

| フィールド | 型 | 注記 |
|---|---|---|
| `ordinal` | int | ID と同じ N |
| `type` | string | `"integer"`、`"numeric(12,2)"`、`"text"`、`"json"`、`"binary"`、`"timestamp"`、`"uuid"`、`"array<integer>"`、`"user-defined"` などの正規化された型ファミリー。実際の domain、enum、alias、composite、user-defined type の名前は出力されません。 |
| `nullable` | bool | |
| `value_source` | string | Schema v6 の任意の閉じたトークン: `identity-always`、`identity-default`、`auto-increment`、`identity`、`sequence-default`、`generated-stored`、`generated-virtual`、`computed-persisted`、`computed-virtual`、`system-time`、`rowversion`。通常値または証拠不明の場合は省略。 |
| `has_default` | bool | Schema v6 の任意のカタログ観測。省略は不明、明示的な `false` は default がないことをカタログが確認したことを示します。 |
| `default_kind` | string | Schema v6 の任意の分類 `constant`、`function`、`expression`。`has_default = true` の場合のみ有効で、default のテキストやリテラルはシリアル化しません。 |
| `type_kind` | string | Schema v6 の任意の閉じたトークン: `enum`、`set`、`domain`、`composite`、`array`、`range`、`alias`。基本型または証拠不明の場合は省略。 |
| `member_count` | int | Schema v6 の正確な正の構造的 member 数。`enum` と `set` でのみ必須で、member 名はシリアル化しません。 |
| `domain_has_check` | bool | Schema v6 の任意の domain CHECK 観測。`type_kind = "domain"` の場合のみ有効。 |
| `hidden`, `masked`, `encrypted`, `sparse` | bool | Schema v6 の任意のカタログ観測。省略は不明、明示的な `false` はその特性がないことをカタログが確認したことを示します。 |
| `has_check` | bool | Schema v6 の任意の単一列 CHECK 観測。明示的な `true` はすべてテーブルの `check_count` に含まれます。 |
| `null_fraction` | float | `0.0` から `1.0` までの任意の観測 NULL 比率。丸められた集計値のみで、NULL bitmap は保持しません。 |
| `native_type` | string | `varchar` や `longtext` など、任意のサニタイズ済みエンジン基本型。識別子、enum member、default、expression は含みません。現在は修正済み MySQL capture により出力されます。 |
| `declared_max_chars` | int | 任意の宣言済み文字容量。PostgreSQL の `character`/`character varying` カタログ値、および既定の balanced/exact MySQL モードでは exact。MySQL で `--length-fidelity strict` を使った場合のみ粗く丸めます。 |
| `declared_max_bytes` | int | 任意の宣言済みバイト容量。既定の balanced/exact MySQL モードでは exact。`--length-fidelity strict` の場合のみ粗く丸めます。 |
| `numeric_precision`, `numeric_scale`, `datetime_precision` | int | 任意のエンジン宣言済み scalar precision。 |
| `charset`, `collation` | string | 任意のサニタイズ済み MySQL 文字メタデータ。これらはカタログ名であり、顧客の識別子や値ではありません。 |
| `len_avg` | int | 可変長値についてサンプリングされた平均バイト数。既定の relative bucket は最大誤差が約 3.2% で、32 バイトまでの値を正確に保持します。`--length-fidelity exact --yes` では exact。strict モードのみ 10 単位で粗く丸めます。0 = 固定長または未測定。 |
| `len_p95` | int | 同じ既定の relative bucket を使用するサンプリング済み 95 パーセンタイル。`--length-fidelity exact --yes` では exact。strict モードのみ 100 単位で粗く丸めます。0 = 未測定。 |
| `style` | string | Tier 2 のみ。`"json"`、`"xml"`、`"natural-text"`、`"base64"`、`"hex"`、`"numeric-text"`、`"mixed"` のいずれか。分類されない場合は空。 |
| `magnitude_min`, `magnitude_max` | int | Schema v6 の任意の符号付き 10 進指数で、サンプリングした非 NULL 数値の桁を表します。`has_negative` と一緒に出力し、正確な値はシリアル化しません。 |
| `has_negative` | bool | Schema v6 の任意の符号観測。両方の magnitude 境界と一緒にのみ出力。 |
| `time_span` | string | Schema v6 の任意のサンプリング日時範囲: `intraday`、`days`、`weeks`、`months`、`years`、`decades`。 |
| `time_recent_decade` | int | Schema v6 で最新のサンプリング日時を含む decade。`time_span` と一緒にのみ出力し、常に 10 で割り切れる値。 |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Tier 2 のみ。サンプリングされた text/binary 候補列に存在します。テーブルレベルの compression と同じフィールド構成ですが、1 つの匿名化列に限定されます。 |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | スキーマ v3 のサンプリング値分布サマリー。上限付きまたは丸められた件数と頻度のみを含みます。 |

### `[tables.<id>.cols.<cid>.cardinality]`（スキーマ v3）

行サンプリングが有効な場合、コレクターは列ごとに最大 8,192 個の一時的な 64-bit fingerprint をメモリに保持し、NDV と偏りの集計統計を導出した後、fingerprint を破棄します。値も fingerprint もシリアライズしません。このブロックには `measured`、`sample_rows`、`non_null_rows`、`observed_distinct_count`、`estimated_distinct_count`、`top_value_fraction`、`frequency_p50`、`frequency_p95`、`frequency_p99`、`frequency_max`、`sample_method`、`sampled_with_bias`、`bias_reason` が含まれます。

件数と比率は、必要に応じてプライバシー保護のために丸められます。この統計は、合成フィクスチャで重複密度、頻出値の偏り、有限ドメインを再現するためのもので、ソース値や業務上の意味を復元することはできません。

### `[tables.<id>.cols.<cid>.compression]`（Tier 2 のみ）

列単位の圧縮は、`--measure-compression --yes` を使用した場合に限り、制限された
text/binary 候補について出力されます。これにより下流ツールは、テーブルレベルの比率だけに
依存せず、より現実的なエントロピーを持つ合成 text/binary データを生成できます。

このブロックには、`[tables.<id>.compression]` と同じフィールドがあります: `measured`、
`sample_rows`、`sample_bytes`、`sample_method`、`sampled_with_bias`、
`bias_reason`、`ratio_zstd_3`、`ratio_zstd_19`、`ratio_stddev`、
`sample_encoding`。

例:

```toml
[tables.table-001.cols.col-2]
ordinal = 2
type = "json"
nullable = false
len_avg = 430
len_p95 = 0
style = "json"

[tables.table-001.cols.col-2.compression]
measured = true
sample_rows = 1000
sample_bytes = 65536
sample_method = "column TABLESAMPLE SYSTEM(0.1) LIMIT N (text format)"
sampled_with_bias = false
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

サンプリングされた列の値は、Blueprintファイルへ書き込まれません。

## `[tables.<id>.idxs.<iid>]`

識別子は `idx-N` です。`N` は、インデックス名をドメイン分離した HMAC-SHA256 でソートしたテーブル内インデックスの
1 始まりの序数です。

| フィールド | 型 | 値 |
|---|---|---|
| `type` | string | `"btree"`、`"hash"`、`"gin"`、`"gist"`、`"brin"`、`"spgist"`、`"fulltext"`、`"spatial"`、`"clustered"`、`"nonclustered"`、`"clustered columnstore"`、`"nonclustered columnstore"`、`"other"` などの正規化されたインデックス方式ファミリー。extension/custom method の名前は出力されません。 |
| `primary` | bool | 任意。primary-key index では `true` として出力されます。それ以外は省略/false。 |
| `unique` | bool | |
| `cols` | array of int | インデックス列順に参加する列の序数 |
| `prefix_lengths` | array of int | `cols` と整列した任意の MySQL index prefix length。ゼロは列全体を意味します。既定では exact。`--length-fidelity strict` の場合のみ切り下げて丸めます。 |
| `include_cols` | array of int | 任意。ソースエンジンが公開する場合の非キー INCLUDE 列の序数。 |
| `expression` | bool | 任意。expression/function key material が存在し、単純な列序数として表現できない場合は true。 |
| `filtered` | bool | 任意。filtered/partial index の場合は true。 |
| `descending` | bool | 任意。いずれかのキー列が明示的に descending の場合は true。 |
| `prefix_distinct_counts` | array of int | スキーマ v3 で、1 列から N 列までの各キープレフィックスについて推定した distinct tuple 数。ゼロはそのプレフィックスで利用できないことを示します。 |
| `cardinality_sample_method` | string | `prefix_distinct_counts` の上限付き来歴。推論による積は明示的にラベル付けされ、直接の tuple sample としては提示されません。 |

## `[tables.<id>.compression]` および `[tables.<id>.cols.<cid>.compression]`（Tier 2 のみ）

ファイルが `--measure-compression --yes` で生成された場合にのみ存在します。
テーブルレベルのブロックはサンプリングされた行ストリーム全体を測定し、
テーブル全体の転送見積もりについて権威ある比率であり続けます。
列レベルのブロックは同じサンプリング行から列ごとに投影され、下流の
合成フィクスチャジェネレーターが顧客値を見ずに列ごとのエントロピーを
調整するために存在します。追加のデータベース読み取りは発生しません。

| フィールド | 型 | 精度 |
|---|---|---|
| `measured` | bool | ブロックが存在する場合は常に `true` |
| `sample_rows` | int | exact |
| `sample_bytes` | int | メモリ内サンプルバッファのサイズ。**bucketed**: 1 MiB 未満は最も近い **64 KiB**、1 GiB 未満は最も近い **1 MiB**、それ以上は最も近い **100 MiB**。バイトはディスクへ決して書き込まれません。bucketing により、正確な `buf.len()` が公開してしまうテーブルごとの low-bit hidden channel を排除します。 |
| `sample_method` | string | エンジン固有の制限付きサンプリング説明。例: `"TABLESAMPLE SYSTEM(0.1) LIMIT N"`、`"LIMIT N (fallback after empty TABLESAMPLE)"`、`"SELECT TOP N"` |
| `sampled_with_bias` | bool | LIMIT-only fallback など、サンプルが不均一な場合は true |
| `bias_reason` | string | `sampled_with_bias = false` の場合は空。それ以外は `"unordered_limit_after_empty_TABLESAMPLE"` などのタグ |
| `ratio_zstd_3` | float | 最も近い **0.05** に丸める zstd level 3（本番既定値）。`sample_encoding` でエンコードされたバイトについて測定。 |
| `ratio_zstd_19` | float | 旧キャプチャから受け入れるレガシー zstd level 19 比率。ツールはもう測定も出力も行いません |
| `ratio_stddev` | float | 最も近い **0.05** に丸める行境界に揃えた 64 KiB チャンクごとの level-3 比率の stddev。列レベル投影ブロックは、分散モデルではなく補助的なエントロピーヒントであるため、現在 `0.0` を出力します。 |
| `sample_encoding` | string | サンプルを zstd 圧縮する際に使用した byte-level encoding の識別子。現在の値: `"dbwarp-blueprint-rowframe-v1"`。dbwarp estimator は比率を使用する前にこの文字列を検証しなければなりません。同じ論理データでも encoding が異なると異なる比率になり、互換性は**ありません**。古いBlueprintファイルにはこのフィールドが含まれない場合があります。estimator は encoding tag が存在し、認識可能な場合に限って測定比率を使用する必要があります。 |

dbwarp estimator は合成フィクスチャを構築するとき、認識可能な列単位の圧縮ブロックを
優先し、次にテーブル単位の圧縮、最後に type/style 既定値へフォールバックする必要があります。

### `dbwarp-blueprint-rowframe-v1` byte-level encoding

Tier 2 sampler は、この形式で行またはサンプリングされた列値をメモリ内バッファへ連結し、
その後 zstd level 3 を実行します。バッファは破棄され、結果として得られた
丸め済み比率だけがBlueprintファイルへ出力されます。

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

型タグは encoding contract の一部であり、`-v2` suffix bump なしに番号を変更しません。

| タグ | 名前 | 用途 |
|---|---|---|
| 0x00 | Null | SQL NULL（length なし、payload なし） |
| 0x01 | TextUtf8 | UTF-8 text |
| 0x02 | TextUtf16Le | UTF-16LE bytes。主に SQL Server `nvarchar`/`nchar`/`ntext` |
| 0x03 | TextOther | 別の charset のバイト |
| 0x04 | NumberText | 数値の decimal-textual representation |
| 0x05 | BoolText | text としての Boolean |
| 0x06 | TimestampText | ISO-8601 timestamp text |
| 0x07 | DateText | ISO-8601 date text |
| 0x08 | TimeText | `HH:MM:SS[.fff]` text |
| 0x09 | UuidText | 正規の 36 文字 UUID text |
| 0x0F | JsonText | JSON UTF-8 |
| 0x10 | BinaryRaw | `bytea`、`varbinary`、`image`、または blob bytes |
| 0xFE | UnknownText | DB が提供する textual representation への fallback |

### 精度の境界

`ratio_zstd_3` は指定された `sample_encoding` を表すもので、データベースプロトコルや移行転送のバイトを測定したものではありません。公開された自動テストスイートは、決定論的なエンコード、制限付きサンプリング、シリアライズを検証しますが、すべてのエンジンと抽出経路に共通する誤差率を保証するものではありません。

重要な容量判断にこの比率を使用する前に、代表的なソースデータと予定する抽出方法を使って、現在のバイナリとエンジンバージョンを検証してください。比較方法、サンプルサイズ、バイナリハッシュ、エンジンバージョン、観測誤差を結果の計画と共に記録してください。基本関係は、記録された `sample_encoding` が生成するバイト分布における `compressed_bytes ≈ sample_bytes / ratio_zstd_3` です。

## `[fk_edges]`

任意の inline table で、各キーは edge のリストに対応する `table-NNN` ID です。スキーマ v3 は、親列の序数、参照アクション、match mode、遅延可能性、validation/trust state、および任意のプライバシー保護された関係サマリーを保持します。edge は宛先、次に列リストの順でソートされます。

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

任意の `statistics` ブロックは、サンプリングまたは推論された `non_null_rows`、`distinct_parent_values`、`parent_coverage_fraction`、fanout p50/p95/p99/max、`orphan_rows` に加え、来歴とバイアスのフィールドを記録します。検証済みのソース制約は orphan がゼロであることを意味します。列ごとのサンプルから導出した複合推定値は、推論値として明示的に表示されます。ジェネレーターはこれらの集計値を使用して NULL のカバレッジと fanout を再現し、各複合子キーを一貫した 1 つの合成親 tuple に対応付けます。

## `[artifact_inventory]`（スキーマ v4 以降、データベースソース）

独立してバージョン管理される `dbwarp-blueprint-artifacts/v1` 契約は、ソース名や
定義をシリアライズせずに非テーブルオブジェクトを記述します。構造化ファイルソース
および `--artifact-detail none` 選択時には存在しません。

既定の `--artifact-detail summary` は `object_count`、
`external_prerequisite_count`、`counts_by_kind`、
`counts_by_external_class` を出力します。`graph` は成果物ごとの匿名オブジェクト
レコードと依存エッジを追加します。`analyzed` は利用可能な定義から一時的に導出した
有界な `dbwarp-language-feature-census/v1` レコードを追加します。グラフトポロジが
アプリケーションを識別できるため、`graph` と `analyzed` は明示的な `--yes` を必要とします。

インベントリレベルの証拠には次が含まれます:

| フィールド | 値 / 規則 |
|---|---|
| `detail` | `none`、`summary`、`graph`、`analyzed` |
| `visibility` | `full`、`privilege_filtered`、`unknown` |
| `inventory_complete` | 完全な可視性があり、読み取り不能なカタログと宣言済み未モデル化ファミリがない場合だけ true |
| `dependencies_complete` | モデル化された依存カタログを読み取れた場合だけ true |
| `analysis_complete` | analyzed 詳細で、出力した全解析が完全な場合だけ true |
| `catalogs_read` | 正常に検査した標準エンジンカタログの閉じたラベル |
| `catalogs_unreadable` | 失敗したカタログラベル。1 件でもあれば完全性を主張できない |
| `families_not_inventoried` | 現在のコレクター契約外にある既知のオブジェクトファミリ |

オブジェクト ID は `<kind>-NNN` 形式で、例は `view-001`、`function-002` です。
レコードには閉じた kind/subkind/tier トークン、匿名スキーマ/親 ID、匿名依存関係、
未解決依存件数、有界な定義可視性/セキュリティモード、任意の外部前提条件、任意の
言語調査だけが含まれます。ソースオブジェクト名、SQL テキスト、プリンシパル、
エンドポイント、資格情報、鍵、証明書、バイナリは契約フィールドではありません。

外部前提条件は閉じた `class`、デプロイスコープ、未収集のバイナリ/秘密/
エンドポイント素材が必要かどうか、および有界な互換性カテゴリを記録します。その件数は
移行計画の証拠であり、DBWarp が自動プロビジョニングまたは翻訳できるという主張ではありません。

言語調査レコードは `analyzer_version = "lexical-v1"` と
`status = "partial"` を使用します。件数、サイズ、ネスト、複雑度、不透明領域の値は
バンドであり、正確なソース fingerprint ではありません。機能は閉じた語彙から選びます。
解析器はコメント、リテラル、引用識別子を除去します。これはパーサー、意味バインダー、
翻訳成功の保証ではありません。

運用ガイダンスとエンジン範囲は[非テーブル成果物インベントリ](ARTIFACT_INVENTORY.md)を
参照してください。

## ベクター別のステガノグラフィー対策

| ベクター | 防御方法 |
|---|---|
| 識別子の順序 | 秘密のプロセス固有キーを使うドメイン分離 HMAC-SHA256 により、オフラインの候補名検証を防ぎます。安定した実行間ラベルが必要な場合にだけ、顧客管理キーを再利用します。 |
| 数値の low-bit | 統計値は既定で文書化された精度に丸められます。exact-length mode は明示的で同意を必要とし、監査ログに記録され、より機密性の高いメタデータとして扱う必要があります。 |
| 秒未満のタイムスタンプ | 最上部に 1 つの UTC タイムスタンプ。秒精度のみ |
| TOML formatting | 正規化: キーはアルファベット順、固定インデント、挿入コメントなし |
| Sampling randomness | サンプリングは固定 seed（PG の決定論的 `TABLESAMPLE SYSTEM`）を使用します。別途、識別子の匿名化は、顧客がキーを提供しない限り、意図的にオペレーティングシステムの CSPRNG から秘密キーを取得します。 |
| 未使用フィールド | すべてのフィールドを上記で文書化。無制限のデータを保持する "metadata"/"comment"/"reserved" フィールドなし |
| 成果物ソーステキストと外部素材 | 定義は一時的で、有界解析後にゼロ化されます。名前、SQL テキスト、エンドポイント、プロバイダー文字列、資格情報、鍵、証明書、パッケージ名、バイナリにはシリアライズされるフィールドがありません |

## スキーマバージョンの互換性

現在の producer はスキーマバージョン 6 を出力します。バージョン 1～5 は
後方互換性のため引き続き読み取り可能です。v1/v2 ファイルには分布ブロックが
ないため、ジェネレーターは型、
幅、均一なリレーションについて決定論的なフォールバックを使い、忠実度の低下を
報告します。v3 ファイルには分布メタデータがありますが、成果物インベントリは
ありません。v4 ファイルには成果物インベントリが含まれる場合がありますが、現在の
Blueprint 契約識別子より前の形式です。Reader は以前の v4 識別子を入力時に正規化し、
正規 Blueprint 識別子を使用して再出力します。v5 ファイルには、v6 で追加された
トポロジとデータセットスコープの適格性情報がありません。未知の将来バージョンは、
フィールドを黙って破棄せず、明確なアップグレードメッセージとともに拒否する必要があります。

## JSON ではなく TOML を使用する理由

- TOML は構造セクションと leaf data をより読みやすく分離します
  （`[tables.table-001.cols.col-2]` と nested JSON の比較）。
- 差分を取りやすくなります（1 行につき 1 キー。identifier-based sub-table が
  連続した状態を維持）。
- 顧客は、共有前に特定のフィールドを編集して redact できます。

JSON は SQL fallback パスで**中間形式**として使用されます
（`sql/blueprint.pg.sql` が JSON を生成し、`blueprint_format.py` が TOML へ正規化します）。
dbwarp と共有する最終状態のファイルは常に TOML です。

## 構造化ファイルの来歴拡張

スキーマバージョン 3 以降では、次の有界フィールドを出力できます。

構造化ファイルのBlueprintは、データベースのBlueprintと同じ匿名識別子を使用します。
入力の決定論的な順序で `table-NNN`、スキーマの ordinal 順で `col-N` です。
ファイルの stem、Parquet path、Avro field name、manifest の `logical_table` は、
table または column identifier として出力されません。

`engine` または `source_kind` が `"parquet"` か `"avro"` の場合、
`table_bytes` は転送サイズの論理推定値、`storage_bytes` はソースオブジェクトの
実サイズです。デコードサンプリングを行わない Parquet では、非圧縮 column-chunk
bytes を `table_bytes` に使用します。任意のデコードサンプリングを行うと、推定した
`dbwarp-blueprint-rowframe-v1` bytes に置き換わります。Avro は完全なデコード走査から
値を導出します。`source_partitions`、`row_group_count`、`source_codec` は配置と
スケジューリング来歴を示し、複数ファイルの dataset では集約されます。
`row_group_count` は Parquet 固有で、単一入力の `source_partitions` は `1` です。

列単位の `null_fraction` は `0.0` から `1.0` の観測値です。
`length_sample_rows` と `length_sample_method` は `len_avg` と `len_p95` の取得方法を
示します。`source_semantics` は `"repeated-leaf"`、`"nested-json"`、
`"multi-type-union"` などの有限な互換性情報を保持します。小数精度、タイムスタンプ
精度と UTC/ローカル意味論、UUID、固定長バイナリ情報は既存のスカラーフィールドと
`native_type` に保持されます。

table 単位の `ratio_storage` は `table_bytes` とソースオブジェクトの実 bytes を
比較します。Parquet column 単位では、footer の非圧縮/圧縮 column-chunk bytes を
比較します。どちらもファイル計画用であり、DBWarp の転送見積もりではありません。
`ratio_zstd_3` と `ratio_zstd_19` は `sample_encoding` が
`"dbwarp-blueprint-rowframe-v1"` の場合だけ有効です。Parquet footer または Avro
コンテナの比率をこれらの zstd フィールドへコピーしてはなりません。
