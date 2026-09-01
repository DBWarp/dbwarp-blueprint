# dbwarp-blueprint が読み書きする内容

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../../AUDIT.md)を参照してください。

**言語:** [English](../../AUDIT.md) | [Deutsch](../de/AUDIT.md) | [Français](../fr/AUDIT.md) | [Español](../es/AUDIT.md) | [Polski](../pl/AUDIT.md) | **日本語** | [简体中文](../zh/AUDIT.md)

この文書は、ツールが実行し得るすべての操作を列挙します。お客様の
セキュリティポリシーと照合してください。

## ネットワークエグレス

ライブ `--connect` モードでは、指定されたエンドポイントへのデータベース
ドライバーセッションを 1 つ確立します。バッチモードはソースを順次処理し、
データベースソースごとに 1 つのセッションを確立します。DNS 名前解決では
構成済みリゾルバーを使用する場合があり、統合 Kerberos/SSPI 認証では KDC や
ドメインコントローラーに接続する場合があります。オフラインの TOML、Parquet、
Avro、およびバンドル操作では、アプリケーションが開始するネットワーク接続は
ありません。ただし、ネットワークファイルシステム上のパスは引き続きホストの
ストレージスタックの対象です。

バイナリにテレメトリ、ライセンス確認、バージョン更新、クラウド API 呼び出し、
アップロード経路はありません。

使用するプラットフォームに応じて、`strace -f -e trace=connect,sendto,recvfrom`、
`tcpdump`、または eBPF で検証できます。

## ファイルシステムからの読み取り

本ツールは、アクティブなモードで選択された入力を読み取ります:

| ファイル | 読み取る時点 | 内容 |
|---|---|---|
| `--user-file PATH` | 指定された場合 | ユーザー名のみ。末尾の空白を除去し、空のファイルはエラー。 |
| `--password-file PATH` | 指定された場合 | 1 回読み取り、使用後にゼロ化。全ユーザー/グループが読み取り可能なモードは拒否。 |
| `--azure-token-file PATH` | 指定された場合 | SQL Server Entra ID トークン。1 回読み取り、使用後にゼロ化。全ユーザー/グループが読み取り可能なモードは拒否。 |
| `--tls-ca PATH` | 指定された場合 | 接続時に読み取る信頼済み CA PEM。PostgreSQL/MySQL はバンドルを受け入れ、SQL Server は正確に 1 つの証明書を受け入れます。指定したファイルはエンジンの既定ルートを置き換えます。 |
| `--tls-cert PATH` | 指定された場合 | PostgreSQL/MySQL のクライアント TLS 証明書（PEM）。接続時に読み取ります。SQL Server では `DBP1015E` で拒否されます。 |
| `--tls-key PATH` | 指定された場合 | PostgreSQL/MySQL のクライアント TLS 鍵（PEM）。全ユーザー/グループが読み取り可能なモードは拒否。接続時に読み取り、SQL Server では `DBP1015E` で拒否されます。 |
| `--from-toml PATH` | 指定された場合 | データベース接続なしでデッキを構築するため、ローカルで読み取る既存の dbwarp-blueprint TOML ファイル。 |
| `--from-parquet PATH` | 指定された場合 | Parquet メタデータと、明示的にサンプリングへ同意した場合に限り、上限付きのデコード済み行。 |
| `--from-avro PATH` | 指定された場合 | Avro コンテナーのメタデータとレコード。行数を得るためにコンテナー全体を走査。 |
| `--batch-manifest PATH` | 指定された場合 | マニフェスト、およびマニフェストが参照するすべてのローカル入力、資格情報、トークン、TLS パス。 |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | 指定された場合 | バンドル TOML と、一覧表示、抽出、作成に必要な相対パスの Blueprint ファイル。 |
| `/dev/tty` | パスワードソースが指定されていない場合 | echo を無効にしたプロンプト。 |
| （ビルド時のみ）`rust-toolchain.toml`、`Cargo.toml`、`Cargo.lock`、vendored release の `.dbwarp-source-revision`、`vendor/mysql_async`、`vendor-crates/*` | `./build.sh` の実行時のみ | toolchain、source provenance、標準 Cargo ビルド入力。 |

**読み取らない**もの:
- `~/.pgpass`、`~/.my.cnf`、`~/.aws/credentials`、`~/.azure/credentials`
- すべての `~/.ssh/*` ファイル
- `/etc/passwd`、`/etc/shadow`
- `--password-env`、`--user-env`、または `--azure-token-env` で名前を指定したもの以外のデータベース資格情報変数。統合 Kerberos ビルドでは `KRB5CCNAME` も参照される場合があります。言語と端末表示の変数は以下に記載します。

