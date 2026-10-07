# 圧縮測定

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../COMPRESSION_MEASUREMENT.md)を参照してください。

**言語:** [English](../COMPRESSION_MEASUREMENT.md) | [Deutsch](../de/COMPRESSION_MEASUREMENT.md) | [Français](../fr/COMPRESSION_MEASUREMENT.md) | [Español](../es/COMPRESSION_MEASUREMENT.md) | [Polski](../pl/COMPRESSION_MEASUREMENT.md) | **日本語** | [简体中文](../zh/COMPRESSION_MEASUREMENT.md)

`dbwarp-blueprint` は、代表的なテーブルデータがどの程度圧縮できるかを任意で測定できます。WAN 転送時間とエグレスコストは、生のテーブルサイズではなく圧縮後のバイト数に依存するため、これにより DBWarp の見積もりがより正確になります。

圧縮測定は opt-in であり、明示的な同意が必要です。対話的なライブ実行では事前確認を承認できます。無人実行と構造化ファイルでは次を使用します:

```bash
--measure-compression --yes
```

圧縮測定を無効にした場合、ライブデータベースの取得ではユーザーテーブルの行の値をサンプリングしません。
構造化ファイルの動作は異なり、Avro では行数、長さ、NULL メタデータを収集するため、
引き続きレコードを走査する必要があります。[構造化ファイル](STRUCTURED_FILES.md)を参照してください。

## サンプリングされる内容

空であると安全に証明されていない対象ユーザーテーブルごとに、ツールは制限された
行数をメモリへ読み取り、安定した一時 probe バッファへエンコードし、それらを
ローカルで zstd level 3 により圧縮します。次に、集約された圧縮、NULL 密度、
カーディナリティ／頻度、長さ、スタイルの測定値を導出し、サンプリング値と一時的な
fingerprint を破棄します。

選択されたtext/binary列について、Tier 2は、その列のみをサンプリングすることもあります。これにより、テーブル全体の平均値だけでなく、列ごとの圧縮率を把握できます。

ライブデータベースのテーブルの比率は、中立的な、境界が設定された1,000行のグループを使用し、各列に1つの記述子、固定幅の値の長さ、および列に連続したデータ構造を持ちます。これにより、圧縮に関連する構造を測定しますが、データベースや転送プロトコルに関する情報は一切含まれません。各列の比率は`blueprint-compression-probe-v2`を保持し、タグ付けされた長さがプレフィックスされた値は、より具体的なエントロピー入力となります。

PostgreSQLのテーブルブロックは`blueprint-columnar-transfer-probe-v2`を使用し、行グループを1つの永続的なzstdレベル3コンテキストを通して処理し、各グループの後にフラッシュを行います。MySQLとSQL Serverは`blueprint-columnar-transfer-probe-v3`を使用します。これは、同じ中立的なバイトと永続的なコンテキストを使用しますが、256 KiBのプローブチャンク境界で追加のフラッシュを行います。SQL Serverの`nvarchar`、`nchar`、および`ntext`のサンプルは、UTF-16LEのバイト分布として測定されます。SQL Serverの`varchar`、`char`、および`text`は、サンプリングされた狭いバイト幅を保持します。Blueprintは、ソースの照合のカタログコードページを`utf-8`、`windows-N`、または`code-page-N`として記録します。データベースドライバは、デコードされた文字列をサンプラーに公開するため、レガシーコードページのバイト同一性は主張されません。テーブル`ratio_stddev`は、外部の行グループ出力全体で測定されます。列ごとの投影ブロックは、独立したワンショットのエントロピー測定として残り、`0.0`を出力します。以前のバージョンのBlueprintには`blueprint-columnar-transfer-probe-v1`が含まれている場合があります。異なるタグを持つ比率は比較できません。

サンプリングされたバイトは、選択したデータベースセッションだけを通ってローカル
プロセスへ移動します。ディスクへ書き込まれず、`blueprint.toml` や監査ログに含まれず、
アップロードされず、DBWarp インフラストラクチャへ送信されません。

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
ワーカー数に制限されます。ワーカー数は測定値を変えません。匿名ラベルの順序は、
既定の新しいキーによって意図的に変化します。承認済みの実行間比較に限り、保護された
`--anonymization-key-file` を再利用してください。

コレクターが行およびスタイルのクエリを省略するのは、エンジン管理のカタログ
値がカタログ読み取り時点でテーブルが空であると安全に証明する場合だけです。
PostgreSQL では後続変更のない新しい分析済み統計を要求し、SQL Server では
パーティション行カウンターを使用します。MySQL の推定行数は非空テーブルでも
ゼロを返すことがあるため、省略判定には使用しません。この保守的な違いにより
忠実度を保護します。

