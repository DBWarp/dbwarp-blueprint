# 貢献方法

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../../CONTRIBUTING.md)を参照してください。

**言語:** [English](../../CONTRIBUTING.md) | [Deutsch](../de/CONTRIBUTING.md) | [Français](../fr/CONTRIBUTING.md) | [Español](../es/CONTRIBUTING.md) | [Polski](../pl/CONTRIBUTING.md) | **日本語** | [简体中文](../zh/CONTRIBUTING.md)

まず、問題と小さな合成データによる再現例を説明する、機密情報を含まない issue を作成してください。
大きな変更では、パッチを準備する前に方針を相談してください。
提案された変更が製品とそのセキュリティ境界に適合するかどうかは、メンテナーが判断します。
issue やプルリクエストを作成しても、採用されるとは限りません。

安全な問題報告は [SUPPORT.md](SUPPORT.md)、非公開の脆弱性報告は
[SECURITY.md](SECURITY.md) に従ってください。相手を尊重し、再現可能な動作に焦点を当てて議論してください。

## 変更の準備

- [BUILD.md](BUILD.md) に記載された、固定バージョンのツールチェーンとロック済み依存関係を使用してください。
- 変更の範囲を絞り、動作を変更した箇所には回帰テストを追加してください。
- パッチやテスト用データに、顧客データ、資格情報、非公開インフラストラクチャの詳細、
  または識別情報を含む監査証跡を含めないでください。
- 明示的な同意、ソースへの負荷の上限、最小権限アクセス、
  欠落または劣化した観測結果の正直な報告を維持してください。
- Blueprint 契約または圧縮エンコーディングの変更を文書化してください。
  既存の任意フィールドと互換性の規則も、インターフェースの一部です。
- CLI オプション、診断コード、シリアライズされるフィールドは正規の英語のままにしてください。
  人向けの実行時表現を変更する場合は、出荷するすべての実行時カタログを更新する必要があります。
  英語の Markdown が正本であり、翻訳された Markdown は補足資料です。

## ローカル検査

固定バージョンのツールチェーンをインストールしたうえで、ソースリポジトリから実行します:

```bash
cargo fmt --all --check
cargo test --locked --all-targets
./tools/check_blueprint_core_sync.sh
python3 tools/check_public_tree.py
```

共有コアには独自のユニットテスト一式があります:

```bash
cargo test --locked --manifest-path crates/dbwarp-blueprint-core/Cargo.toml --lib
```

ローカルテストの成功は、データベースバージョンやプラットフォームの資格検証ではありません。
実際にテストした内容を説明し、それ以外の構成は明示的に未テストとしてください。
メンテナーの承認なしに、パッチの一環として成果物を公開したり、リリースタグを更新したり、
リポジトリのセキュリティ設定を変更したりしないでください。

## メンテナーのワークフロー

正規のソースは、英語の Rust help と `src/i18n.rs` にある message/UI definition です。
顧客向けの文言を変更する場合:

1. 同じコミットで `locales/` 以下のすべての locale catalog を更新する。
2. すべての placeholder と正規の operational token を正確に保持する。
3. focused exact-coverage test を実行する。
4. failure または warning を変更した場合は、`tests/cli_errors.rs` に関連する operator-boundary case を追加または更新する。
5. 完全な test suite を実行し、代表的な help/deck output を確認する。
6. 新しい文言を顧客契約、規制当局への提出、または公開マーケティング向けの最終版として扱う前に、ネイティブによる技術レビューを受ける。

この完全網羅性のワークフローは、バイナリに組み込まれる実行時カタログに適用されます。
翻訳された Markdown は補足資料です。
[`docs/TRANSLATIONS.md`](../TRANSLATIONS.md) を参照してください。

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
