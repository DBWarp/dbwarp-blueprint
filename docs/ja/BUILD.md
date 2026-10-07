# ソースからの dbwarp-blueprint のビルド

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。英語が正本です。契約上の正式文書として扱わないでください。[英語の正本](../../BUILD.md)を参照してください。

**言語:** [English](../../BUILD.md) | [Deutsch](../de/BUILD.md) | [Français](../fr/BUILD.md) | [Español](../es/BUILD.md) | [Polski](../pl/BUILD.md) | **日本語** | [简体中文](../zh/BUILD.md)

データベースに対して実行する前に、このツールを自分でビルドする場合は、このガイドを参照してください。

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

各リリースでは、展開済み実行ファイル用の
`dbwarp-blueprint-<platform>.binary.sha256` ファイルも公開します。検証コマンドは
[バイナリのダウンロード](BINARIES.md)を参照してください。

## 認証方式固有のビルド

既定のビルドは password、token-file、token-env、TLS の各フローをサポートします。クライアント証明書 mTLS は PostgreSQL と MySQL で使用できます。

SQL Server 統合認証のサポートはプラットフォームごとに異なります:

| プラットフォーム | ビルドコマンド | 用途 |
|---|---|---|
| Linux | GitHub リリースの Linux バイナリ、または `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | パスワード、トークン、TLS による認証、および選択時の Kerberos / GSSAPI |
| Windows | GitHub release Windows binary, or `cargo build --release --locked --features winauth` | Windows Integrated Auth / SSPI |

Linux リリース版のバイナリは、起動時に Kerberos ライブラリを必要としません。
プラットフォームの GSSAPI ランタイムは、`--auth-mode integrated` が選択された
場合にのみ読み込まれます。`kinit` が動作する場合、必要なランタイムコンポーネントは
通常すでに存在します。ソースビルドでは、上記のように
`integrated-auth-gssapi` を使用して Kerberos/GSSAPI を有効にします。

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

`./build.sh` は、レビューされたソースコードが正常にビルドされることを証明します。バイト単位の同一性は、さらにリリース版の完全なネイティブビルド入力が必要となります。正確なソースコードのバージョンは `PROVENANCE.json` に記録されており、各リリース版とともに公開されています。そのターゲットと機能リスト、固定された Rust ツールチェーン、記録されたネイティブ compiler/linker、コミットのタイムスタンプ `SOURCE_DATE_EPOCH`、およびリリースワークフローのパスリマッピングとリンカーフラグを参照してください。Windows 版では、`clang-cl` と `/Brepro` も使用されます。

入力を再現した後、展開したリリースバイナリをローカル結果と比較します:

```bash
SOURCE_BIN=target/release/dbwarp-blueprint \
  ./verify.sh /path/to/extracted/dbwarp-blueprint
```

もしハッシュ値が異なる場合、そのバイナリを同等とみなさないでください。各リリースは2回ビルドされ、1バイトでも不一致があるとリリースは失敗します。`PROVENANCE.json`には、ソースのバージョン、ターゲット、機能、ツールチェーン、ソースの日付（エポック秒）、ネイティブコンパイラ、バイナリサイズ、およびローカルでの再現を評価するために必要なハッシュが記録されます。

## Vendored 依存関係

リポジトリには、パッチ済みの依存関係が `vendor/` 配下に含まれています。これらは
MySQL と SQL Server の `--tls-ca` に対する制限的な信頼規則を維持します。Linux 統合
認証は、要求された場合にのみ GSSAPI を読み込みます。Windows 統合認証では、保守
されている乱数生成用の依存関係を使用します。その他すべての依存関係バージョンは
`Cargo.lock` によって固定されています。

各 GitHub Release では、すべての依存関係ソースファイルをオフラインで検査してビルドしたいセキュリティチーム向けに、別個の `dbwarp-blueprint-source-vendored.tar.gz` バンドルを公開します。

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

このバンドルには、`vendor/` 配下のパッチ済み依存関係、その他すべての依存関係用に生成された
`vendor-crates/` ツリー、および crates.io をローカル vendor ツリーへリダイレクトする
生成済みの `.cargo/config.toml` が含まれます。このモードでは、`build.sh` は
`cargo build --release --frozen --offline --locked` を使用します。