## ファイルシステムへの書き込み

本ツールが書き込むのは、アクティブなモードで選択された出力だけです:

| ファイル | 書き込む時点 | 内容 |
|---|---|---|
| `--out PATH`（既定は `./blueprint.toml`） | ライブデータベース、Parquet、Avro、bundle-extract、bundle-pack の実行時 | Blueprint またはパック済みバンドル TOML。デッキ専用、bundle-list、dry-run、help/version の各モードでは書き込まれない。 |
| `--deck PATH` | 指定された場合のみ | 匿名化されたBlueprintを要約する PowerPoint (.pptx) デッキ。同じメモリ内Blueprintまたは `--from-toml` 入力からローカルに構築され、追加のデータベース読み取り、ネットワーク、サードパーティライブラリは不要。 |
| `--audit-log PATH` | 指定された場合のみ | stderr に出力される監査ログのアトミックな置換コピー。既存内容への追記ではありません。 |
| `--out-dir DIR` | dry-run 以外のバッチモード | `bundle.toml`、ソースごとの `blueprints/` と `audits/`、所有権マーカー、および一部失敗後の `errors.txt`。公開には同階層のステージングディレクトリと復旧マーカーを使用。 |
| （ビルド時のみ）`./target/`、`./build/` | `./build.sh` の実行時のみ | 標準の Cargo ビルド出力。 |

**書き込まない**もの:
- `/var/log/*`
- `~/.cache/*`、`~/.local/*`、`~/.config/*`
- 暗黙のシステム一時ディレクトリ（ユーザーは出力またはバッチディレクトリを
  明示的に指定することは可能）

## 読み取る環境変数

監査には実際に参照した変数だけが記録されます。`--lang` が対応言語を
選択していない場合、言語選択は `DBWARP_BLUEPRINT_LANG`、`LC_ALL`、`LC_MESSAGES`、`LANG` を参照できます。
端末表示は `NO_COLOR`、`TERM`、`COLORTERM`、`COLUMNS` を参照できますが、
これらは表示だけに影響します。

`--password-env VAR_NAME` または `--user-env VAR_NAME` を指定すると、
本ツールはその名前の変数だけを読み取ります。`PGPASSWORD`、`MYSQL_PWD`、
`MSSQL_PASSWORD`、`USER`、`LOGNAME` などの一般的な既定値への
フォールバックはありません。これらのフォールバックは意図的に
実装されていません。

`./build.sh` の実行時は、`PINNED_RUST`（上書き）、`ALLOW_NETWORK`
（rustup-init ダウンロードへの opt-in）、`TARGET`（cross-compile target）に加え、
標準の cargo / rustup 変数を読み取ります。本ツール自体は実行時に
これらを読み取りません。

## 実行ごとの監査ログ

本ツールは、すべての実行で監査ログを stderr に出力します。形式は
決定論的なプレーンテキストです。`2>audit.txt` でファイルへリダイレクトするか、
明示的なコピーに `--audit-log PATH` を使用します。

サンプル（Tier 1）:

