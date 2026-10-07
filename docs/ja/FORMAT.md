# DBWarp Blueprint ファイル形式 バージョン7

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../../FORMAT.md)を参照してください。

**言語:** [English](../../FORMAT.md) | [Deutsch](../de/FORMAT.md) | [Français](../fr/FORMAT.md) | [Español](../es/FORMAT.md) | [Polski](../pl/FORMAT.md) | **日本語** | [简体中文](../zh/FORMAT.md)

人が読めます。差分を取れます。フォレンジックレビューが可能です。

> **この形式は、境界付きスキーマ、秘密キーに基づく識別子、文書化された数値精度により、
> 隠れチャネルと直接開示のリスクを低減します。匿名グラフ構造や明示的に有効化した
> 正確なフィールドからワークロードを識別できる場合があるため、組織のデータ分類
> ポリシーに従ってファイルをレビューしてください。**

## ファイルヘッダー

逐語的かつバイト単位で同一:

```
# dbwarp-blueprint v7
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

空行も正規ヘッダーの一部です。Rust コレクターはこのヘッダーだけを正確に出力し、
他のコメントは出力しません。SQL フォールバックの正規化ツールはヘッダーをそのまま
保持し、受信者が生成元を区別できるよう、キーの取得元を示す固定コメント
`Producer: blueprint_format.py SQL fallback` を 1 つ追加します。残りの構造化
フィールドから特徴的なスキーマや依存関係グラフを識別できないという主張ではありません。

## トップレベルフィールド

| フィールド | 型 | 説明 |
|---|---|---|
| `schema_version` | int | フォーマットバージョン。現在`7`。バージョン1から6までは引き続き読み込み可能です。 |
| `generated_at` | ISO-8601形式の文字列。 | UTCタイムスタンプ、秒単位の解像度、小数部は含まない。**`--generated-at "2026-04-26T00:00:00Z"`** CLIフラグでピン留め可能。バイト単位で完全に同一のライブキャプチャも、同じ保護された**`--anonymization-key-file`**、ソースの状態、オプション、およびコレクタのビルドが必要です。監査ログには、フラグが設定されるたびに**`generated_at_pin: ...`**が記録されるため、ピン留めされた情報はフォレンジック分析で確認できます。どの環境変数もこの値をピン留めしません。 |
| `engine` | 文字列. | `"postgresql"`、`"mysql"`、`"sqlserver"`、`"oracle"`、`"parquet"`、または`"avro"`。`oracle` は、Oracle のプレビューからの出力にのみ表示されます。 |
| `engine_version` | 文字列. | ソースデータベースの数値バージョン。構造化ファイルの場合、これは空になります。ディストリビューションに関する情報は除外されます。 |
| `source_kind` | 文字列. | データベースのソースは、オペレーターが宣言した`"production"`、`"staging"`、`"scrubbed-replica"`、または`"synthetic"`を使用します。構造化されたソースは、`"parquet"`または`"avro"`を使用します。 |
| `length_metadata` | 文字列. | 要約：以前の読者向けのマーカー：`"hybrid-v2"`、`"exact"`、`"rounded"`、または`"not-captured"`。以下の3つのフィールドが正式な情報です。 |
| `declared_length_fidelity` | string | PostgreSQL の宣言文字容量、および既定の balanced/exact MySQL モードでは `"exact"`、厳格な MySQL プライバシーでは `"coarse-rounded-v1"`、利用できない場合は `"not-captured"`。 |
| `index_length_fidelity` | string | 既定の balanced/exact MySQL index prefix では `"exact"`、厳格なプライバシーでは `"rounded-down-v1"`、利用できない場合は `"not-captured"`。 |
| `observed_length_fidelity` | string | サンプリング時の既定値は `"relative-rounded-v2"`、exact モードでは `"exact"`、strict モードでは `"coarse-rounded-v1"`、または `"not-sampled"`。サンプリングのカバレッジは、引き続き列ごとに独立した要件です。 |
| `[totals]` | inline table | 集約された件数（下記参照）。 |
| `[network]` | table | クライアントからデータベースへの接続とクエリ RTT の任意の証拠。 |
| `[database_topology]` | テーブル. | スキーマ v6 以降のデータベースソースでは必須です。スキーマ v7 は、トポロジコントラクト v2 を使用し、各メンバーの数を記録します。構造化ファイルには適用されません。 |
| `[dataset_scope]` | テーブル. | schema-v6 以降のすべての Blueprint で必要です。集計対象の範囲と、テーブル、行、およびバイトの網羅性が完全かどうかを宣言します。 |
| `[structure_scope]` | テーブル. | v7で必須です。テーブル、カラム、インデックス、およびリレーションシップのインベントリの完全性を個別に評価します。 |
| `[source_environment]` | テーブル. | v7データベースのBlueprintで必須であり、構造化されたファイルでは禁止されています。データベースのエンドポイントまたは明示的なプロバイダーを通じて観察された、粗いcapacity/hostingレベルの証拠のみが含まれています。 |
| `[statistics_evidence]` | テーブル. | v7で必須。各テーブルの行数、オプティマイザー統計、およびサイズに関する情報を含む、正確な集計データが必要です。 |
| `[activity_snapshot]` | テーブル. | DBWarp Blueprint 1.6によって書かれていません。 |
| `[tables.X]` | tables | テーブルごとに 1 つ。匿名化 ID。 |
| `[fk_edges]` | inline table | 匿名化テーブル間の FK グラフ。任意。 |
| `[artifact_inventory]` | テーブル. | v7で必須であるため、「要求されていない」「該当しない」「読み取れない」、および検証済みのゼロオブジェクトのインベントリが区別される必要があります。これには、範囲が限定された、名前を含まないオブジェクトの数、オプションの型付き匿名関係、要件、および範囲が限定された言語のカタログが含まれます。 |

## `[totals]`

| フィールド | 型 | 精度 |
|---|---|---|
| `table_count` | int | exact |
| `row_count` | int | 各テーブルのシリアライズされたデータの合計 `rows` です。カタログの推定値は丸められていますが、完全に読み込まれた範囲内のデータは正確です。 |
| `table_bytes` | int | テーブル単位で丸められた `table_bytes` の合計 |
| `index_bytes` | int | テーブル単位で丸められた `index_bytes` の合計 |

これらの数値は、必ずしもクラスタ全体の合計値ではありません。`[dataset_scope]` と合わせて解釈してください。シャーディングされたゲートウェイやコーディネーターは、完全に見えるカタログを表示する一方で、その背後にあるシャードのデータ自体を保持していない場合があります。スキーマのバージョン6と7は、この不確実性を明示的に表現しており、ローカルカタログの統計を黙ってグローバルな真実として扱うことはありません。

`row_count` は、各テーブルのシリアライズされた値の合計であり、2番目の丸められていない測定値ではありません。既知の正の値が最初のプライバシーバケットを下回る場合、それは`100`として表現され、各テーブルごとに`row_count_quality = "engine-estimate"`が使用されます。そのため、多くの小さなテーブルを含む環境では、集計値が保守的に高く評価される可能性があります。その場合、`dataset_scope.limitations`にも`row-counts-statistical`が含まれます。ゼロは、ソーステーブルが空であることを示す証拠として予約されています。

## `[database_topology]` (データベースのソース)

このブロックは、接続したデータベース endpoint から見える有界な事実だけを
記録します。node 名、hostname、IP address、cluster 名、replication channel 名、
server identifier、endpoint は決して保存しません。

| フィールド | 値 / 規則 |
|---|---|
| `contract` | `dbwarp-blueprint-topology/v1` はスキーマ v6 に、`dbwarp-blueprint-topology/v2` は v7 にあります。 |
| `deployment` | `single-node`、`replicated`、`sharded`、`distributed`、または `unknown`。 |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member`, `physical-standby`, `logical-standby`, `snapshot-standby`、または `unknown`。 |
| `visibility` | `full`、`partial`、または `unknown`。データの正しさではなく topology evidence を示します。 |
| `member_count` | 成功した evidence query から見える member 数。`0` は不明を意味し、member がゼロという意味ではありません。 |
| `member_count_scope` | V7のみ: `deployment`、`visible-subset`、`connected-member`、または`unknown`。 完全なトポロジーの可視化には`deployment`が必要です。`connected-member`には、1つのカウントが必要です。 |
| `identifiers_redacted` | `true` でなければなりません。 |
| `role_counts` | closed role token ごとの任意の件数。full visibility では合計が `member_count` と一致する必要があります。 |
| `features` | ソートされたクローズドトークン（例：`citus`）、MySQL replication/cluster形式、`postgresql-streaming-replication`、`sqlserver-availability-group`、`oracle-non-cdb`、`oracle-cdb`、`oracle-pdb`、`oracle-rac`、`oracle-data-guard`、または`vitess`。 |
| `catalogs_read` | 正常に読み取った topology catalog のソート済み closed label。 |
| `catalogs_unreadable` | 読み取れなかった topology catalog のソート済み closed label。1 件でもあれば full visibility を主張できません。 |
| `catalogs_not_applicable` | V7のみ。このソースに適用できないことが証明された、ソート済みのクローズドラベルです。読み取り可能セットおよび読み取り不可能セットと互いに重複しません。 |

通常のエンドポイントは、`deployment = "unknown"` を正当に報告しながら、ローカルの完全コピーテーブル統計を完全に報告できます。Blueprint は、クラスター機能が表示されなかったという理由だけで、特徴のない通常のサーバーを単一ノードとは推定しません。

## `[dataset_scope]` (スキーマ バージョン6以降)

このブロックは、すべてのサイズに関する合計値を個別に評価します。必要な完全性次元が`incomplete`または`unknown`である場合、これらの合計値をデータセット全体の数値として扱わないでください。

| フィールド | 値 / 規則 |
|---|---|
| `contract` | 常に `dbwarp-blueprint-dataset-scope/v1`。 |
| `layout` | `full-copy`、`sharded`、`distributed`、`structured-dataset`、または `unknown`。 |
| `table_inventory_completeness` | `complete`、`incomplete`、または `unknown`。 |
| `row_count_completeness` | `complete`、`incomplete`、または `unknown`。 |
| `size_completeness` | `complete`、`incomplete`、または `unknown`。 |
| `row_count_method` | `postgres-planner-estimate`、`mysql-table-statistics`、`sqlserver-partition-counter`、`oracle-table-statistics`、または`oracle-segment-statistics`のような、クローズドなプロビナンストークン。 `bounded-complete-read`と`mixed-catalog-and-bounded-read`は、完全であることが証明されたTier-2の読み込みから取得された合計値を、単独で、またはカタログのカウントと組み合わせて識別します。 Oracleでは、`not-applicable`が、同じサイズの手法とともに、コピーの合計に含まれるテーブルが一つも存在しない場合にのみ使用されます。`distributed-aggregate`は入力として受け入れられますが、このバージョンでは書き込まれません。 |
| `size_method` | `postgres-local-relation-size`、`citus-distributed-relation-size`、`mysql-information-schema`、`sqlserver-partition-pages`、`oracle-segment-bytes`、`oracle-table-logical-estimate`、`mixed`、または`not-applicable`のような、クローズドなプロビナンストークン。Oracleは、含まれるテーブルが属性セグメントカウンターとラベル付きの論理的な推定値を組み合わせる場合に`mixed`を使用します。また、`not-applicable`は、非空のインベントリにコピー全体の対象となるテーブルが存在しない場合にのみ使用され、完全なゼロの合計が誤って測定方法を主張することを防ぎます。`distributed-aggregate`は入力として受け入れられますが、このリリースでは書き込まれません。 |
| `limitations` | 不完全または不明な範囲を示すソート済み closed reason。すべての dimension が complete でない限り、少なくとも 1 件必要です。 |

`selection-limited` は、合計と完全性の表明が、反復可能なライブ `--schema` セレクターで要求されたスキーマだけを対象とし、接続先データベース全体を対象とするとは主張しないことを意味します。`--schema` を省略すると、表示可能なすべてのスキーマを取得する従来の動作が維持されます。

