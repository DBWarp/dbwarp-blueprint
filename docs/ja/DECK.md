# ビジュアルサマリーデッキ

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../../DECK.md)を参照してください。

**言語:** [English](../../DECK.md) | [Deutsch](../de/DECK.md) | [Français](../fr/DECK.md) | [Español](../es/DECK.md) | [Polski](../pl/DECK.md) | **日本語** | [简体中文](../zh/DECK.md)

`dbwarp-blueprint --deck blueprint.pptx` は、`--out` TOML ファイルと並べて、
任意の PowerPoint (`.pptx`) Blueprintサマリーを書き込みます。`dbwarp-blueprint --from-toml
blueprint.toml --deck blueprint.pptx` は、既存のレビュー済みBlueprintファイルから、
データベースへ接続せずに後から同じデッキを構築します。これは同じ匿名化データの
提示であり、データベースから追加で読み取ったり、データベースへ送信したりしません。
デッキは、Blueprint にすでに存在するフィールドから、文書化されたローカルの要約と
予測だけを計算します。

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

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx \
  --lang ja
```

`--lang en|de|fr|es|pl|ja|zh` は、デッキの人向け文章と PowerPoint の言語
メタデータをローカライズします。匿名識別子、データベース型名、インデックス方式、
測定値、ソース TOML は正規のままで、言語に依存しません。カタログ検証では、
デッキの文言が欠けている場合に英語を代用せず、fail closed します。
[`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md)を参照してください。

## フッターと機密レベル

各コンテンツスライドは DBWarp の標準フッターを使用します。左側に小型ロゴ、
任意の区切り記号と機密レベル、中央に番号だけのスライド番号、右側に
`DBWarp.com` を配置します。タイトルスライドには番号を付けません。

`--deck-confidentiality public|internal|confidential|restricted` を使用すると、
ローカライズされた組み込み分類ラベルのいずれかが追加されます。それ以外の安全で
空でない値はカスタムラベルとして入力どおりに表示されます。空白を含む値は、
`--deck-confidentiality "CLIENT // SENSITIVE"` のように引用符で囲んでください。
ラベルの先頭または末尾の空白、制御文字、双方向書式制御文字は使用できず、表示幅は
48 単位以下でなければなりません。分類ラベルを表示しない場合は、このオプションを
省略します。この設定はデッキの表示だけを変更し、Blueprint ファイルやデッキが
要約するデータは変更しません。レビュー済み Blueprint、言語、ラベル、タイムスタンプが
完全に同じであれば、デッキのバイト列を再現できます。

## 信頼性の特性

- **メモリからローカルに構築。** デッキは、`blueprint.toml` を生成するものと同じメモリ内
  Blueprintからレンダリングされます。追加のデータベースクエリも、カタログに対する
  2 回目の走査もありません。`--from-toml` モードでは、代わりにレビュー済み TOML
  ファイルからメモリ内Blueprintを読み込みます。
- **アプリケーションによるネットワーク接続なし。** デッキの生成はネットワーク接続を開きません。ただし、ネットワークマウントされたパスから Blueprint を読む場合は、ホストのストレージスタックの影響を受けます。
- **サードパーティーライブラリなし。** OOXML は `src/deck.rs` で直接作成されます。
  `.pptx` は XML パーツを収めた通常の ZIP であり、`unzip` して読めます。PowerPoint
  自動化、レンダリングサービス、依存関係グラフへの追加 crate はありません。承認済みの
  DBWarp ロゴ画像と静的 DM Sans フォントは Rust バイナリに埋め込まれ、OOXML
  メディアパーツおよびフォントパーツとして書き込まれます。生成時に実行時のアセットパスは
  読み取りません。
- **実際の識別子なし、行データなし。** テーブル、列、インデックスは、Blueprintファイルと
  同じ匿名プレースホルダー (`table-001`、`col-1`、`idx-1`、`schema-A`) として表示されます。
  ソース測定値は文書化された精度を維持し、予測は Blueprint にすでに存在するフィールド
  だけから計算されます。デッキには、その入力を超える顧客固有の情報は含まれません。
- **固定入力から再現可能。** 同じレビュー済み Blueprint は、同じ選択言語、機密ラベル、
  固定タイムスタンプに対してバイト単位で同一の `.pptx` を生成します（固定されたパーツ順序と
  タイムスタンプ）。これによって 2 回のライブ取得が同一になるわけではありません。同じ保護済み
  `--anonymization-key-file`、ソース状態、取得オプションも必要です。

## 含まれる内容

デッキはスキーマサイズに適応します:

- **タイトル** — DBWarp ロゴとタグライン、エンジン、バージョン、source kind、テーブル数、生成タイムスタンプ。
- **エグゼクティブサマリー** — 移行規模、データ集中度、関係の複雑さ、レビュー用の証拠シグナル。
- **概要** — テーブル数、行数、データサイズ、インデックスサイズの合計、および列、
  インデックス、外部キー、スキーマの数。
- **小規模スキーマ**（数テーブル）— テーブルごとのサイズ付きパネル（行、バイト、
  列型、インデックス）と外部キー図。
- **大規模スキーマ** — 列挙の代わりに特徴付け:
  - *最大テーブル*: サイズ上位のテーブルと `+ N more` の残数表示。
  - *スキーマ構成*: 列型の分布とインデックス/合計の統計。
  - *関係*: 外部キー数、接続済みテーブルと独立テーブル、および最も多く参照される
    ハブテーブル。
- **測定済み圧縮**（Tier 2 のみ）— サンプリング済みテーブル数、加重 zstd-3 比率、
  予測圧縮フットプリント、および最も圧縮しやすいサンプリング済みテーブル。
- **信頼モデル** — 上記の保証を要約する最後のスライド。

## 出力のレビュー

`.pptx` は標準 OOXML パッケージです。含まれる内容を正確に監査するには:

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

PowerPoint、LibreOffice Impress、Google Slides で開けます。ジェネレーターは
[`src/deck.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/deck.rs) であり、Rust バイナリに組み込まれています。
インストール、監査、同期維持が必要な別個のデッキジェネレーターはありません。