```
=== dbwarp-blueprint audit ===
build_source_revision: 0123456789abcdef0123456789abcdef01234567
build_source_dirty:    false
build_toolchain:     1.94.0 (vendored)
mode:                tier-1
started_at_unix_ms:  1745596800000
outcome:             ok
schema_selector_count: 1

connection:
  - postgresql://app@db.example:5432/payments
    auth: scram-sha-256-or-md5
    tls: yes (protocol version unavailable from driver)
    tls_ca_only: false

auth:
  user_source:        file:/etc/dbwarp/db.user
  password_source:    file:/etc/dbwarp/db.pass (mode 0o600)
  password_persisted: false
  password_logged:    false
  authenticated_principal: (not observed)
  effective_server_principal: (not observed)
  database_principal: (not observed)
  expected_server_principal: (not requested)
  principal_assertion: not-observed

topology_and_scope:
  topology:
    deployment: unknown
    local_role: unknown
    visibility: partial
    member_count: 2
    identifiers_redacted: true
    role_counts: primary=1, secondary=1
    features: postgresql-streaming-replication
    catalogs_read: pg-is-in-recovery, pg-stat-replication
    catalogs_unreadable: (none)
  dataset_scope:
    layout: full-copy
    table_inventory_completeness: complete
    row_count_completeness: complete
    size_completeness: complete
    row_count_method: postgres-planner-estimate
    size_method: postgres-local-relation-size
    limitations: row-counts-statistical

blueprint_fidelity_estimate:
  basis: evidence-coverage-v1
  overall_score: 79/100
  band: good
  structure_score: 90/100
  sizing_score: 100/100
  column_statistics_score: 68/100
  relationship_score: 75/100
  artifact_score: 50/100
  limitations: biased-column-sampling, cardinality-lower-bounds
  qualification: evidence estimate, not source-truth accuracy or a confidence interval

artifact_inventory:
  detail: summary
  visibility: full
  objects: 42
  dependency_edges: 0
  external_prerequisites: 3
  inventory_complete: false
  dependencies_complete: false
  analysis_complete: false

database_operations_observed:
  1. [succeeded, 14ms, 28 rows]   server version lookup
  2. [succeeded, 9ms, 312 rows]   column catalog lookup
  ... (every observed catalog operation enumerated)

wire_bytes_observed:
  catalog_responses: unknown (driver does not expose wire-byte totals)
  row_data:          unknown (driver does not expose wire-byte totals)

local_sample_processing:
  encoded_rowframe_bytes: 0 B

sampling_work:
  compression_workers: 0
  compression_queue_capacity: 0
  compression_jobs_submitted: 0
  compression_jobs_completed: 0
  compression_pipeline_wall_ms: 0
  compression_worker_ms: 0
  tables_skipped_proven_empty: 0
  chunk_level_3_attempts: 0
  table_level_3_attempts: 0
  column_level_3_attempts: 0

files_read_local:
  - /etc/dbwarp/db.pass        (mode 0o600 ✓)

files_written_local:
  - ./blueprint.toml         (12 KiB, sha256: 7f3e2af1...)

warnings:
  - (none)

network_egress:
  - db.example:5432 (the DB connection only)

env_vars_read:
  - (none)

trust_assertions:
  - no row content was read
  - no telemetry was sent anywhere
  - all numeric statistics rounded to documented precision
  - identifier ordering is deterministic (sha256-based)
  - no random or pseudorandom data in output
  - artifact summary stores bounded counts only; no object identities or definitions
  - artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries
  - credential read once via Secret wrapper, zeroized when dropped at end of engine run; see SECURITY.md for driver-owned copy lifetimes (MySQL clones to non-zeroizing String for the driver API)

run_duration_ms:    142
finished_at_unix_ms: 1745596800142
=== end audit ===
```

MySQL の実行では、モード固有の `length policy balanced|strict|exact`
assertion が出力されます。構造上の長さとサンプル長が正確か丸められているかを
個別に示すため、balanced または exact の実行で、監査がすべての数値を
丸めたと主張することはありません。

監査ログは:

- 反復可能なライブ `--schema` セレクターの数だけを記録する。その値は対話型事前表示に示されるが、監査には追加されない。既存の編集済み接続 URI は引き続き接続先データベースを識別し、MySQL ではそれがスキーマ名でもある。選択された Blueprint は `dataset_scope` で `selection-limited` と記録される。
- コンパイル時に埋め込まれたソースリビジョンと worktree 変更の有無を記録する。バイナリは自身の最終ハッシュを埋め込めないため、最終 SHA-256 はリリースまたはレジストリの外部チェックサムとする。
- 資格情報の値ではなく、**ソース**（ファイルパス、環境変数名、TTY）を記録する。
- SQL Server では `ORIGINAL_LOGIN()`、`SUSER_SNAME()`、`USER_NAME()` が
  返す正確なセッション識別情報を記録する。`--expect-server-principal` を
  指定した場合は、期待値とカタログ取得前のサーバー側比較結果も記録する。
- 観測した各データベース操作を、結果、経過時間、ドライバーが提供した場合の行数とともに列挙する。終了時の失敗は識別子を含まない有界ラベルを使う。
- ドライバーが公開しないネットワークバイト数は `unknown` とし、ローカルでエンコードしたサンプルバイトを別に報告する。
- ローカルに書き込んだ総バイト数（各ファイルの sha256 付き）を報告する。
- 致命的でない収集およびサンプリングの機能低下を、安定した DBP 警告コードで記録する。空のセクションは、既知の機能低下が観測されなかったことを意味する。
- 検証済みの `[database_topology]` と `[dataset_scope]` evidence を、closed token と count だけで `topology_and_scope` にコピーする。node 名、endpoint、cluster ID、database ID は出力できない。
- topology または dataset coverage が不完全な場合に `DBP1411W`、`DBP1412W`、`DBP1413W` を保持し、成功した capture が sizing caveat を隠さないようにする。
- 決定論的でディメンション別の Blueprint 忠実度推定を記録する。このスコアは、構造、サイジング、列統計、リレーションシップ、アーティファクトについて取得したエビデンスの網羅度を示す。ソースの真値に対する測定誤差や統計的信頼区間ではない。
- モード（Tier 1 または Tier 2）に適した trust assertion を宣言する。
- 同じ入力に対して決定論的である。同じ DB、同じ引数なら、タイミングフィールドを除き同じ監査になる。