読み取り可能な選択されたスキーマは、テーブル以外のオブジェクトのみを含むことが正当であり、該当するインベントリに保持されます。ただし、完全なキャプチャにテーブルが全く含まれていない場合、コレクタは定義済みの空のデータセット（テーブル、行、サイズの完全性）を公開してはなりません。代わりに、`table-inventory-visibility-unknown` は、より保守的な境界を示します。

`row-count-evidence-incomplete` と `size-evidence-incomplete` は、少なくとも1つの含まれているテーブルが、対応するカタログ値を持っていなかったことを意味します。 数値の合計は、既知の要素の合計であり、利用できないテーブルがゼロ行またはゼロバイトを含んでいたという主張ではありません。 テーブルごとの統計情報には、どのレコードが利用できないかが示されています。

Oracleの場合、`oracle-segment-bytes` が推奨される、正確な割り当てサイズを示す情報です。テーブルのストレージを `DBA_SEGMENTS` から特定できない場合（たとえば、クラスタ化されたテーブルや、インデックスカタログが利用できないインデックス組織化テーブルの場合）、収集ツールは、すでに利用可能な `DBA_TABLES.NUM_ROWS * AVG_ROW_LEN` の値を使用して、`oracle-table-logical-estimate` を出力する可能性があります。そのテーブルの情報には、`size_quality = "engine-estimate"`、`size_scope = "table-only"`、`size_accounting = "logical-estimate"`、`size_visibility = "partial"` が含まれ、データセットのサイズに関する完全性は `incomplete` です。ただし、LOB やインデックスのバイト数は含まれません。リファインメントカタログが存在しない場合でも、その論理テーブルにすでに割り当てられているバイト数が破棄されることはありません。測定された貢献は `oracle-segment-bytes` のままになり、部分的な可視性と不完全な集計範囲となります。属性を特定できない共有ストレージやインデックス組織化ストレージは、正確なゼロとして公開されません。ドメインインデックスを持つ Oracle テーブルも、Text、Spatial、その他のドメインの実装が、出力されるユーザーテーブルのインベントリ外のセカンダリオブジェクトにバイトを格納する可能性があるため、部分的な可視性と不明確な範囲となります。論理的な代替手段は、割り当てられたバイト数をカウントするのではなく、コピーサイズに関する情報です。ストレージが解放された後（たとえば、`TRUNCATE ... DROP STORAGE` の後）、オプティマイザーの行数が古くなっている場合、現在の割り当てを過大評価する可能性があります。その推定値の出所は保持する必要があります。この代替手段には、`DBA_TABLESPACES` の権限は必要ありません。

Oracleのインデックス格納において、完了したインデックスカタログの読み込みは、論理的なスナップショットの境界となります。境界時点に存在していたものの、対応するインデックスの識別子を持たない`DBA_SEGMENTS`行は、その対象集団外であり、任意のテーブルに割り当てられません。また、境界時点で存在していたものの、期待されるセグメントのデータが不足しているインデックスは、そのテーブルに対して完全なサイズでの可視性を失います。

`logical-partition-root-unmeasured` は、PostgreSQL 特有の証拠であり、含まれている論理パーティションのルートが、意図的に行もバイトも提供しないことを示しています。これは、その値が物理的なリーフに存在するためです。修復可能な `row-count-evidence-incomplete` の統計の欠落とは異なり、別のテーブルを完全に読み取っても、そのようなルートが選択されたインベントリに残っている限り、データセットレベルの完全性を回復することはできません。

`table-inventory-visibility-unknown` は、システムがユーザーオブジェクトとサポートオブジェクトを区別するために必要な、エンジンが所有する分類情報を読み込めなかったことを意味します。 記録は表示されている可能性がありますが、テーブル、行、およびサイズの完全性は失われ、そのサブセットを全体として扱うことはできません。

`catalog-capture-truncated` は、意図されたすべてのカタログセッション、または選択されたスキーマが読み込まれる前に、単一ソースのカタログセッションが停止したことを意味します。 すでに完全であることが証明されているレコードは出力される可能性がありますが、未読の残りの部分については、データセットまたは構造の完全性に関する主張は適用されません。

ネイティブ PostgreSQL、MySQL、SQL Server collector は、ローカル統計が論理 dataset を
表せるか判断する前に、対応する topology catalog を検査します。既知の distributed
gateway は、信頼できる aggregate がない場合に危険な total を抑止します。SQL fallback
formatter には topology probe がないため、有用なローカル推定値を残しつつ、すべての
scope dimension を `unknown` とし、`topology-unobserved` と
`topology-visibility-unknown` を limitation にします。

構造化 Parquet と Avro の Blueprint は `[database_topology]` を省略し、footer/container
provenance とともに `layout = "structured-dataset"` を使用します。

Blueprint は通常の capture 中に storage speed test を実行せず、client を実行する
machine から database server hardware を推測しません。database byte total は指定された
catalog method による保存 data volume を示すだけで、disk type、IOPS、throughput、CPU、
RAM、target migration performance を主張しません。

## `[structure_scope]` (スキーマ v7)

このブロックは、検証済みの空のカタログと、フィルタリングされたカタログ、読み取れないカタログ、または検査されなかったカタログを区別します。

| フィールド. | 値 / ルール |
|---|---|
| `contract` | 常に`dbwarp-blueprint-structure-scope/v1`。 |
| `visibility` | `full`、`privilege-filtered`、または`unknown`。 データの網羅性は、選択されたスキーマとアクセス可能な権限の範囲内であり、データベース全体への無制限なアクセスを保証するものではありません。 |
| `table_inventory_completeness`, `column_inventory_completeness`, `index_inventory_completeness`, `relationship_inventory_completeness` | 独立して`complete`、`incomplete`、または`unknown`。 依存関係のあるグループは、必要な親グループが不完全な場合、完全であることを主張できません。 |
| `catalogs_read`, `catalogs_unreadable`, `catalogs_not_applicable` | ソートされた、互いに排反なクローズドカタログラベル。`catalogs_read` は、正常な読み込みが確認されたことを示します。マルチオーナーキャプチャの場合、意図されたオーナーのうち少なくとも1つの読み込みが成功した場合、別の読み込みが失敗した場合でも、そのカタログが保持されることがあります。`catalogs_unreadable` は、正常な読み込みが一切確認されなかったことを意味します。完全なファミリー（データセット）には、`catalogs_read` にエンジン固有のカタログが含まれている必要があり、意図されたすべてのファミリークエリが完了し、影響を受けたオブジェクトにギャップがあってはなりません。 |
| `limitations` | ソートされたクローズ理由として、`selection-limited`、`metadata-visibility-privilege-filtered`、または`table-kinds-not-inventoried`などが挙げられます。部分的な証拠または不明な証拠の場合は、理由が必要です。 |

スキーマ選択子はスコープの一部です。`complete` は、解決された選択されたスキーマ全体を意味しますが、必ずしもサービス内のすべてのスキーマを意味するわけではありません。どのスキーマにも解決されない選択子はエラーであり、完全な空のBlueprintになってはいけません。

`catalog-capture-truncated` は、構造的な証拠において同じ意味を持ちます。公開されたテーブルと列のレコードは、検証されたプレフィックスまたは所有者の一部であり、残りの意図されたカタログ作業が完了したことを意味するものではありません。その処理で試行されなかったカタログは、3つのカタログセットのいずれにも表示されません。それらを「読み込めない」または「適用できない」と再ラベル付けしてはなりません。

マルチオーナーによる読み込みの場合、`index-inventory-unavailable` が `catalogs_read` に含まれるカタログと共に使用されることがあります。 `relationship-inventory-unavailable` も、そのカタログに付属する場合があります。 カタログのラベルは、正常に処理を行った所有者の肯定的な証拠を保持します。 完全性フィールドと制限レコードは、選択された対象全体が観察されなかったことを示しています。 テーブルごとの制限は、出力されたオブジェクトに表現のギャップがあることを示しますが、これは、テーブルを一切出力しなかった、拒否された、または試行されなかった所有者に関するクエリの状態の証拠を置き換えるものではありません。

Oracleでは、`oracle-identity-columns`と`oracle-constraint-columns`のレコードが、親となるカラムと制約のカタログとは別に管理されます。これらのレコードの有無は、オプションのID生成と関連キーに関する情報を示しており、親カタログが読み取れないという主張に統合されるべきではありません。

## `[source_environment]` (スキーマ v7 のデータベース ソース)

このブロックは、`dbwarp-blueprint`を実行しているワークステーションについて記述することはありません。`collector_machine_excluded`は`true`でなければなりません。キャパシティに関する情報は、接続されているデータベースのエンドポイント、または明示的に承認されたプロバイダー、オーケストレーター、またはオペレーターからの証明のみから得られます。

| フィールド. | 値 / ルール |
|---|---|
| `contract` | 常に`dbwarp-blueprint-source-environment/v1`。 |
| `evidence_origin` | `database-endpoint`、`provider-api`、`orchestrator-api`、`operator-attested`、`mixed`、または`none`。 |
| `hosting_model` | `managed-service`、`self-managed`、`orchestrated`、または`unknown`。 |
| `infrastructure_location` | `cloud`、`on-premises`、`hybrid`、または`unknown`。 |
| `capacity_scope` | `connected-instance`、`database-resource`、`cluster-aggregate`、`member-subset`、または`unknown`。 |
| `capacity_visibility` | `capacity_visibility` は、`full`、`partial`、`unknown`、または `not-requested` である可能性があります。`not-requested` は、不明な容量範囲、基準、および範囲を必要とし、分類された容量カタログは存在しません。容量分類されていない情報、例えば SQL Server エディションなどが存在する場合があります。 |
| `cpu_capacity_band` | `1`, `2`, `3-4`, `5-8`, `9-16`, `17-32`, `33-64`, `65-128`, `129-plus`、または `unknown`。 |
| `cpu_capacity_basis` | `logical-cpu-limit`、`database-resource-limit`、`operating-system-visible`、`physical-host`、または`unknown`。`operating-system-visible`は、VM、コンテナ、またはマネージドサービスの割り当てが、その基盤となる物理ホストであることを主張するものではありません。 |
| `memory_capacity_band` | `under-2-gib`から`512-gib-plus`、または`unknown`までの大まかな範囲。 |
| `memory_capacity_basis` | `database-buffer-cache`、`database-resource-limit`、`operating-system-visible`、`physical-host`、または`unknown`。`operating-system-visible`は、その値がベアメタルではなく、ゲストまたはコンテナを記述する可能性のある、エンジンDMVの保守的な基準です。 `database-buffer-cache`バンドは、構成されたキャッシュ割り当てであり、したがって、ソースの合計メモリの下限にすぎません。これは、その基準なしに、ホストの容量として解釈されるべきではありません。 |
| `member_capacity_uniform` | オプションの observed/attested ブール値。省略されている場合は、不明を意味します。 |
| `features` | `autoscaling`、`burstable`、`container-limits-visible`、`database-resource-governed`、`serverless`、または`shared-host`のような、ソート済みのクローズドな機能事実。 |
| `limitations` | ソート済みのクローズドなプロベナンス制限。`oracle-client-version-mismatch` または `oracle-client-version-unreadable` は、Oracle SQL*Plus クライアントのバージョンを完全に証明できなかったことを示します。`oracle-client-version-below-tested-floor` は、この契約でエンコードされた比較基準である 12.1 より古い、証明済みのクライアントを示します。クライアントバナーのプロベナンスはデータベース構造を決定しないため、カタログ取得は続行されます。 |
| カタログセット | 試行された正確なソース環境カタログに関する、整理された、独立した証拠。SQL Serverのエディション分類も含まれます。ただし、オプションのキャパシティ DMVが読み取れない場合でも含めます。 |

