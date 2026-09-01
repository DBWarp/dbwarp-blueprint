# dbwarp-blueprint のダウンロード

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../../binaries/README.md)を参照してください。

**言語:** [English](../../binaries/README.md) | [Deutsch](../de/BINARIES.md) | [Français](../fr/BINARIES.md) | [Español](../es/BINARIES.md) | [Polski](../pl/BINARIES.md) | **日本語** | [简体中文](../zh/BINARIES.md)

ビルド済みの `dbwarp-blueprint` バイナリは GitHub Releases ページで公開されています:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

バイナリをダウンロードし、チェックサムを検証してローカルで実行し、DBWarp と何かを共有する前に生成された `blueprint.toml` を確認できます。

たとえば `https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0` のような正確なリリースタグを選び、同じタグからアーカイブと `SHA256SUMS.txt` をダウンロードしてください。再現可能または監査対象の実行では、可変の `releases/latest` URL を使用しないでください。

## ファイル

| プラットフォーム | ファイル |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |
| オフラインソース監査バンドル | `dbwarp-blueprint-source-vendored.tar.gz` |
| チェックサム | `SHA256SUMS.txt` |

各リリースには `SHA256SUMS.txt` も含まれます。

## ダウンロードの検証

Linux/macOS:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

表示されたハッシュを `SHA256SUMS.txt` の対応する行と比較してください。

## ダウンロード済みバイナリかローカルビルドか

ダウンロード可能なバイナリは利便性のために提供されています。最も強い信頼経路は、依然としてソースからビルドすることです:

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
git checkout <release-tag>
./build.sh
```

通常のソースクローンは意図的に小さく保たれ、依存関係のバージョンを固定するために `Cargo.lock` を使用します。

ポリシーでビルド前にすべての依存関係ソースファイルをレビューする必要がある場合は、同じリリースから `dbwarp-blueprint-source-vendored.tar.gz` をダウンロードし、展開したツリー内でビルドしてください:

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

[`BUILD.md`](BUILD.md)を参照してください。

## ツールの処理

`dbwarp-blueprint` はデータベースメタデータを読み取り、任意で小規模なローカルサンプルの圧縮率を測定します。DBWarp の移行見積もり用に匿名化されたテキストファイルを書き込みます。ファイルのアップロードもテレメトリの送信も行いません。
