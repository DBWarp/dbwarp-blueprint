# ソースからの dbwarp-blueprint のビルド

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../../BUILD.md)を参照してください。

**言語:** [English](../../BUILD.md) | [Deutsch](../de/BUILD.md) | [Français](../fr/BUILD.md) | [Español](../es/BUILD.md) | [Polski](../pl/BUILD.md) | **日本語** | [简体中文](../zh/BUILD.md)

このガイドは、データベースに対してツールを実行する前に、自身でビルドすることを希望する顧客向けです。

## クイックビルド

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
./build.sh
```

バイナリは次の場所に書き込まれます:

```text
target/release/dbwarp-blueprint
```

他の例では `./dbwarp-blueprint` を使用します。ソースビルド後は
`target/release/dbwarp-blueprint` を直接実行するか、そのファイルを
`./dbwarp-blueprint` にコピーしてから例を実行してください。

固定された Rust toolchain がまだなく、レビュー済みのネットワークアクセスが
許可される場合は、明示的に opt-in します:

```bash
ALLOW_NETWORK=1 ./build.sh
```

## ビルドスクリプトの処理

`build.sh` は意図的に保守的な設計です:

- 固定された Rust バージョンを `rust-toolchain.toml` から読み取る
- 既存の `rustc` が固定バージョンと一致する場合はそれを使用する
- `ALLOW_NETWORK=1` が設定されていない限り Rust のダウンロードを拒否する
- rustup ブートストラップのバージョンを固定し、使用前に公式 SHA-256 を検証する
- ツールチェーンの状態を `./build/` 配下に保持する
- 再現可能な依存関係バージョンのために Cargo.lock を使用する
- 既定では `cargo build --release --locked` でビルドする
- vendored ソースバンドルから実行された場合は、自動的に `--frozen --offline --locked` へ切り替える
- `vendor-crates/` が存在しない場合、`DBWARP_BLUEPRINT_OFFLINE=1` を拒否する
- 生成されたバイナリの SHA256 を表示する
- 監査に正確なソースリビジョンと worktree の変更状態を埋め込む

`sudo` は使用せず、システムの Rust インストールを変更しません。

## ダウンロード可能なバイナリ

再現可能な実行では、正確なリリースタグを固定して SHA-256 を検証し、可変のダウンロード URL を使用しないでください。

ビルド済みバイナリは Releases ページで入手できます:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

これらは利便性のために提供されています。ポリシーでソースレビューが必要な場合は、同じタグからローカルでビルドしてください。

プラットフォームバイナリアーカイブは運用者向けバンドルであり、ソースツリーでは
ないため、その場で再ビルドすることはできません。含まれるこのガイドと
`verify.sh` は、対応するソースを使った検証経路を説明する参照資料です。
`build.sh`、Cargo ソース、またはローカル比較ビルドが必要な場合は、正確な
リリースタグのチェックアウトか、そのリリースの依存関係同梱ソースアーカイブを
使用してください。

リリースファイル:

| プラットフォーム | ファイル |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## ダウンロードしたアーカイブの検証

Linux:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS:

```bash
shasum -a 256 dbwarp-blueprint-macos-arm64.tar.gz
```

表示された値を `SHA256SUMS.txt` の該当行と比較します。

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

## 認証方式固有のビルド

既定のビルドは password、token-file、token-env、TLS の各フローをサポートします。クライアント証明書 mTLS は PostgreSQL と MySQL で使用できます。

SQL Server 統合認証には、プラットフォーム固有のビルドがあります:

| プラットフォーム | ビルドコマンド | 用途 |
|---|---|---|
| Linux | `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Kerberos / GSSAPI |
| Windows | GitHub release Windows binary, or `cargo build --release --features winauth` | Windows Integrated Auth / SSPI |

Linux Kerberos には通常の MIT Kerberos ランタイムライブラリが必要です。ホスト上で `kinit` が動作する場合、必要なランタイム部品は通常すでに存在します。

## スクリプトを使用しないビルド

ポリシー上、Cargo コマンドを直接使用する場合:

```bash
cargo build --release --locked
```

Windows SSPI ビルド:

```powershell
cargo build --release --locked --features winauth
```

Linux Kerberos ビルド:

```bash
cargo build --release --locked --features integrated-auth-gssapi
```

## リリースバイナリの再現

`./build.sh` はレビュー済みソースがビルドできることを示します。バイト単位の
同一性には、リリースの完全なネイティブビルド入力も必要です。
`PROVENANCE.json` に記録された正確なソースリビジョン、ターゲット、features、
固定 Rust toolchain、ネイティブコンパイラー/リンカー、コミット時刻を使った
`SOURCE_DATE_EPOCH`、リリースワークフローのパス再マッピングとリンカーフラグを
使用してください。Windows リリースでは `clang-cl` と `/Brepro` も使います。

入力を再現した後、展開したリリースバイナリをローカル結果と比較します:

```bash
SOURCE_BIN=target/release/dbwarp-blueprint \
  ./verify.sh /path/to/extracted/dbwarp-blueprint
```

ハッシュが異なる場合は、バイナリを同等として扱わないでください。リリース CI
は同じ runner 上の別々の Cargo target ディレクトリで 2 回ビルドし、差異を
拒否します。`PROVENANCE.json` はローカル再現の評価に必要な情報を記録します。

## Vendored 依存関係

通常のリポジトリには、MySQL の `--tls-ca` がツールの他の部分と同じ制限的な
信頼セマンティクスを持つようにするため、`vendor/mysql_async` 配下に小さな
パッチ済み依存関係が 1 つ含まれています。その他すべての依存関係バージョンは
`Cargo.lock` によって固定されています。

各 GitHub Release では、すべての依存関係ソースファイルをオフラインで検査してビルドしたいセキュリティチーム向けに、別個の `dbwarp-blueprint-source-vendored.tar.gz` バンドルを公開します。

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

このバンドルには、パッチ済みの `vendor/mysql_async`、その他すべての依存関係用に生成された
`vendor-crates/` ツリー、および crates.io をローカル vendor ツリーへリダイレクトする
生成済みの `.cargo/config.toml` が含まれます。このモードでは、`build.sh` は
`cargo build --release --frozen --offline --locked` を使用します。