**trust assertion の条件付き出力。**
"credential read once via Secret wrapper..." 行は、
資格情報を実際に読み取った実行でのみ出力されます。資格情報取得前に中止する
失敗経路（URI parse error、URI-embedded password の拒否、dry-run など）では、
意図的にこの行を出力しません。読み取られていない資格情報について
assertion できることはないためです。この行の有無と
`auth.password_source` を併用して、その実行で資格情報処理が
行われたかどうかを判断してください。

**監査は運用上の成功経路と失敗経路で出力されます。** 起動後のコマンドライン
解析エラーも含みます。help/version の終了、および組み込みローカライズ契約を
読み込む前の失敗では完全な監査は出力されません。それ以降の失敗は stderr と、
指定されていれば `--audit-log PATH` に `outcome: error: <stage>` の形式で出力されます。
失敗時の outcome 行の例:

```
outcome:             error: parsing --connect URI (value redacted to avoid logging embedded credentials)
```

端末出力には、因果チェーンとともに `DBP1001E` や `DBP0001E` などの
コード付きオペレーターサマリーも含まれます。監査の outcome は長さが制限され、
長いテキストが切り詰められる場合があります。サポートでの切り分けには、端末出力と
メッセージコードを併用してください。`docs/MESSAGES.md` を参照してください。

任意の RTT、圧縮、text-style probe は、主要なカタログ収集を無効にすることなく
失敗する場合があります。その場合は `DBP1405W` から `DBP1408W` として出力され、
`warnings:` に保持されるため、成功したものの一部だけを収集した Tier 2 結果を
完全な結果と区別できます。監査を制限内かつ機械でスキャン可能に保つため、
同一の警告は重複排除され、複数行の driver detail は 1 行に平坦化されます。

## 非テーブル成果物の読み取り

成果物収集は Tier 2 行サンプリングとは独立しています:

- `--artifact-detail none` は成果物カタログと定義をスキップします。
- `summary` はモデル化されたオブジェクトカタログを読みますが、定義テキストは読みません。
- `graph` は依存カタログも読みますが、定義テキストは読みません。
- `analyzed` は字句解析のため、利用可能な SQL/手続き定義を有界なプロセスメモリへ追加で読みます。

監査は要求詳細、可視性、オブジェクト/依存/外部前提条件の件数、およびすべての完全性フラグを記録します。すべての成果物カタログ操作は `database_operations_observed` に表示されます。任意カタログの失敗は `DBP1410W` を出力し、`warnings` に記録され、不正確な完全性主張を防ぎます。

analyzed モードでは、定義はゼロ化所有者に保持されて消去され、有界なバンドと閉じた機能トークンに削減されます。定義テキスト、ソースオブジェクト名、外部エンドポイント、アーティファクトのプリンシパル、資格情報、鍵/証明書素材、パッケージ/ライブラリ名、バイナリは Blueprint または監査ログへ決して書き込まれません。保持する正確なプリンシパル名は、上記の明示的な `auth` 監査ブロックにある 3 つの SQL Server セッション識別情報だけです。これらは Blueprint、プレゼンテーション、公開アーティファクトには決して書き込まれません。graph と analyzed モードは匿名トポロジでもアプリケーションを識別できるため `--yes` が必要です。

監査は次のいずれかの信頼表明でプライバシー姿勢を区別します:

- summary: 有界件数のみで、オブジェクト ID や定義なし;
- graph: 匿名依存グラフ、定義なし;
- analyzed: 定義を一時的に読み、有界な機能バンドだけを保持。

