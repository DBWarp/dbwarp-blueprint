# 圧縮測定

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../COMPRESSION_MEASUREMENT.md)を参照してください。

**言語:** [English](../COMPRESSION_MEASUREMENT.md) | [Deutsch](../de/COMPRESSION_MEASUREMENT.md) | [Français](../fr/COMPRESSION_MEASUREMENT.md) | [Español](../es/COMPRESSION_MEASUREMENT.md) | [Polski](../pl/COMPRESSION_MEASUREMENT.md) | **日本語** | [简体中文](../zh/COMPRESSION_MEASUREMENT.md)

`dbwarp-blueprint` は、代表的なテーブルデータがどの程度圧縮できるかを任意で測定できます。WAN 転送時間とエグレスコストは、生のテーブルサイズではなく圧縮後のバイト数に依存するため、これにより DBWarp の見積もりがより正確になります。

圧縮測定は opt-in であり、明示的な同意が必要です。対話的なライブ実行では事前確認を承認できます。無人実行と構造化ファイルでは次を使用します:

```bash
--measure-compression --yes
```

これらのフラグを指定しない場合、ツールはカタログメタデータのみを読み取ります。

## サンプリングされる内容

ツールはユーザーテーブルごとに、制限された行数をメモリへ読み取り、決定論的な row-frame バッファへエンコードし、そのバッファをローカルで zstd level 3 により圧縮して、丸められた比率を記録した後、バッファを破棄します。

選択された text/binary 列について、Tier 2 はその列だけをサンプリングする場合もあります。これにより、後続の計画ツールはテーブルレベルの平均だけに依存せず、列ごとのエントロピーに一致できます。

各測定は、入力サイズを事前申告した独立したワンショット zstd フレームです。比率の分散（`ratio_stddev`）は、同じバッファを行境界に揃えた 64 KiB チャンクで測定されるため、分散は 1 つのバッファ全体の平均ではなく、推定器が予測する転送を表します。入力サイズを事前申告するため、zstd は推定器の転送モデルと一貫したサイズ適応パラメータを選択します。小さなサンプル（およそ 1 MiB 未満）では、サイズ申告なしのストリーミングコンテキストで測定していた旧リリースのキャプチャと比べて比率が目に見えて変わることがあり、その境界をまたいだ小テーブルの比率は直接比較できません。転送に一致するのは、サイズを申告した測定の方です。

サンプリングされたバイトはディスクへ書き込まれず、`blueprint.toml` に含まれず、監査ログにも含まれません。また、データベースサーバーから、顧客が実行したローカルプロセスへの転送以外には、どこにも送信されません。

## ローカルワーカーの並行処理

データベースサンプリングでは、常に単一の逐次接続を使用します。任意の
`--compression-workers N` 設定は、読み取り済みのメモリ内サンプルに対する
ローカル圧縮だけを並列化します。1～32 ワーカーを指定でき、ソースホストへの影響を
最小限にするため既定値は 1 です。ローカル CPU をさらに使用する場合は明示的に増やしてください。

```bash
--measure-compression --yes \
--compression-workers 4
```

zstd がボトルネックの場合、値を増やすと経過時間を短縮できますが、ローカル
CPU とピークメモリは増加します。データベースの同時サンプリング接続は作成
されません。各ワーカーは独自の zstd コンテキストを所有し、入力キューは
ワーカー数に制限されます。出力順序と v6 Blueprint 値は決定的なままです。

コレクターが行およびスタイルのクエリを省略するのは、エンジン管理のカタログ
値がカタログ読み取り時点でテーブルが空であると安全に証明する場合だけです。
PostgreSQL では後続変更のない新しい分析済み統計を要求し、SQL Server では
パーティション行カウンターを使用します。MySQL の推定行数は非空テーブルでも
ゼロを返すことがあるため、省略判定には使用しません。この保守的な違いにより
忠実度を保護します。

## Blueprintファイルに記録される内容

出力されるのは要約数値だけです。text-like 列について、Tier 2 パスは `json`、`xml`、`natural-text`、`base64`、`hex`、`numeric-text`、`mixed` などの制限された style ラベルを出力する場合があります。

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
sample_method = "column LIMIT N (engine-specific bounded sample)"
sampled_with_bias = true
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "dbwarp-blueprint-rowframe-v1"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

これらの値は、承認された後続ツールがネットワーク転送サイズを見積もり、同様の圧縮特性を持つ合成 text/binary データを生成するために役立ちます。

## 重要である理由

生のテーブルサイズが同じ 2 つのデータベースでも、移行中の動作は大きく異なる場合があります:

- JSON、XML、繰り返される業務コード、疎なテキスト、自然言語テキストは、多くの場合よく圧縮されます。
- 暗号化された値、圧縮済み blob、ランダムトークン、高エントロピーの binary は圧縮効果が低い傾向があります。
- SQL Server の `nvarchar` データは UTF-8 テキストと異なるバイト分布を持ち、サンプリング時もそれに応じてエンコードされます。

通常、列型から推測するより、小規模なローカル測定の方が有用です。

## バイアスと透明性

一部のエンジンは、完全に均一なテーブルサンプリングを提供しません。ツールが理想的でない方法へフォールバックした場合、Blueprintファイルでは `sampled_with_bias` と `bias_reason` によってそのことを示します。

バイアスのあるサンプルも有用ですが、下流ツールは信頼度を下げて扱う必要があります。監査には行サンプリングが有効だったことと、ローカルでエンコードした row-frame バイト数が記録されます。ドライバーが公開しないネットワークバイト数は `unknown` です。

## 実用的なサンプリング設定

本番環境で安全な最初のパス:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

読み取りレプリカまたはメンテナンス時間帯を使用できる場合の、より優れた estimator 入力:

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

大規模なデータベースでも、巨大なサンプルは必要ありません。目標は、正確な行レベルプロファイリングではなく、安定した圧縮信号です。`--max-wall-secs` は接続、カタログ、RTT、サンプリングを含むライブ収集全体の厳格な期限であり、フェーズごとに更新されません。

ライブデータベースのサンプリングには、テーブルごとに変更不可能な 16 MiB の投影ペイロード上限もあります。
SQL 投影はドライバーがデータを受信する前にサーバー上で可変幅セルを切り詰め、非常に幅の広いテーブルでは行数上限を減らします。
そのため、非常に大きな LOB 値は全体ではなく、上限付きの接頭辞だけが対象になります。
監査には、有効なテーブルペイロード上限と、ローカルにエンコードされた正確な row-frame バイト合計が記録されます。

## 後続 consumer による使用方法

後続の consumer は、次の順序で圧縮エビデンスを使用する必要があります:

1. 認識可能な列単位の圧縮ブロック。
2. 認識可能なテーブル単位の圧縮ブロック。
3. 測定済み比率が存在しない場合の type/style 既定値。

`sample_encoding` フィールドは契約の一部です。同じ論理データでもサンプルエンコーディングが異なると圧縮率が変わり得るため、consumer は認識可能な encoding tag を持つ比率だけを使用する必要があります。