## Blueprintファイルに記録される内容

出力されるのは集約された要約だけです。text-like 列について、Tier 2 パスは `json`、`xml`、`natural-text`、`base64`、`hex`、`numeric-text`、`mixed` などの制限された style ラベルを出力する場合があります。

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
sample_method = "LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "blueprint-compression-probe-v2"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "blueprint-columnar-transfer-probe-v3"
```

これらの値は、ネットワーク転送サイズを推定するために使用されます。

## 重要である理由

生のテーブルサイズが同じ 2 つのデータベースでも、移行中の動作は大きく異なる場合があります:

- JSON、XML、繰り返される業務コード、疎なテキスト、自然言語テキストは、多くの場合よく圧縮されます。
- 暗号化された値、圧縮済み blob、ランダムトークン、高エントロピーの binary は圧縮が効きません。
- SQL Server の Unicode text と narrow text は異なる byte 分布を持ちます。sampler は `nvarchar` を UTF-16LE としてモデル化し、全ての text 列を UTF-8 とみなすのではなく、`varchar` の解釈に必要な collation code page を記録します。

通常、列型から推測するより、小規模なローカル測定の方が有用です。

## バイアスと透明性

一部のエンジンは、完全に均一な table sampling を提供しません。MySQL はそのアクセスパスが
利用可能な場合、制限されたサンプルを 4 つの numeric primary-key range に分散し、それ以外は
`LIMIT N` にフォールバックします。どちらも統計的な無作為サンプルではないため、明示的にバイアスありと記録されます。
最後以外の range window には排他的な上限があります。primary key が疎または偏っている領域では window が不足する場合が
ありますが、次の window の行を再度読み取ることはありません。その他の理想的でない engine fallback も
`sampled_with_bias` と `bias_reason` で記録されます。

Blueprintは、限られた範囲のサンプルデータと、それに関連するテキストフィールドを別々に記録します。MySQLの数値主キー範囲サンプリングでは、`sample_layout = "primary-key-range-windows"`が出力され、各ウィンドウは完全な主キーでソートされます。

偏ったサンプルは依然として有用ですが、信頼性は低くなります。監査ログには、行サンプリングが有効になっていることと、ローカルでエンコードされたプローブバイト数が記録されています。データベースセッションのバイト合計は、ドライバーがそれらを提供しない場合、`unknown`として報告されます。

## 実用的なサンプリング設定

本番環境で安全な最初のパス:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

読み取りレプリカまたはメンテナンスウィンドウが利用可能な場合、より正確な測定が可能です。

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

大規模なデータベースでも、巨大なサンプルは必要ありません。目標は、正確な行レベルプロファイリングではなく、安定した圧縮信号です。`--max-wall-secs` は接続、カタログ、RTT、サンプリングを含むライブ収集全体の厳格な期限であり、フェーズごとに更新されません。

ライブデータベースのサンプリングには、テーブルごとに変更不可能な 16 MiB の投影ペイロード上限もあります。
初期 SQL 投影は型ごとに予算が割り当てられ、元のオクテット長を別途観測します。
投影された値が短縮された場合、MySQL と SQL Server は行数を減らし、
予算内に収まるよう修正した列ごとの上限で再試行できます。
予算に収まらないほど幅広い値は、引き続き上限付きの接頭辞になります。
圧縮と値の要約の来歴情報にはこの制約が記録されますが、長さの統計は、
選択された長さ精度ポリシーのもとで、サーバーが報告した元のサンプリング値の長さを保持します。

この上限は、ネットワークのバイト数やプロセスメモリの上限ではありません。
プロトコルのエンコード、元の長さのメタデータ、再試行、ドライバーバッファがオーバーヘッドを追加します。
監査には設定されたペイロード上限、実行したクエリ、ローカルでエンコードした正確なプローブバイト合計が記録されます。
データベースの実測通信量は報告しません。

## 測定値の解釈方法

`sample_encoding` フィールドは、契約の一部です。比率は、同じエンコーディングタグ内でのみ比較可能です。なぜなら、異なるサンプルエンコーディングは、同じ論理データに対して異なる圧縮率を生み出す可能性があるからです。特に、テーブルレベルの列指向転送プローブ比率と、列ごとのv2比率は、補完的な測定値であり、互いに置き換えることはできません。