不明な容量はゼロ容量ではありません。リモート接続は、コレクターホストのCPUやメモリを読み取り、それをサーバー容量として再表示することを許可するものではありません。

Oracleの場合、意図されたクエリセットの一部しか完了しなかったキャパシティカタログは、依然として`catalogs_read`という証拠として有効ですが、その値は非表示であり、`capacity_visibility`は`unknown`です。一部の行は、システム全体のCPUまたはメモリ制限として提示されるべきではありません。

Oracle SQL*Plus の機能設定は、実行中のセッションの数値バージョンが読み取れる場合はそれを使い、次に実行可能ファイルのバナーを使い、どちらも読み取れない場合は `ROWLIMIT` も CSV マークアップも出力機能として選択しない保守的なプロトコルを使います。2 回の設定パスはどちらも、継承された `ROWLIMIT` と CSV モードの解除を常に試みます。古いクライアントの不明なオプション診断は、フレーム化されたリセットウィンドウ内でのみ許容されます。バージョンの不一致、部分的な証明、解析失敗、または 12.1 より古い証明済みクライアントは、プロベナンスを弱めるだけです。カタログ取得をブロックすることはありません。

## `[statistics_evidence]` および `[tables.<id>.statistics]` (スキーマ v7)

すべてのv7テーブルには、統計ブロックがあります。最上位のブロックには、`statistics_state`、`row_count_quality`、および`size_quality`による正確なカウントが含まれており、各マップはすべてのテーブルをカバーし、テーブルレベルの分類と完全に一致する必要があります。集計可視性は、すべてのカウントされたテーブルがフルサイズの可視性、既知の行データ、および分類された統計状態を持ち、統計カタログが読み取れないものが存在しない場合にのみ、`full`です。意図的に除外された外部、一時的、および派生オブジェクトは、依然としてインベントリに登録されます。ポリシー主導の行およびサイズの利用不可は、そのコピー集団の可視性を低下させませんが、分類されていない統計状態は依然として可視性を低下させます。すべてのテーブルが除外されている場合、集計可視性は`unknown`で、`statistics-visibility-unknown`です。空の集団は、無意味に`full`を得てはなりません。`catalog-capture-truncated`は、すべての所有者に到達する前に、意図された統計カタログの作業が停止したことを記録しますが、すでに取得された肯定的なカタログ読み取り証拠は保持されます。1つの所有者の読み取りが成功し、別の所有者の読み取りが拒否された場合も、同じ肯定的な証拠ルールが適用されます。カタログは`catalogs_read`のままになり、`statistics-partial`と集計可視性は、選択された集団が完全に観察されなかったことを記録します。

テーブルレベルのフィールドは以下の通りです。

| フィールド. | 値 / ルール |
|---|---|
| `row_count_method` | Engine/version-aware カタログメソッド。 `bounded-complete-read` Tier-2のステートメントが、表示されているテーブル、構造化ファイルカウンタ、または`unknown`を安全に列挙した場合に発生します。 通常のキャプチャは、`COUNT(*)`に自動的に切り替わることはありません。 |
| `row_count_quality` | `exact-counter`, `exact-read`, `engine-counter`, `engine-estimate`, `cached-engine-estimate`、`sample-extrapolation`、`unavailable`、または`unknown`。 既知の正の値を持つSQL Serverのカウンタが、最初のゼロ以外のプライバシーバケットよりも小さい場合、`engine-estimate`が使用されます。これは、シリアル化された`rows = 100`の後に行われます。これにより、プライバシーバケットが、正確なカウンタと測定されたゼロの両方と区別されます。 |
| `statistics_state` | `current`、`possibly-stale`、`known-stale`、`never-analyzed`、`locked`、`user-supplied`、`not-applicable`、または`unknown`。 |
| `refresh_age_band` | `under-1h`、`1h-1d`、`1-7d`、`1-4w`、`1-3m`、`3m-plus`、`unknown`、または`not-applicable`。 |
| `modification_ratio_band` | `none`、`under-1pct`、`1-5pct`、`5-10pct`、`10-20pct`、`20-50pct`、`over-50pct`、`unknown`、または`not-applicable`。 |
| `sample_fraction_band` | `full`、`75-99pct`、`50-74pct`、`25-49pct`、`under-25pct`、`unknown`、または`not-applicable`。 |
| `statistics_scope` | `global`, `partition`, `subpartition`, `session`, `local-member`, `logical-dataset`, `database-resource`, `structured-dataset`, `selected-object`、または `unknown`。 |
| `size_method`, `size_quality`, `size_scope`, `size_accounting`, `size_visibility` | サイズがどこから取得されたのか、それがカウンターなのか推定値なのか、LOB/indexストレージが含まれているのか、割り当てられているサイズなのか論理サイズなのか、そして可視性が完全、部分的、利用不可、または不明であるのかを、それぞれ個別に説明してください。 |

最上位の統計ブロックでは、`visibility = "full"`、`"partial"`、または`"unknown"`が、上記で説明した非空のコピー合計の対象データに使用されます。ただし、`full`の表示のために、除外されたオブジェクトのみを使用することはありません。

Oracle `oracle-segment-bytes` の証拠は、`exact-counter`、`allocated-segment`、完全または部分的な可視性、およびセグメントの集計が属性付けされたことを証明する `segment_state` の場合にのみ有効です (`created`、`deferred`、`mixed`、または `mixed-table-and-index`)。論理的なフォールバックでは、`oracle-table-logical-estimate`、`engine-estimate`、`logical-estimate`、部分的な可視性、および利用できないセグメントの状態が使用されます。これにより、属性付けされていないストレージクラスが測定されたゼロにならないことが防止されます。この状態は、プライバシー保護処理の前に、属性付けされたカタログの証拠から派生します。`created` は、既知の正の値を持つ生のテーブルカウンターが最初のバイトバケットを下回った場合に、シリアル化されたゼロのテーブルバイトに付随する可能性があります。部分的に測定された Oracle の証拠では、`size_scope = "unknown"` が使用されます。属性付けされたバイトは正確ですが、LOB、ネストされたストレージ、またはインデックスのマッピングが欠落している場合、コレクターは完全な table/LOB/index の範囲を正直に主張できません。Oracle では、`mixed` は、正の属性付けされたインデックス割り当てが、丸めによってシリアル化された `index_bytes = 0` に抑制されることを意味します。これにより、セグメントの集計でインデックス割り当てが見つからなかったテーブルとは、ゼロが区別されます。`mixed-table-and-index` は、丸め処理の前に、テーブルとインデックスの両方の割り当てが正であり、両方のシリアル化されたカウンターがゼロであることを意味します。これにより、サブバケットのバイト値を開示することなく、両方の事実は保持されます。`deferred` は、属性付けされた生のカウンターがゼロであり、両方のシリアル化されたバイト値がゼロであることを必要とします。状態を使用して、丸め処理されたサブバケットの割り当てと、マテリアライズされていないことが証明されたストレージを区別してください。

最上位のブロックには、ソートされた、互いに重複しないカタログと、制限事項が記録されます。`rows`または`table_bytes`の値は、サポートとなるquality/visibilityの証拠がある場合にのみ、観測されたゼロとして使用できます。プロベナンスブロックを無視しないでください。行またはサイズの品質が`unavailable`または`unknown`であるテーブルは、対応するデータセットの完全性を`complete`から離れたものにします。バリデータは、完全な網羅性として提示された数値プレースホルダーを拒否します。特に、オプティマイザの統計情報も、完全な範囲の読み取りが証明されていないPostgreSQLテーブルは、行の数が不明であり、測定されたゼロではありません。

Oracle Basicは、辞書が宣言された`NOT NULL`制約と、明示的でテキスト的に同一の`CHECK`制約を区別できない場合、`check_count`を省略します。 また、生成された制約名や、列の現在のNULL許容属性から推測することはありません。 制約の行が明確な他のテーブルでは、正確な件数が残っている場合があります。

## `[activity_snapshot]`

DBWarp Blueprint 1.6 は、このブロックを書き込みません。

## `[network]`（任意）

コレクターを実行しているマシンからデータベースまでの往復時間です。これは、移行のソースとターゲット間の往復時間ではありません。

プローブは接続確立後、カタログクエリの前に実行されるため、query-cache warmup によって
タイミングが偏ることはありません。**5× `SELECT 1`** を実行し、レイテンシーの中央値を
出力します。各 `SELECT 1` は定数の整数 1 を返します。このプローブで行データが
読み取られることはありません。

`--no-rtt-probe` が使用されている場合、またはプローブ自体が実行中に失敗した場合（エラーメッセージとして標準エラー出力と監査ログに記録されます。Blueprint ファイルは、そのブロックを含まない状態で引き続き出力されます）。

| フィールド | 型 | 精度 |
|---|---|---|
| `sample_count` | int | exact（v1 では常に 5） |
| `connect_total_ms` | int | TCP 接続開始から認証済みセッションの準備完了までの総 wall-clock（ミリ秒）。TCP handshake + TLS handshake（該当する場合）+ auth challenge/response を含みます。最も近い ms に丸められます。通常は `query_rtt_ms_p50` の 3–6 倍です。 |
| `query_rtt_ms_p50` | int | 5 回の `SELECT 1` サンプルから得た単一往復レイテンシーの中央値（ミリ秒）。最も近い ms に丸められます。自然なネットワークノイズフロア（実際には 1 ms 以上）は丸め粒度より広いため、有用な精度を失うことなく low-bit hidden channel を排除します。sub-ms LAN 値は 0 または 1 になります。 |
| `query_rtt_ms_p95` | int | 5 回のサンプルに nearest-rank 法を適用して求めた 95 パーセンタイル（すなわち最も遅い観測値）をミリ秒で表します。最も近い ms に丸められます。短時間のレイテンシースパイクを把握するには p50 と併用してください。5 回のサンプルはあくまで目安であり、ワークロード性能テストではありません。 |

5 回のプローブクエリは、監査ログに `5x SELECT 1 (RTT probe;
constant integer 1, no row data)` というラベルの**単一の要約エントリ**として
記録されます（5 つの別々の行ではありません）。これは、行内容を読み取らないという
信頼姿勢に一致します。

## `[tables.<id>]`

識別子は`table-NNN`であり、`NNN`は、スキーマ名とテーブル名のドメイン区切りのHMAC-SHA256順序における1始まりの順序番号です。既定のキーは、プロセスごとに新規に生成され、決して出力されません。同じ保護された`--anonymization-key-file`を渡すと、承認された比較実行間で順序が維持されます。スキーマv7では、`table-001`から出力されたテーブル数までの完全な連続した順序セットが必要です（幅は`table-1000`で自然に増加します）。スキップされた、ゼロ、非10進数、またはソースから派生したサフィックスは無効です。

