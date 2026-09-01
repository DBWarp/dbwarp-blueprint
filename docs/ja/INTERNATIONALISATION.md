# 国際化

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../INTERNATIONALISATION.md)を参照してください。

**言語:** [English](../INTERNATIONALISATION.md) | [Deutsch](../de/INTERNATIONALISATION.md) | [Français](../fr/INTERNATIONALISATION.md) | [Español](../es/INTERNATIONALISATION.md) | [Polski](../pl/INTERNATIONALISATION.md) | **日本語** | [简体中文](../zh/INTERNATIONALISATION.md)

`dbwarp-blueprint` は、人向けの表示と運用構文を分離します。これは単なる表示設定ではなく、
セキュリティと自動化の境界です。

## サポートされる言語

英語の原文が正本です。英語以外の表示カタログは機械支援で作成されており、
キーとトークンの網羅性が検証されていても誤りを含む可能性があります。
セキュリティ、契約、規制、最小権限に関する判断は英語の本文と照合してください。
翻訳文書の公開に関する別個のゲートは [`TRANSLATIONS.md`](../TRANSLATIONS.md) を
参照してください。

| 値 | 言語 | 生成されるデッキで使用する locale tag |
|---|---|---|
| `en` | 英語 | `en-US` |
| `de` | ドイツ語 | `de-DE` |
| `fr` | フランス語 | `fr-FR` |
| `es` | スペイン語 | `es-ES` |
| `pl` | ポーランド語 | `pl-PL` |
| `ja` | 日本語 | `ja-JP` |
| `zh` | 簡体字中国語 | `zh-CN` |

言語を明示的に選択します:

```bash
dbwarp-blueprint --lang de --help
dbwarp-blueprint --lang ja --connect postgresql://db.internal/app --dry-run
```

`--lang` を指定しない場合の解決順序は次のとおりです:

1. `DBWARP_BLUEPRINT_LANG`
2. `LC_ALL`
3. `LC_MESSAGES`
4. `LANG`
5. 英語

環境の locale tag では region と encoding の suffix を受け付けるため、
`de_CH.UTF-8`、`pl_PL.UTF-8`、`ja-JP` はそれぞれの base language に解決されます。
明示的な `--lang` の値は、表にある 7 つの正規トークンに意図的に限定されています。

Windows では通常 `LC_ALL`、`LC_MESSAGES`、`LANG` が設定されないため、`--lang` または `DBWARP_BLUEPRINT_LANG` がなければ英語になります。PowerShell では `$env:DBWARP_BLUEPRINT_LANG = "de"`、cmd では `set DBWARP_BLUEPRINT_LANG=de` と設定できます。Windows の環境変数名は大文字小文字を区別しませんが、Linux と macOS では区別するため、常に正規の大文字名を使用してください。

## 翻訳されるもの

- top-level および option help の説明
- usage や possible-values label などの help scaffolding
- pre-flight plan と consent prompt
- DBP メッセージの summary、cause、corrective action
- progress および warning の文章
- PowerPoint デッキの heading、label、explanation、locale metadata

オペレーティングシステム、TLS ライブラリ、またはデータベースドライバーから返される
診断に必要な致命的 technical detail は、localized DBP message の下にそのまま残る場合があります。非致命的 database warning は source identifier を含む可能性のある raw driver detail を伏せますが、安定した DBP code と匿名 Blueprint target は残ります。

## 決して変わらないもの

次の項目は、すべての表示言語で正規の英語トークンを維持します:

- `dbwarp-blueprint` のコマンド名と `--measure-compression` などのオプション名
- `verify-full`、`balanced`、`exact` などの受け付ける値
- `postgresql://`、`mysql://`、`sqlserver://` などの URI スキーム
- 環境変数名とファイルパス
- `source=ID`、`table=ID` などのセレクター
- `DBP1001E` などの DBP 識別子
- `table-001`、`col-1`、`schema-A` などの匿名化された識別子
- 監査キー、TOML キー、バンドルキー、データベース型名、インデックスメソッド

したがって、スクリプトで言語固有のオプションや値を扱う必要はありません。
ほかの決定論的な入力がすべて同じなら、`--lang ja` で生成したBlueprintは
`--lang en` で生成したBlueprintとバイト単位で同一です。

## 厳格なカタログ動作

すべてのカタログはバイナリにコンパイルされています。起動時に、プログラムは
広告する英語以外のすべての locale が次を正確に網羅することを検証します:

- 現在の live Clap help tree
- すべての安定した DBP code と、3 つすべての diagnostic field
- すべての安定した prompt、progress、warning、deck key
- すべての必須 placeholder と保護された operational token

entry の不足または過剰、placeholder の変更、operational token の変更、
無効な JSON、不可視/双方向 format control がある場合、`DBP1010E` で fail closed します。
翻訳が欠けている場合に英語を暗黙に代用することはありません。

## メンテナーのワークフロー

正規のソースは、英語の Rust help と `src/i18n.rs` にある message/UI definition です。
顧客向けの文言を変更する場合:

1. 同じコミットで `locales/` 以下のすべての locale catalog を更新する。
2. すべての placeholder と正規の operational token を正確に保持する。
3. focused exact-coverage test を実行する。
4. failure または warning を変更した場合は、`tests/cli_errors.rs` に関連する operator-boundary case を追加または更新する。
5. 完全な test suite を実行し、代表的な help/deck output を確認する。
6. 新しい文言を顧客契約、規制当局への提出、または公開マーケティング向けの最終版として扱う前に、ネイティブによる技術レビューを受ける。

重点的な検証:

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

integration test では、option token が言語間で同一であること、ローカライズされた
DBP code が安定していること、出力 TOML が言語に依存しないこと、生成された deck prose に
選択した locale が設定されることも証明します。