オブジェクトファミリの範囲と完全性の解釈は [`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) を参照してください。

## Tier 2 の追加処理

圧縮測定を対話的に承認するか、非対話で `--measure-compression --yes` を渡すと、本ツールはさらに次を行います:

- 空であると証明されていない各テーブルについて、エンジン固有の境界付き
  サンプリング経路を実行する。PostgreSQL は
  `TABLESAMPLE SYSTEM(0.1) LIMIT N` から開始し、必要に応じて `LIMIT N` へ
  フォールバックする。MySQL は `LIMIT N`、SQL Server は `TOP N` を使用する。
  バイアスのある経路では出力に `sampled_with_bias = true` を設定する。
- サンプル行をローカルのメモリ内バッファへ読み込む。
- データベース読み取りは逐次のままにする。`--compression-workers N`
  により、1～32 個の境界付きローカル圧縮ワーカーを実行できる
  （ソースホストへの影響を最小限にするため既定値は 1）。ローカル CPU を
  さらに使用する場合は明示的に増やす。各ワーカーは独自の zstd
  コンテキストを所有し、共有 zstd ロックを使用しない。
- レベル 3 で zstd を使用して圧縮する。
- 得られた ratio と stddev を記録する。
- **境界付きローカル圧縮ジョブの完了時に各バッファを破棄する**。
  バイトはディスクへ書き込まれず、送信もされない。ワーカープールが
  保持するのは、キュー内の最大 N サンプルと圧縮中の最大 N サンプルである。

`local_sample_processing.encoded_rowframe_bytes` は圧縮用にローカルで
エンコードしたバイト数であり、データベースのネットワークバイト数ではありません。
ドライバーが公開しない値は `unknown` のままです。`[compression]` ブロックには比率が入ります。`--max-wall-secs` は接続、
カタログ、RTT、Tier 2 を含むライブ収集全体の厳格な期限です。
PostgreSQL はセッション `statement_timeout` も設定し、MySQL は読み取り専用 `SELECT` にセッション
`max_execution_time` を設定します。SQL Server は同等のセッション全体の文経過時間制限を持たないため、セッション
`LOCK_TIMEOUT` を設定します。外側の期限に達すると、クライアントは接続を切断します。
監査は、その切断を SQL Server がキャンセルを確認した証拠とは扱いません。そのため、オペレーターは再試行前にサーバー処理が停止したことを確認する必要があります。

`sampling_work` は識別子を含まない運用証跡です。ローカルのワーカー数と
キューの上限、テーブルごとの 16 MiB の投影ペイロード上限、投入済みおよび完了済みジョブ、圧縮試行、ならびにカタログ
読み取り時点でエンジンのカタログが空であると証明したためサンプリングを
省略したテーブル数を記録します。`compression_worker_ms` はワーカーの
合計経過時間であり、プロセス CPU 時間ではありません。ワーカーが並行して
動く場合は `compression_pipeline_wall_ms` を超えることがあります。
パイプラインの経過時間は、逐次のままのデータベース読み取りと重なる場合が
あります。これらは実行作業のカウンターであり、データベース行数、ネットワーク
バイト測定、またはソース精度の主張ではありません。

## 検証手順

本ツールが文書に記載された操作だけを行うことを*証明*するには:

1. **ソース監査**: リポジトリを clone し、`src/secret.rs` を読んだ後、そのファイル以外にある `\.expose\(\)` を grep します:
   ```
   $ rg -n '\.expose\(\)' src --glob '!secret.rs'
   ```
   本番の call site は公開された `&str` を直ちに driver の connection-builder
   へ渡します。MySQL では `mysql_async` の API が `String` を要求するため、
   さらに `.to_string()` を
   呼び出します。この copy はゼロ化されず、`OptsBuilder` が drop されるまで存続します。
   Tier 1 と Tier 2 は同じ MySQL 接続を再利用します。詳細は SECURITY.md §2 を参照してください。
2. **ソースからビルド**: `./build.sh`。リリース CI は同じ runner 上の別の Cargo target ディレクトリで独立した再ビルドを行い、バイト差があれば拒否します。ローカル比較が有効なのは、source revision、target、features、固定された Rust toolchain、linker、build flags が同一の場合だけです。
3. **リリースと比較**: `./verify.sh release/dbwarp-blueprint-X.Y.Z-...`
4. **実行時トレース**: sandbox 内で `strace -f -e trace=open,connect,read,write` を付けて実行します。上記の一覧と比較してください。
5. **ネットワークトレース**: ホスト上で `tcpdump` を実行します。パスワード認証の
   ライブ実行では、データベースセッションと想定される DNS 通信を確認します。
   統合認証では、想定される KDC またはドメインコントローラーとの通信も考慮します。
   バッチモードでは、データベースソースごとに 1 つのセッションがあることを照合します。

いずれかがこの文書と一致しない場合は、トレースを添えて issue を登録してください。72 時間以内に調査します。