| フィールド | 型 | 精度 / 値 |
|---|---|---|
| `rows` | int | カタログの推定値は、以下のルールで四捨五入されます：10,000以下の場合は100、100万以下の場合は1000、100万を超える場合は10,000。通常は0に四捨五入されるはずの、正の値の推定値の場合、最初のゼロ以外のバケット(`100`)が使用されます。ゼロは、カタログがゼロであるか、利用可能なデータがないことを示すために予約されています。Tier-2の読み込みで、テーブル全体が完全に網羅されていることが確認された場合、`rows`は、そのサンプルの正確な`sample_rows`によって既に開示されている正確な数値です。これにより、テーブルごとの、カーディナリティ、および集計カウントの矛盾を回避し、新しいチャネルを追加する必要がありません。 |
| `table_bytes` | int | 大きさに応じて、最も近い 1KiB / 1MiB / 100MiB に丸める |
| `index_bytes` | int | `table_bytes` と同じ丸め |
| `schema` | 文字列. | 匿名ID: `schema-A`, `schema-B`, ..., `schema-AA`。スキーマv7では、出力されるテーブルまたはgraph/analyzedオブジェクトによって参照されるすべてのスキーマに対して、連続したアルファベット順の順序セットが必要です。読み取り可能な選択されたスキーマは、テーブル以外のオブジェクトのみを含むことが正当であり、該当するインベントリに保持されます。 |
| `object_kind` | 文字列. | V7では、以下のいずれかのクローズトークンが必要です: `ordinary-table`, `materialized-view`, `external-table`, `temporary-table`, `nested-table`, または `object-table`。オブジェクトの識別子は、物理的なストレージやパーティショニングとは独立しています。 |
| `storage_organization` | 文字列. | V7では、以下のいずれかのクローズトークンが必要です：`heap`、`index-organized`、`clustered`、`external`、または`unknown`。`external`は、`object_kind = "external-table"`でのみ有効です。 |
| `partitioning` | 文字列. | V7では、以下のいずれかのクローズドトークンが必要です: `none`, `range`, `list`, `hash`, `interval`, `reference`, `composite`, `system`, `key`, `linear-hash`, `linear-key`, または `unknown`。 |
| `segment_state` | 文字列. | V7では、クローズドトークンとして`created`、`deferred`、`mixed`、`mixed-table-and-index`、`unavailable`、または`unknown`が必要です。これにより、メタデータのみのオブジェクトと、マテリアライズされたストレージが区別されます。これは、バイト丸め処理の前に確立されるカテゴリカルな証拠であり、したがって`created`は、シリアル化されたテーブルのバイト数がゼロの場合でも、正のサブバケット割り当てを伴うことがあります。Oracleのセグメントカウンターの証拠として、`mixed`は、属性付きのインデックス割り当てが正であることを記録し、そのシリアル化された`index_bytes`がゼロに丸められます。`mixed-table-and-index`は、生の割り当ての両方が正であり、シリアル化されたカウンターの両方がゼロに丸められたことを記録します。 |
| `parent_table`, `child_tables` | 文字列 / 配列 | ネストされた、パーティション化された、またはその他の方法で包含されたオブジェクトに対する、オプションの双方向匿名テーブルリンク。子IDはソートされ、一意です。親グラフは非巡回グラフである必要があります。 |
| `table_features` | 配列. | ソートされたクローズドトークン：`graph-edge`、`graph-node`、`memory-optimized`、`temporal-current`、または`temporal-history`。 |
| `unlogged` | bool | オプションのPostgreSQLログ状態の観測結果。取得されていない場合は省略されます。`false`と明示的に記載されている場合は、そのカタログがテーブルがログされていることを証明しています。 |
| `partition_count` | int | 正確な、対象範囲内の物理的なリーフ・パーティションの数を報告します。これは、`partitioning` が既知のパーティショニング戦略を指す場合に必要です。PostgreSQL は、再帰的なリーフ・パーティションを報告し、解決されたスキーマ外のリーフは除外されます (`selection-limited`)。MySQL の複合テーブルは、サブパーティションを数えます。なぜなら、それらが物理的なリーフだからです。例えば、上位レベルのパーティションが4つで、それぞれに8つのサブパーティションがある場合、`32` が報告されます。0 は、`segment_state = "unavailable"` で定義された論理的なパーティションのルートであり、対象範囲内のリーフが存在しない場合にのみ有効です。 |
| `partition_key_cols` | 整数の配列。 | 単純なパーティションキーの列の番号をすべて記述します。式ベースのキーの場合、またはカタログの情報が利用できない場合は、一部または完全に省略されます。部分的な番号リストとキー式は、いかなる場合もシリアライズされません。 |
| `partition_rows_max` | int | オプションで、最も大きいリーフノードの行数の概算値を丸めて表示します。概算値としてテーブルの合計を表示する場合、既知の正の値は、シリアル化された`rows`によって上限が設定された、最初のゼロ以外の行のバケットを使用します。正確な読み取りが可能なテーブルの場合、プライバシーバケットがゼロになるか、正確なテーブルの総数を超える最大のリーフノードの概算値は、誤った値に丸められるのではなく、表現できないと判断されて省略されます。存在する場合、`rows`が正のときにゼロであってはならず、`rows`を超えてもなりません。 |
| `temporal_history` | 文字列. | ペアになった時間履歴テーブルの匿名テーブルID。 `temporal-current` 機能を使用する場合に必須です。ただし、そのテーブルに適用可能なオブジェクトごとの `table_limitations` トークンが含まれている場合は不要です。 収集範囲全体での選択だけでは、この関連付けは解除されません。 |
| `table_limitations` | 配列. | ソートされた、オブジェクトごとの証拠。 `table-classification-unavailable` は、オブジェクトの種類に関する入力が不完全なテーブルを識別します。`column-inventory-unavailable` は、1つ以上の欠落または読み取り不可能な列レコードを持つテーブルを識別します。`dependent-structure-suppressed` は、そのテーブルのインデックスとリレーションシップ構造の両方が、エミットされた列がないため、完全であると断言できません。`index-inventory-unavailable` および `relationship-inventory-unavailable` は、影響を受ける依存関係ファミリーに限定して、リファインメントのみのギャップを特定し、必要な列のインベントリを削除しません。 存在しないエミットされた列を参照するインデックス、パーティションキー、またはリレーションシップは、全体的なキャプチャを無効にするのではなく、除外されます。`relationship-target-outside-selected-scope` は、このテーブルで宣言された少なくとも1つの外部キーが、解決された選択されたスキーマの範囲外のオブジェクトをターゲットにしていることを記録します。 これは、`selection-limited` キャプチャでのみ有効です。`relationship-target-visibility-unknown` は、カタログが解決できない外部キーのターゲットを公開したことを記録します。 そのため、リレーションシップの完全性は不完全になります。`row-security-filter-active` は、有効なSQL Serverのフィルター述語が検出されたことを記録します。`row-security-visibility-unknown` は、完全なSQL Serverのセキュリティポリシーカタログの可視性を証明できなかったため、Tier-2のサンプリングが抑制され、潜在的にフィルタリングされたサブセットがテーブルの全体として扱われないことを記録します。`temporal-history-outside-selected-scope` は、コレクターが選択範囲外の履歴スキーマを解決した後、`selection-limited` キャプチャでのみ、リンクされていない一時的な現在のテーブルに対して有効です。`temporal-history-visibility-unknown` は、カタログが履歴オブジェクトIDを公開したが、それを解決するための十分なメタデータがないことを記録します。 |
| `counted_in_totals` | bool | 省略とは、含まれることを意味します。 `external-table`、`materialized-view`、`temporary-table`、 またはテーブルに`memory-optimized`が含まれる場合、明示的な`false`が必要です。これにより、外部データ、派生データ、セッション範囲のデータ、または現在測定されていないデータが `table_count`、`row_count`、`table_bytes`、および `index_bytes` から除外されます。各オブジェクトに関する情報は、利用できない値を使用せずに、再作成計画のために引き続き利用可能です。 他の明示的な値は、標準的なものではありません。 |
| `check_count` | int | オプションで、正確な構造CHECK制約の数を指定できます。省略した場合、不明です。`0`は、関連するカタログにそのようなものが存在しないことを意味します。 |
| `has_clustered_index` | bool | PostgreSQL では常に `false` |
| `[tables.<id>.statistics]` | サブテーブル | 行数、オプティマイザ統計の状態、およびサイズに関するv7形式の情報を必須とします。v6形式の`stats_freshness`フィールドは、古いファイルを読み込む場合にのみ有効であり、v7形式では決して出力されません。 |
| `[tables.<id>.cols.<cid>]` | sub-tables | 列ごとに 1 つ |
| `[tables.<id>.idxs.<iid>]` | sub-tables | インデックスごとに 1 つ |
| `[tables.<id>.compression]` | sub-table | Tier 2 の場合のみ |

## `[tables.<id>.cols.<cid>]`

識別子は`col-N`であり、`N`は列の自然な属性順序（1始まり、ディスク上の順序を保持）です。これは実行間で一貫しています。スキーマv7では、10進数の接尾辞は正確に`ordinal`と一致する必要があります。ゼロ、先頭のゼロ、およびソースから派生したラベルは無効です。ソースエンジンは、列が削除された後に、物理的な列の順序にギャップが残る場合があります。

| フィールド | 型 | 注記 |
|---|---|---|
| `ordinal` | int | ID と同じ N |
| `type` | string | `"integer"`、`"numeric(12,2)"`、`"text"`、`"json"`、`"binary"`、`"timestamp"`、`"uuid"`、`"array<integer>"`、`"user-defined"` などの正規化された型ファミリー。実際の domain、enum、alias、composite、user-defined type の名前は出力されません。 |
| `nullable` | bool |  |
| `value_source` | string | Schema v6 の任意の閉じたトークン: `identity-always`、`identity-default`、`auto-increment`、`identity`、`sequence-default`、`generated-stored`、`generated-virtual`、`computed-persisted`、`computed-virtual`、`system-time`、`rowversion`。通常値または証拠不明の場合は省略。 |
| `has_default` | bool | Schema v6 の任意のカタログ観測。省略は不明、明示的な `false` は default がないことをカタログが確認したことを示します。 |
| `default_kind` | string | Schema v6 の任意の分類 `constant`、`function`、`expression`。`has_default = true` の場合のみ有効で、default のテキストやリテラルはシリアル化しません。 |
| `default_on_null` | bool | V7オプションのソースカタログ観察機能（Oracle `DEFAULT ON NULL`用）；既定値が存在する場合にのみ有効です。省略されている場合は、観察されていません。 |
| `type_kind` | string | Schema v6 の任意の閉じたトークン: `enum`、`set`、`domain`、`composite`、`array`、`range`、`alias`。基本型または証拠不明の場合は省略。 |
| `member_count` | int | Schema v6 の正確な正の構造的 member 数。`enum` と `set` でのみ必須で、member 名はシリアル化しません。 |
| `domain_has_check` | bool | Schema v6 の任意の domain CHECK 観測。`type_kind = "domain"` の場合のみ有効。 |
| `hidden`, `invisible`, `masked`, `encrypted`, `sparse` | bool | オプションのカタログに関する情報。 `invisible` は、エンジンによって作成された隠し列とは異なります。 省略されている場合は、不明であることを意味します。 明示的に `false` と示されている場合は、カタログがそのプロパティが存在しないことを証明しています。 |
| `has_check` | bool | Schema v6 の任意の単一列 CHECK 観測。明示的な `true` はすべてテーブルの `check_count` に含まれます。 |
| `null_fraction` | float型 | `0.0`から`1.0`までのオプションの観測されたNULL値の割合。カーディナリティが存在する場合は、そのブロックのプライバシー保護された公開カウントから導出されます。カーディナリティが存在しない場合は、個別に丸められます。NULLビットマップは保持されません。 |
| `native_type` | string | `varchar` や `longtext` など、任意のサニタイズ済みエンジン基本型。識別子、enum member、default、expression は含みません。ネイティブ MySQL および SQL Server collector が出力します。 |
| `declared_max_chars` | int | 任意の宣言済み文字容量。PostgreSQL の `character`/`character varying` カタログ値、および既定の balanced/exact MySQL モードでは exact。MySQL で `--length-fidelity strict` を使った場合のみ粗く丸めます。 |
| `declared_max_bytes` | int | 任意の宣言済みバイト容量。既定の balanced/exact MySQL モードでは exact。`--length-fidelity strict` の場合のみ粗く丸めます。 |
| `length_semantics` | 文字列. | V7オプションで指定可能な長さの単位は、`characters`、`bytes`、`not-applicable`、または`unknown`です。これにより、宣言をシリアライズすることなく、OracleのCHARとBYTEの区別を維持できます。 |
| `numeric_model` | 文字列. | V7では、以下のいずれかのクローズドファミリーが必要です: `integer`, `fixed-decimal`, `unconstrained-decimal`, `decimal-float`, `binary-float`, `not-applicable`, または `unknown`。 `not-applicable` は、既知の数値型ではないことを示します。`unknown` は、意味が分類されていない数値型またはユーザー定義型のために予約されています。`decimal-float` には、正確なOracle `FLOAT(p)` 値が含まれており、IEEE浮動小数点型ではありません。 |
| `numeric_precision` | int | オプションの正の宣言精度で、ソースエンジンとモデルによって制限されます：Oracle `NUMBER` および SQL Server の decimal は合計最大38桁、Oracle `FLOAT(p)` は2進数で最大126桁、MySQL の decimal は合計最大65桁、PostgreSQL の numeric は合計最大1,000桁です。 |
| `numeric_scale` | int | オプションの符号付き宣言されたスケールは、ソースエンジンに対して検証されます。Oracle `NUMBER` は `-84..127` を使用します。PostgreSQL は、バージョンによって異なる、より広い宣言範囲をサポートしますが、MySQL、SQL Server、Parquet、および Avro は、非負で精度を超えるスケールを許可しません。エンジンが許可する場合、負の Oracle/PostgreSQL スケールと、精度を超えるスケールは保持されます。 |
| `numeric_precision_radix` | 文字列. | 数値モデルで必要な場合、`decimal` または `binary` を使用します。Oracle `FLOAT(p)` は、正確な `decimal-float` 値モデルを使用したバイナリ精度を使用し、`BINARY_FLOAT` と `BINARY_DOUBLE` は `binary-float` を使用します。 |
| `numeric_unsigned`, `bit_width` | bool / int | オプションの整数型に関する機能。ソースエンジンがそれらの機能を提供する場合に適用されます。 |
| `datetime_precision` | int | オプションのエンジンで宣言されたdate/timeの分数精度。 |
| `charset`, `collation` | string | 任意のサニタイズ済み文字メタデータ。MySQL はカタログの charset 名と collation 名を出力します。SQL Server は `nchar`/`nvarchar`/`ntext` に `utf-16le`、コードページ 65001 に `utf-8`、Windows コードページ 1250–1258 に `windows-N`、その他の正のカタログコードページに `code-page-N` を出力し、カタログの collation 名も出力します。これらはエンコード上の事実とカタログ名であり、あなたの識別子や値ではありません。 |
| `len_avg` | int | 可変長値についてサンプリングされた平均バイト数。既定の relative bucket は最大誤差が約 3.2% で、32 バイトまでの値を正確に保持します。`--length-fidelity exact --yes` では exact。strict モードのみ 10 単位で粗く丸めます。0 = 固定長または未測定。 |
| `len_p95` | int | 同じ既定の relative bucket を使用するサンプリング済み 95 パーセンタイル。`--length-fidelity exact --yes` では exact。strict モードのみ 100 単位で粗く丸めます。0 = 未測定。 |
| `style` | string | Tier 2 のみ。`"json"`、`"xml"`、`"natural-text"`、`"base64"`、`"hex"`、`"numeric-text"`、`"mixed"`、`"precompressed"` のいずれか。分類されない場合は空。`"precompressed"` は、認識済みの標準コンテナシグネチャを持つ、バイト量で実質的に支配的なバイナリ値サンプルにだけ出力されます。検出したコンテナファミリーは意図的に開示しません。 |
| `[tables.<id>.cols.<cid>.lob_storage]` | サブテーブル | V7オプションのデータベースLOBストレージに関する情報：`storage_class` (`basicfile`, `securefile`, `external`, `unknown`)、圧縮 (`none`, `low`, `medium`, `high`, `not-applicable`, `unknown`)、重複排除 (`enabled`, `disabled`, `not-applicable`, `unknown`)、オプションのin-row/encryptedフラグ、および可視性 (`full`, `partial`, `unknown`)。外部コンテンツの場合、2つのストレージ制御が`not-applicable`である必要があり、データベース内のフラグは省略されます。パスまたはセグメント名は保持されません。 |
| `magnitude_min`, `magnitude_max` | int | Schema v6 の任意の符号付き 10 進指数で、サンプリングした非 NULL 数値の桁を表します。`has_negative` と一緒に出力し、正確な値はシリアル化しません。 |
| `has_negative` | bool | Schema v6 の任意の符号観測。両方の magnitude 境界と一緒にのみ出力。 |
| `time_span` | string | Schema v6 の任意のサンプリング日時範囲: `intraday`、`days`、`weeks`、`months`、`years`、`decades`。 |
| `time_recent_decade` | int | Schema v6 で最新のサンプリング日時を含む decade。`time_span` と一緒にのみ出力し、常に 10 で割り切れる値。 |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Tier 2 のみ。サンプリングされた text/binary 候補列に存在します。テーブルレベルの compression と同じフィールド構成ですが、1 つの匿名化列に限定されます。 |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | スキーマ v3 のサンプリング値分布サマリー。上限付きまたは丸められた件数と頻度のみを含みます。 |

`numeric_model` は数値の意味に関する正式な定義です。`type` はエンジンファミリーの表記を維持します。Oracleの`NUMBER`ファミリーには、`type = "number"`と`FLOAT(p)`があり、`"float"`がそれに伴います。また、`native_type`は、元の宣言を修正したものを保持しています。

### `[tables.<id>.cols.<cid>.cardinality]`（スキーマ v3）

行サンプリングが有効になっている場合、コレクタは各列につき最大8,192個の一時的な64ビットフィンガープリントをメモリに保持し、集計統計NDV/skewを算出後、フィンガープリントを破棄します。 値もフィンガープリントもシリアライズされません。 このブロックには、`measured`、`sample_rows`、`non_null_rows`、`observed_distinct_count`、`estimated_distinct_count`、`top_value_fraction`、`frequency_p50`、`frequency_p95`、`frequency_p99`、`frequency_max`、`sample_method`、`complete_source_read`、`sample_layout`、`sampled_with_bias`、および`bias_reason`が含まれています。 `complete_source_read = true` は、ある範囲内で観測された完全なソースデータセット全体が、値のレベルでの切り捨てなしに、この列を保持していることを示す、機械可読な証拠です。 検証済みの完全な行読み込みは、`sample_rows`をテーブルの正確な行領域内に維持します。ただし、セルの上限または境界付きフィンガープリントリザーブによって`complete_source_read`がfalseになる場合でも、その状態が維持されます。 正確なテーブルの内容はすでに`tables.<id>.rows`に記載されているため、これは追加の情報を提供しません。 `non_null_rows` はプライバシー保護処理が最初に行われ、その後、`null_fraction` が `(sample_rows - non_null_rows) / sample_rows` として算出されます。 したがって、その割合は公開されている数値と完全に一致し、別の情報を開示することなく、独立した0.005の割合のグリッドから取り除くことができます。 正確なゼロ値と、すべてのNULL値を持たないエンドポイントは、そのまま保持されます。 混合集計では、正の非NULL値を持つ人口を維持し、`sample_rows`を下回るようにします。 Mixed `non_null_rows` は、他のカーディナリティカウントと同様に、同じ相対的なカウントグリッドを使用します。 この状況下では、カウント値が実際の保持されているデータ量よりも最大で1つのカウントバケット分少なくなることがあります（例えば、`9,728` が `9,999` の場合）。 これは、ほぼ正確な数値ではありません。 正確な、すべての項目が非NULLであるという結果は、保持された行の中にNULL値が一つも存在しないことを意図的に示しています。一方、NULL値が一つでも検出されると、カウントは`sample_rows`を下回ります。 個別の値と頻度カウントは、その対象集団によって制限されていても、文書化されたプライバシー基準に基づいて維持されます。 データベースのソースの場合、不完全な読み込みは、丸められたカタログの行数の見積もり`sample_rows`で上限が設定されます。 ParquetおよびAvroの場合、正確なフッター行数は上限です。 両方のパスにおいて、`sample_rows` は、テーブルレベルの圧縮ブロックの `sample_rows` によって既に開示されている正確な保持行数です。ただし、その上限が下回る場合は除きます。 完全に網羅されていることを示唆するように、決して上方へ制限してはなりません。 値の切り捨ては、区別性や頻度に関する情報を保守的に保ち、テーブル全体のデータに基づいて、それを過大評価してはなりません。 `sample_method` の解析によって完全性を推測しないでください。 `sample_layout` は、オプションの機械可読な列挙型です。 現在の出力値は`primary-key-range-windows`です。 欠如とは、注文契約が利用できない状態を意味します。 人間が読める`sample_method`フィールドを解析して意味を推測しないでください。

カウントと割合は、必要に応じてプライバシー保護のために丸められています。これらの統計は、重複の密度、特定の値の偏り、および有限の範囲について説明しています。これらにはサンプリングされた値は含まれていませんが、特徴的な分布はワークロードを特定する可能性があります。これらを不可逆的なものとして扱ったり、外部の知識からビジネス上の意味を推測できないことの証明として扱ったりしないでください。制限された範囲の記述は、可視な行の数を示すことができますが、すべてのサンプリングされたセルが完全に保持されていることを証明するものではありません。サーバー側のセルの上限が列を切り捨てる場合、そのカーディナリティは、テーブルの行数が完全な範囲の読み込みから記録されている場合でも、制限された、偏った推定値のままです。影響を受けていない列は、完全な読み込みによるカーディナリティの情報を含んでいる可能性があります。

### `[tables.<id>.cols.<cid>.compression]`（Tier 2 のみ）

列単位の圧縮は、text/binary の条件を満たす場合にのみ適用されます。`--measure-compression --yes` が使用される場合に、列単位の圧縮の見積もりを提供します。

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
sample_method = "TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "server_side_cell_cap"
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "blueprint-compression-probe-v2"
```

サンプリングされた列の値は、Blueprintファイルへ書き込まれません。

バイナリ列では、同じ制限付き Tier 2 サンプルから、粗い
`style = "precompressed"` プロファイルが出力される場合があります。認識は
サンプル値の境界でのみ行われ、バイト量で実質的に支配的な観測を必要とします。
Blueprint は値を解析または展開せず、シグネチャを保持せず、この 1 つの高信頼
ラベルを超えて画像、アーカイブ、圧縮メディア、暗号化済み、ランダムなペイロードを
区別しません。テキストおよび base64 エンコードは引き続きテキストスタイルで分類し、
事前圧縮済みバイナリコンテナとして扱いません。

## `[tables.<id>.idxs.<iid>]`

識別子は`idx-N`であり、ここで`N`はテーブル内のインデックスの1始まりの順序番号です。インデックス名は、ドメインで区切られたHMAC-SHA256でソートされます。スキーマv7では、各テーブルに対して密なセット`idx-1`から`idx-N`が必要です。ゼロ、先頭のゼロ、欠落、および10進数以外のサフィックスは無効です。

| フィールド | 型 | 値 |
|---|---|---|
| `type` | string | `"btree"`、`"hash"`、`"gin"`、`"gist"`、`"brin"`、`"spgist"`、`"fulltext"`、`"spatial"`、`"clustered"`、`"nonclustered"`、`"clustered columnstore"`、`"nonclustered columnstore"`、`"other"` などの正規化されたインデックス方式ファミリー。extension/custom method の名前は出力されません。 |
| `primary` | bool | 任意。primary-key index では `true` として出力されます。それ以外は省略/false。 |
| `unique` | bool |  |
| `cols` | array of int | インデックス列順に参加する列の序数 |
| `prefix_lengths` | array of int | `cols` と整列した任意の MySQL index prefix length。ゼロは列全体を意味します。既定では exact。`--length-fidelity strict` の場合のみ切り下げて丸めます。 |
| `include_cols` | array of int | 任意。ソースエンジンが公開する場合の非キー INCLUDE 列の序数。 |
| `expression` | bool | 任意。expression/function key material が存在し、単純な列序数として表現できない場合は true。 |
| `filtered` | bool | 任意。filtered/partial index の場合は true。 |
| `descending` | bool | 任意。いずれかのキー列が明示的に descending の場合は true。 |
| `partitioning` | 文字列. | V7オプションの物理パーティション：`none`、`local`、`global`、または`unknown`。 |
| `visibility` | 文字列. | V7オプションのソース可視性：`visible`、`invisible`、または`unknown`。 |
| `state` | 文字列. | V7 オプションの動作状態：`usable`、`unusable`、`in-progress`、`failed`、または `unknown`。 |
| `prefix_distinct_counts` | array of int | スキーマ v3 で、1 列から N 列までの各キープレフィックスについて推定した distinct tuple 数。ゼロはそのプレフィックスで利用できないことを示します。 |
| `cardinality_sample_method` | string | `prefix_distinct_counts` の上限付き来歴。推論による積は明示的にラベル付けされ、直接の tuple sample としては提示されません。 |

## `[tables.<id>.compression]` および `[tables.<id>.cols.<cid>.compression]`（Tier 2 のみ）

`--measure-compression --yes`でファイルが生成された場合にのみ表示されます。テーブルレベルのブロックは、完全なサンプルデータの中立的なカラム形式での投影であり、テーブル全体のデータ転送の見積もりにおける信頼できる比率です。カラムレベルのブロックは、同じサンプル行から、1つのカラムずつ投影され、どのカラムが効率的に圧縮されるかを示しますが、サンプルデータは公開されません。これらは、追加のデータベースへの読み込みを引き起こしません。

PostgreSQLのテーブルで、アクティブな行レベルセキュリティが適用されているもの、および、直接の子クエリによって親ポリシーがバイパスされる継承された子テーブルやパーティションの子テーブル、また、有効なセキュリティフィルター述語によって制御されているSQL Serverのテーブルは、サンプリングされません。これらのテーブルのカタログ情報は保持されますが、実行記録`DBP1407W`は、ポリシーでフィルタリングされたサブセットをテーブル全体として外挿するのではなく、記録されます。

| フィールド | 型 | 精度 |
|---|---|---|
| `measured` | bool | ブロックが存在する場合は常に `true` |
| `sample_rows` | int | exact |
| `sample_bytes` | int | メモリ内サンプルバッファのサイズ。**bucketed**: 1 MiB 未満は最も近い **64 KiB**、1 GiB 未満は最も近い **1 MiB**、それ以上は最も近い **100 MiB**。バイトはディスクへ決して書き込まれません。bucketing により、正確な `buf.len()` が公開してしまうテーブルごとの low-bit hidden channel を排除します。 |
| `sample_method` | string | エンジン固有の範囲指定サンプリングの説明。例：`"TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"`、`"LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"`、または`"SELECT TOP N bounded projection FROM <table> (compression sample; server-side cell cap)"`。 |
| `sampled_with_bias` | bool | LIMIT-only fallback など、サンプルが不均一な場合は true |
| `bias_reason` | string | `sampled_with_bias = false` の場合、このフィールドは空です。それ以外の場合、`"unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"` のようなタグが含まれています。 |
| `ratio_zstd_3` | float | contract の zstd level 3 測定ポリシーに従い、最も近い **0.05** に丸めます。`sample_encoding` でエンコードされたバイトについて測定。 |
| `ratio_zstd_19` | float型 | このリリースで記述されたものではなく、以前のバージョンのファイルに記載されている可能性があります。 |
| `ratio_stddev` | float | 最も近い **0.05** に丸める、制限付きテーブル probe frame ごとの level-3 比率の stddev。列レベル投影ブロックは、分散モデルではなく補助的なエントロピーヒントであるため、現在 `0.0` を出力します。 |
| `sample_encoding` | 文字列. | 測定に使用される、バイトレベルのエンコーディングと圧縮セッションポリシーの識別子です。PostgreSQLのライブテーブルブロックは`"blueprint-columnar-transfer-probe-v2"`を使用しています。MySQLとSQL Serverは`"blueprint-columnar-transfer-probe-v3"`を使用しており、さらに256 KiBのプローブチャンク境界でフラッシュを行います。SQL Serverの`nvarchar`、`nchar`、`ntext`ペイロードは、元のUTF-16LEのバイト分布を保持します。`varchar`、`char`、`text`は、サンプリングされたバイト幅を保持し、`charset`フィールドはカタログのコードページを識別します。V1は入力として受け入れられます。列ごとのブロックは`"blueprint-compression-probe-v2"`を使用します。異なる`sample_encoding`の値で測定された比率は比較できません。 |

PostgreSQLはv2を使用し、MySQLとSQL Serverはv3を使用します。エンコーディング内でのみ、比率を比較してください。

### `blueprint-compression-probe-v2` byte-level encoding

Tier 2 sampler は、この形式で行またはサンプリングされた列値をメモリ内バッファへ連結し、
その後 zstd level 3 を実行します。バッファは破棄されます。Blueprint が保持するのは、
文書化された圧縮、NULL 密度、カーディナリティ/頻度、長さ、スタイルの集約フィールドだけです。

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

型タグは probe contract の一部であり、新しい versioned probe identifier なしに
番号を変更しません。

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

### `blueprint-columnar-transfer-probe-v1`、`v2`、`v3` の byte-level encoding

ライブデータベースのテーブル比率では、同じ制限付き列 v2 サンプルを中立な
1,000 行 frame に変換します。各 frame には versioned probe header があり、各列に
ordinal、1 つの type tag、行ごとの 4-byte length、その後に列連続の payload bytes
が続きます。`0xffffffff` の length は NULL を表します。バイト表現は 3 つの
version で共通です。V1 は連結した frame sequence を、入力サイズを宣言した 1 回の
zstd level-3 operation として圧縮しました。V2 は frame を 1 つの永続的な zstd
level-3 context に渡し、各 frame 後に flush します。V3 はその context と中立な
row-group 表現を保持しつつ、row group 内の 256 KiB probe compression chunk 境界
でも flush します。MySQL と SQL Server の capture は v3 を使用し、v2 は現在の
PostgreSQL measurement です。SQL Server Unicode text は UTF-16LE として測定します。
SQL Server narrow text は source
byte width を保持し、collation code page から得た closed かつ sanitized な charset
を記録します。外側の row-group output が `ratio_stddev` の observation を提供します。
versioned tag により、framing または flushing policy が別の policy として暗黙に
再解釈されることを防ぎます。

この表現は、列指向 bulk transfer に共通する、圧縮に関連する一般的な特性をモデル化
します。データベースプロトコルの capture、migration wire format、または encoded
data export ではありません。sample bytes はメモリ内だけに保持され、集約測定値の
導出後に破棄されます。

### 精度の境界

`ratio_zstd_3` は、指定された `sample_encoding` に関する記述であり、データベースプロトコルや移行処理のデータそのものを記録したものではありません。このリポジトリ内のテストスイートは、決定的なエンコーディング、範囲付きサンプリング、およびシリアル化を検証しますが、すべての抽出パスに対して、あらゆるデータベースエンジン間で普遍的な誤差率を主張するものではありません。

比率を重要なキャパシティ決定に使用する前に、代表的なソースデータと意図した抽出メカニズムに対して、その比率を検証してください。比較方法、サンプルサイズ、バイナリハッシュ、エンジンバージョン、および結果として得られたプランにおける観測されたエラーを記録してください。基本的な関係は、記録されたエンコーディングによって生成されたバイト分布の下で`compressed_bytes ≈ sample_bytes / ratio_zstd_3`です。

## `[fk_edges]`

任意の inline table で、各キーは edge のリストに対応する `table-NNN` ID です。スキーマ v3 は、親列の序数、参照アクション、match mode、遅延可能性、validation/trust state、および任意の境界付きで名前を含まない関係サマリーを保持します。edge は宛先、次に列リストの順でソートされます。

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

任意の `statistics` ブロックは、サンプリングまたは推論された `non_null_rows`、`distinct_parent_values`、`parent_coverage_fraction`、fanout p50/p95/p99/max、`orphan_rows` に加え、来歴とバイアスのフィールドを記録します。検証済みのソース制約は orphan がゼロであることを意味します。列ごとのサンプルから導出した複合推定値は、推論値として明示的に表示されます。

## `[artifact_inventory]` (スキーマバージョン4以降; バージョン7で必須)

スキーマ v7 は、独立したバージョン管理の `dbwarp-blueprint-artifacts/v2` コントラクトを使用して、テーブル以外のオブジェクトを記述しますが、ソースの名前や定義をシリアル化しません。古いスキーマのバージョンは、v1 コントラクトを保持しています。v7 は常にこのブロックを出力します。`--artifact-detail none` は、要求されていないデータベースのインベントリを明示的に記録し、構造化ファイルソースは、適用されないインベントリを明示的に出力します。したがって、欠落したブロックは、検証済みの空のカタログと誤解されることはありません。

既定の `--artifact-detail summary` は `object_count`、
`external_prerequisite_count`、`counts_by_kind`、
`counts_by_external_class` を出力します。`graph` は成果物ごとの匿名オブジェクト
レコードと依存エッジを追加します。`analyzed` は利用可能な定義から一時的に導出した
有界な `dbwarp-language-feature-census/v1` レコードを追加します。グラフトポロジが
アプリケーションを識別できるため、`graph` と `analyzed` は明示的な `--yes` を必要とします。

`object_count`は、収集器が生成する成果物レコードの数であり、特定のネイティブカタログから返される行数ではありません。したがって、パッケージまたは型は、それぞれ個別の仕様、本体、およびメンバーレコードに貢献する可能性があります。複数のカタログに存在するネイティブオブジェクトは、依然として1つのレコードです。たとえば、Oracleのトリガー行は、トリガーカタログとソースカタログの両方から取得されますが、これらの行はネイティブオブジェクトの同一性によって結合され、ソース行はトリガーレコードを重複させるのではなく、それを補完します。

Oracleのパッケージとオブジェクト型は、同じレコード構造を使用します。それは、`specification`レコードで、実装としてリンクされた`body`レコード、そしてカタログの各メンバーに対して1つの`package_member` procedure/functionレコードで、このレコードの仕様が親となっています。パッケージ全体のソーステキストと言語の統計は、ボディのみが所有します。メンバーはカタログの情報は保持しますが、適用されない定義分析を使用します。これにより、字句解析器がパッケージのソースをメンバーのボディに分割できると誤解することを防ぎます。

インベントリレベルの証拠には次が含まれます:

| フィールド | 値 / 規則 |
|---|---|
| `detail` | `none`、`summary`、`graph`、`analyzed` |
| `scope` | V7: `all-visible-schemas`、`selected-schemas`、`structured-source`、または`unknown`。これらは、ファイル内の他の場所にあるスキーマ選択の証拠と一致する必要があります。 |
| `visibility` | `full`、`privilege_filtered`、`unknown` |
| `inventory_complete` | 完全な可視性があり、読み取り不能なカタログと宣言済み未モデル化ファミリがない場合だけ true |
| `dependencies_complete` | モデル化された依存カタログを読み取れた場合だけ true |
| `requirements_complete` | V7 集約値。選択範囲の評価対象が完全に網羅され、出力されたすべての成果物で `requirement_status = complete | not_applicable` の場合にのみ true。省略は false を意味し、空の要件リストは完全性の証拠ではない |
| `analysis_complete` | analyzed 詳細で、出力した全解析が完全な場合だけ true |
| `catalogs_read` | 正常に検査した標準エンジンカタログの閉じたラベル |
| `catalogs_unreadable` | カタログのラベルでエラーが発生した場合、それぞれのエラーは、そのカタログからの完全性に関する情報を妨げます。ただし、関連するオブジェクトごとの要件に関する情報は、引き続き完全である可能性があります。 |
| `catalogs_not_applicable` | 適用できないことが証明された V7 カタログラベルです。読み取り可能なカタログと読み取り不可能なカタログの両方と互いに重複しません。 |
| `families_not_inventoried` | このリリースでインベントリされていない既知のオブジェクトファミリー |

### `[artifact_inventory.complexity]` (スキーマ v7)

`dbwarp-blueprint-artifact-complexity/v1` ブロックは、匿名化されたデータセットに関する集計評価です。これは、`none` および `summary` の詳細情報には存在せず、`graph` および `analyzed` の詳細情報には必要です。ここで、存在する場合、その評価が試行されたことを意味します。計算が失敗した場合、Blueprint の処理を中断するのではなく、失敗した状態の不明な結果が生成されます。

最上位のフィールドは固定されています。

| フィールド. | 値 / ルール |
|---|---|
| `contract` | `dbwarp-blueprint-artifact-complexity/v1` |
| `assessor_version` | `1` |
| `scope` | `artifact_inventory.scope`と完全に一致する必要があります。 |
| `population_policy` | `exclude-known-engine-generated-and-secondary`; 欠落したオプションは引き続き有効であり、一時的なオブジェクトも引き続き有効です。 |
| `assessment_population_complete` | これは、人口ポリシーに基づいて対象となるすべてのオブジェクトが既知である場合にのみ真となります。省略されている場合は偽であり、この主張は、より広範な`inventory_complete`フィールドとは独立しています。 |
| `eligible_object_count` | ポリシーによって評価されるオブジェクト。 |
| `fully_assessed_object_count` | すべての次元は、既知であるか、証明されている`not-applicable`。 |
| `partially_assessed_object_count` | 少なくとも1つの適用可能な次元が既知であり、少なくとも1つの次元が不明。 |
| `unassessed_object_count` | 該当する次元は不明です。 |
| `excluded_object_count` | 記録されたポリシーによって除外されるオブジェクト。 |
| `analyzer_version` | v7のデータ収集で使用される単一の解析器は、`lexical-v2`、またはグラフモードでは`not-applicable`です。 |
| `analysis_spans` | 適格なセンサスレコードにある、ソート済みで一意のクローズド範囲トークン：`executable-body`、`not-applicable`、または `unknown`。グラフモードでは空です。 |
| `dialects` | 適格なセンサスレコードにある、ソート済みで一意のクローズド方言トークン。 |
| `grammar_profiles` | 対象となる国勢調査データに含まれる、一意でソートされた文法プロファイル。 |
| `overall_band` | `trivial`、`low`、`moderate`、`high`、`very-high`、`not-applicable`、または`unknown` |
| `overall_score` | このリリースによって作成されたものではありません。 |
| `limitations` | 以下に、分類された、完了理由を説明します。 |

両方の人口計算式では、検証された演算が使用されています。

```text
artifact_inventory.object_count = eligible_object_count + excluded_object_count
eligible_object_count = fully_assessed_object_count
                      + partially_assessed_object_count
                      + unassessed_object_count
```

`dimensions`には、正確に`volume`、`control_flow`、`feature_breadth`が含まれています。 `entanglement`、`environment_coupling`、`opacity`、および`dialect_coupling`。 各次元には、クローズドな `band` があり、`coverage` 値 (`complete`, `partial`, `not-applicable`、または`unknown`、および1つの固定ヒストグラム。 `volume`次元`band`は、他の次元の評価と同様に、`trivial`、`low`を使用します。 `moderate`, `high`, `very-high`, `not-applicable`, または `unknown`。 そのヒストグラムは、サイズキー `0`、`1-255`、`256-1k`、`1k-4k` を使用します。 `4k-16k`、`16k-64k`、および`64k+`。 他の6つのヒストグラムは、カウントキー`0`、`1`、`2-4`、`5-8`を使用します。 `9-16`、`17-32`、および`33+`。 すべてのヒストグラムには、`not_applicable` と `unknown` のバケットがあります。 各次元において、検証された算術演算には以下のものが必要です。

```text
eligible_object_count = assessed evidence-band counts
                      + not_applicable
                      + unknown
```

カバレッジは、したがって、単一の全体レベルのビットではなく、各次元ごとに測定されます。`not_applicable` の調査結果は、決定的な証拠とみなされ、その次元の `not_applicable` バケットに寄与しますが、オブジェクトが評価されなくなるわけではありません。最上位の fully/partially/unassessed オブジェクトの数は、派生した要約です。すべて「該当なし」のオブジェクトは完全に評価され、「部分的に評価」とは、少なくとも1つの該当する次元が既知であり、別の次元が不明であることを意味し、「評価なし」とは、該当する次元が1つも既知でない（すべて不明）ことを意味します。

部分的な次元は、下限と上限として評価されます。 その`band`は`unknown`ですが、既知の下限がすでに`very-high`である場合は、そうではありません。なぜなら、未知の観測値は、最も高いバケットを占める可能性があるからです。 これは、部分的なヒストグラムが、観測された下限値を最終的な結果として提示することを防ぎます。

`external_binary` は定義の可視性を示す状態であり、除外フラグではありません。サイトにインストールされたプラグイン、CLRアセンブリ、Javaオブジェクト、および外部ライブラリは、引き続き移行の対象となり、通常は定義に依存する未知の情報を持ち込みます。`generated_by_engine = true` フラグを明示的に指定した場合に限り、アッセッサー v1 でエンジンが提供するオブジェクトが除外されます。

ヒストグラムは、意図的に一次元で構成されています。種類、機能、スキーマ、またはその他の属性によるクロス集計は、契約に含まれていません。正確なカウントは、分析モードでシリアル化されたオブジェクトごとの集計データに追加の情報を提供しません。また、固定された形状は、公式なデータベース構造の特定を容易にする情報を公開することを避けるためのものです。

制限が適用される理由には、`definition-analysis-not-requested`、`definitions-withheld`、`unsupported-dialect`、`wrapped-source`、`graph-incomplete`、`requirements-incomplete`、`outside-selected-scope`、`computation-limit`、および`computation-failed`が含まれます。`requirements-incomplete`は、要件の収集が完了していないことを意味します。要件ステータスが`partial`または`unavailable`のオブジェクトは、環境や方言に関する不明な情報を提供するため、ゼロとは見なされません。一方、完全に完了したオブジェクトは評価されます。`unsupported-dialect`は、定義が取得されたものの、その言語または方言に対応するアナライザーが存在しないことを意味します。これは、意図的に隠蔽されているわけではありません。`definition-analysis-not-requested`は、グラフの詳細に使用されます。定義の読み込みに関する制限を主張することはできません。なぜなら、そのような読み込みは試行されていないからです。

不明な証拠は、影響を受ける各次元について個別に制限されます。全体的な範囲は、下限と上限の評価が一致した場合にのみ出力されます。完全に空の、有効な対象グループは`not-applicable`であり、決して`trivial`ではありません。グラフモードでは、常に`unknown`の全体的な範囲が、空でない対象グループに対して使用されます。これは、全体的な評価に必要な定義を読み取らないためです。グラフのエッジが欠落している場合、影響を受ける関連性の証拠は不明になります。`computation-limit`は、このバージョンでは書き込まれません。予期しない計算エラーが発生した場合、`computation-failed`が記録され、完全な成果物インベントリが保持され、集計評価はBlueprintを抑制する代わりに、失敗としてマークされます。

`assessment_population_complete` は、広範囲な `artifact_inventory.inventory_complete` ではなく、限定された結果が確定的であるかどうかを決定する基準となります。評価対象が不完全な場合、既知の下限がすでに `very-high` でない限り、全体の範囲は `unknown` です。見えない可能性があるオブジェクトについて、有限の上限は想定されません。

完全にラップされたオブジェクトは、不透明度に対して`unknown`の寄与をします。部分的な集計が生成できないという理由で、それらが不透明度ヒストグラムから除外されることはありません。不透明度の値と、そのカバー範囲を並べて表示することで、小さな観測された不透明領域のバンドが、大きな未知の集団を隠すことがないようにします。

`unsupported-dialect` は依然として明確な制限事項であり、なぜなら、この機能は方言を特定し、`unavailable` を報告できますが、`unsupported` の状態を示すものではないからです。これは、定義が利用可能であり、記録された方言が指定されたアナライザーによってサポートされていない場合にのみ派生されます。その他の定義に関する制限も、同様に、定義の可視性、センサスの状態、および関連する証拠から派生し、独立した主張として維持されるものではありません。

比較の対象となるファイルは、複雑さの契約、アッセッサーのバージョン、アナライザーのバージョン、正確な分析範囲、方言と文法プロファイル、スコープ、および母集団ポリシーに基づいて計算されます。粗い homogeneous/mixed オプションはシリアライズされません。なぜなら、異なる組み合わせのセットは必ずしも比較可能ではないからです。

複雑さは常にソース側に依存します。バンドルは、各子Blueprintの評価を保持し、エンジン、アナライザーのバージョン、方言、または文法プロファイル間で、バンドルレベルの複雑さの範囲やヒストグラムを作成することはありません。

オブジェクトごとのIDは、`<kind>-NNN`のような形式で、例えば`view-001`、`package-002`、または`procedure-003`です。V7は、一般的なオブジェクトファミリーに加えて、Oracleのパッケージ、スケジューラオブジェクト、データベースリンク、ディレクトリ、ライブラリ、Javaオブジェクト、演算子、インデックスタイプ、ドメイン、アノテーション、およびプロパティグラフを認識します。また、エンジンに依存しない`queue`と`edition`の種類も認識します。3桁の最小値はゼロ埋めされ、各種類は`001`から始まる独自の密な順序セットを持ちます。幅は999を超えると増加します。レコードには、閉じられたkind/subkind/tierトークン、匿名schema/parentID、定義visibility/securityモード、オプションの有効性とカタログフラグ、閉じられた要件カバレッジ、オプションの外部前提条件、およびオプションの言語統計が含まれています。親は、匿名テーブルまたは別のオブジェクトである可能性があります。したがって、パッケージからプロシージャへの階層構造を名前なしで保持できます。親グラフは、サイクルを含まない必要があります。

V7では、すべてのエンジンで共通の`subkind`語彙が使用されます。

```text
ordinary, other, materialized, integer_sequence, stored_procedure,
stored_function, scalar_function, inline_table_function, table_function,
user_defined_aggregate, table_trigger, ddl_event_trigger, before_insert,
before_update, before_delete, after_insert, after_update, after_delete,
generated_column, column_default, default_constraint, check_constraint,
row_security, rewrite_rule, legacy_rule, enum, domain, composite, range,
alias_type, table_type, clr_type, clr_procedure, clr_scalar_function,
clr_table_function, clr_aggregate, clr_trigger, clr_assembly,
server_extension, loadable_udf, foreign_data_wrapper_server, foreign_table,
federated_table, external_table, external_data_source, external_file_format,
logical_replication_publication, logical_replication_subscription,
full_text_catalog, partition_scheme, partition_function, tablespace, filegroup,
database_certificate, symmetric_key, asymmetric_key, column_master_key,
column_encryption_key, database_scoped_credential, linked_server,
enabled_event, disabled_event, enabled_agent_job, disabled_agent_job,
database_synonym, specification, body, package_member, public, private,
java_source, java_class, java_resource, external_library,
user_defined_operator, domain_indextype, scheduler_job, scheduler_program,
scheduler_schedule, scheduler_chain, advanced_queuing, service_broker
```

V7では、曖昧なv1の依存関係リストを、ソートされた型付きの`relationships`に置き換えます。関係の種類は、呼び出し、読み込み、書き込み、table/object参照、トリガーの所有権、実装、物理的な配置、セキュリティ、拡張機能の使用、外部binaries/services、およびリモートdatabase/serverの使用を区別します。すべての関係は、クローズド証拠トークン(`catalog-confirmed`、`dependency-confirmed`、`syntax-confirmed`、`lexical-hint`、または`unresolved`)を記録します。`dependency_edge_count`は、出力されるグラフと完全に一致する必要があります。

`requirements` 閉じたエンジンに特有のトークンを使用し、上限付きのカウント範囲を設定します。これらは、Oracleでラップされたソース、複合トリガー、パッケージの状態、動的SQL、自律トランザクション、pipelined/parallel/aggregate ルーチン、外部ライブラリ、データベースリンク、ドメインインデックス、スケジューラ、object/collection/spatial/vector 型、Javaオブジェクト、またはプロパティグラフなど、互換性に関する要件を特定するために使用されます。これらは、計画の根拠となる情報です。

要件は、範囲が限定されたカタログの事実、または専用のエンジンに対応した構文チェックから得られます。汎用的な字句解析は、エンジンに特化した要件を生成することはありません。すべてのスキーマ v7 のグラフ、または分析された成果物には、`requirement_status = complete | partial | unavailable | not_applicable` が含まれています。`complete` は、その成果物に対してリストが網羅的であることを示します。`not_applicable` は、要件モデルが適用されないため、要件レコードおよび外部前提条件レコードが禁止されていることを示します。`partial` と `unavailable` は、そのオブジェクトの環境および方言に関する情報を不明にしています。`partial` は、少なくとも1つの事実ソースが網羅的な範囲をカバーせずに成功したことを意味します。`unavailable` は、どの要件ソースも有効な範囲を確立できなかったため、既知の要件または外部前提条件の証拠を含めることができないことを意味します。そのような証拠には `partial` が必要です。これにより、アクセスできないオブジェクトがローカルで劣化し、残りのシステム全体にとって有用な範囲が失われるのを防ぐことができます。

インベントリレベル`requirements_complete`は、集計された状態を表します。これは、生成されたすべての成果物が`complete`または`not_applicable`であり、選択された範囲における評価対象がすべて揃っている場合にのみ真となります。ただし、すべての成果物が`complete`であっても、この状態が偽のまれる場合もあります。空の`requirements`配列を、その成果物の状態が`complete`でない限り、常にゼロ結合と解釈しないでください。

`unresolved_relationships` は、範囲が限定された、理由と件数を対応付けたマップです。これは、リモート参照やデータベース間の参照、選択されたスキーマの境界、権限が隠されたターゲット、暗号化または隠蔽された定義、動的SQL、あいまいなバインディング、欠落または不完全なネイティブID、モデル化されていないターゲットファミリー、および不明なケースを区別します。完全な依存関係の証拠を得るには、このマップが空である必要があります。ソースオブジェクト名、SQLテキスト、プリンシパル、エンドポイント、認証情報、キー、証明書、およびバイナリは、この契約のフィールドではありません。

外部前提条件は閉じた `class`、デプロイスコープ、未収集のバイナリ/秘密/
エンドポイント素材が必要かどうか、および有界な互換性カテゴリを記録します。その件数は
移行計画の証拠であり、DBWarp が自動プロビジョニングまたは翻訳できるという主張ではありません。

V7言語の統計データは、`analyzer_version = "lexical-v2"` と `analysis_span` を使用して記録されます。アナライザーは、外側の作成処理、識別子、シグネチャ、戻り値の宣言、およびモジュールのオプションを除き、実行可能コードまたは宣言的な部分のみを受け取ります。ヘッダーの情報は、カタログの要件またはフラグとして残ります。本体を安全に分離できないコレクタは、`analysis_span = "unknown"` を記録し、代わりに利用できない証拠を記録します。検証済みの適用不可能な定義は、`analysis_span = "not-applicable"` を使用します。省略された範囲の証拠は、保守的に `unknown` として解釈されます。これは、エンジンまたはオブジェクトの種類から推測されることはありません。このレキシャル実装によって分析されたサポートされている定義は、`status = "partial"` を使用します。存在しない、またはサポートされていない定義の証拠は、`unavailable` を使用し、検証済みの適用不可能なオブジェクトは、`not_applicable` を使用する場合があります。カウント、サイズ、ネスト、複雑さ、および不透明領域の値は、正確なソースのフィンガープリントではなく、範囲です。機能は、閉じた語彙から選択されます。アナライザーは、コメント、リテラル、および引用された識別子を削除します。これは、パーサー、セマンティックバインダー、または翻訳の成功を保証するものではありません。

Wrapped PL/SQL は、いかなる状況においても、実行可能なコードの証拠となることはありません。コレクタは、それを暗号化されたものとしてマークし、そのバイトを分析から除外します。また、共有アナライザも、ヘッダに "wrapped" マーカーが含まれている PL/SQL ユニットを拒否し、分類の誤りが、もっともらしいが誤った人口統計データを生成することを防ぎます。

運用ガイダンスとエンジン範囲は[非テーブル成果物インベントリ](ARTIFACT_INVENTORY.md)を
参照してください。

## ベクター別のステガノグラフィー対策

| ベクター | 防御方法 |
|---|---|
| 識別子の順序。 | ドメイン区切りされたHMAC-SHA256方式で、プロセス内でローカルに管理される秘密鍵を使用することで、オフラインでの候補名チェックを防ぐことができます。安定したクロスランのラベルが必要な場合にのみ、同じ鍵を再利用してください。 |
| 数値の low-bit | 統計値は既定で文書化された精度に丸められます。exact-length mode は明示的で同意を必要とし、監査ログに記録され、より機密性の高いメタデータとして扱う必要があります。 |
| 秒未満のタイムスタンプ | 最上部に 1 つの UTC タイムスタンプ。秒精度のみ |
| TOML formatting | 標準的な出力は、キーの順序とインデントが一貫しており、変化しません。標準ヘッダーと生成元のコメントのみを含み、入力から派生したコメントは含まれません。 |
| サンプリングにおけるランダム性。 | サンプリングでは、固定されたシード値が使用されます (PGの決定論的`TABLESAMPLE SYSTEM`)。また、識別子の匿名化は、明示的に指定しない限り、オペレーティングシステムのCSPRNGから秘密鍵を取得します。 |
| 未使用フィールド | すべてのフィールドを上記で文書化。無制限のデータを保持する "metadata"/"comment"/"reserved" フィールドなし |
| 成果物ソーステキストと外部素材 | 定義は一時的で、有界解析後にゼロ化されます。名前、SQL テキスト、エンドポイント、プロバイダー文字列、資格情報、鍵、証明書、パッケージ名、バイナリにはシリアライズされるフィールドがありません |

## スキーマバージョンの互換性

現在のバージョンでは、スキーマのバージョンは7です。バージョン1から6までは、互換性のために引き続きサポートされています。 v1/v2ファイルには、配布ブロックがありません。v3ファイルには、配布に関するメタデータがありますが、アーティファクトのインベントリはありません。v4ファイルには、アーティファクトのインベントリが含まれている可能性がありますが、現在のBlueprint契約の識別子よりも古いものです。リーダーは、入力時に以前のv4の識別子を正規化し、そのドキュメントを標準的なBlueprint識別子で再出力します。v5ファイルは、v6で追加されたトポロジーとデータセットの範囲に関する情報よりも古いものです。v6では、トポロジー契約v1、アーティファクト契約v1、結合されたテーブルkind/partitionフィールド、符号なしの10進スケール、およびオプションの統計の鮮度を使用します。v7では、トポロジー契約v2とアーティファクト契約v2を使用し、明示的なstructure/environment/statisticsに関する情報が必要です。また、互いに独立したテーブルのセマンティクスを分離し、符号付きの10進スケールとOracleの数値モデルをサポートし、明示的なアーティファクトインベントリの状態が必要です。さらに、オブジェクトごとのスコアやクロス集計フィールドを追加せずに、固定された`dbwarp-blueprint-artifact-complexity/v1`集計契約を予約しています。リーダーは、不明な将来のスキーマバージョンを、フィールドをサイレントに破棄するのではなく、明確なアップグレードメッセージとともに拒否します。リーダーは、無効な情報を解析中に書き換えるのではなく、スタンドアロンと埋め込みのBlueprintの両方に対して、同じ厳格なv7の検証を適用します。

## JSON ではなく TOML を使用する理由

- TOML は構造セクションと leaf data をより読みやすく分離します
  （`[tables.table-001.cols.col-2]` と nested JSON の比較）。
- 差分を取りやすくなります（1 行につき 1 キー。identifier-based sub-table が
  連続した状態を維持）。
- 組織のデータ分類ポリシーに基づいて確認してから共有してください。
JSON は SQL fallback パスで**中間形式**として使用されます。各
`sql/blueprint.*.sql` スクリプトが JSON を生成し、`blueprint_format.py` が TOML へ
正規化します。中間 JSON には実際のソース識別子が含まれ、MySQL では `COLUMN_TYPE` を
通じて enum/set 宣言も含まれる場合があるため、ソース環境内で保護する必要があります。
正規化ツールは既定で新しい秘密キーを使用し、承認済みの実行間比較には同じ保護済み
`--anonymization-key-file` 契約を受け付けます。DBWarp と共有するためにレビューする
最終状態のファイルは常に TOML です。

## 構造化ファイルの来歴拡張

`engine` または `source_kind` が `"parquet"` または `"avro"` の場合、スキーマのバージョンが3以降では、以下の範囲指定されたフィールドが出力されることがあります。読み込み側は、ソースファイルのストレージと範囲指定されたデコードされたサンプル測定値との区別を維持する必要があります。ドキュメントのスキーマバージョンをサポートしていない読み込み側は、不明なフィールドを破棄するのではなく、アップグレードメッセージを表示して拒否する必要があります。

V7の構造化ファイル形式のBlueprintでは、`[structure_scope]`と`[statistics_evidence]`のブロックが完全に必要であり、`[database_topology]`、`[source_environment]`、および`[activity_snapshot]`は省略され、明示的な「適用不可」の`[artifact_inventory]`が出力されます。これらは、収集元のホストからデータベースの構成やサーバーの処理能力を推測することはありません。

構造化ファイルのBlueprintは、データベースのBlueprintと同じ匿名識別子を使用します。
秘密キーに基づく順序で `table-NNN`、スキーマの ordinal 順で `col-N` です。
ファイルの stem、Parquet path、Avro field name、manifest の `logical_table` は、
table または column identifier として出力されません。

テーブルのスコープにおいて、`table_bytes` は論理的な転送サイズの見積もりであり、一方、`storage_bytes` はディスク上の実際のソースオブジェクトのサイズです。メタデータのみのParquetでは、`table_bytes` には圧縮されていないカラムチャンクのバイト数が使用されます。オプションのデコードされたサンプリングでは、この見積もりを、予測される `blueprint-compression-probe-v2` バイトで置き換えることができます。Avroは、デコードされたフルスキャンからこれを導出します。オプションの `source_partitions`、`row_group_count`、および `source_codec` フィールドは、ファイルレイアウトを記述します。複数のファイルから構成されるデータセットでは、これらの値が集計されます。`row_group_count` はParquetに固有のものであり、`source_partitions` は単一の入力オブジェクトに対する `1` です。

列単位の `null_fraction` は `0.0` から `1.0` の観測値です。
`length_sample_rows` と `length_sample_method` は `len_avg` と `len_p95` の取得方法を
示します。`source_semantics` は `"repeated-leaf"`、`"nested-json"`、
`"multi-type-union"` などの有界な互換性情報を記録します。このフィールドには、
フィールド名や値は含まれていません。十進数の精度とスケール、タイムスタンプ
精度と UTC/ローカル意味論、UUID、固定長バイナリ情報は既存のスカラーフィールドと
`native_type` に保持されます。

圧縮の範囲において、テーブルレベルの`ratio_storage`は、`table_bytes`と実際のソースオブジェクトのバイト数を比較します。Parquetの列レベルの値は、フッターの圧縮前と圧縮後の列チャンクのバイト数を比較します。どちらもファイルストレージの計画に関する情報であり、デコードされたサンプルの推定値ではありません。`ratio_zstd_3`と`ratio_zstd_19`は、`sample_encoding`が認識された`"blueprint-compression-probe-v2"`の値の場合にのみ比較可能です。Parquetのフッター比率またはAvroコンテナの比率は、zstdフィールドに決してコピーしないでください。
