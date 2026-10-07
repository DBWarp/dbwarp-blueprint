# トラブルシューティング

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../TROUBLESHOOTING.md)を参照してください。

**言語:** [English](../TROUBLESHOOTING.md) | [Deutsch](../de/TROUBLESHOOTING.md) | [Français](../fr/TROUBLESHOOTING.md) | [Español](../es/TROUBLESHOOTING.md) | [Polski](../pl/TROUBLESHOOTING.md) | **日本語** | [简体中文](../zh/TROUBLESHOOTING.md)

一般的な `dbwarp-blueprint` の失敗と、次に行うべき対処を示します。

エラーは、安定した`DBPnnnnS`というメッセージコードで始まります。例えば、`DBP1001E`などです。ドキュメントを検索したり、問題を報告したりする際には、このコードを使用してください。詳細は、[オペレーター向けメッセージコード](MESSAGES.md)を参照してください。

## ソースビルドがステータス 2 または 3 で終了する

`build.sh` が、インストールされている `rustc` のバージョンが `rust-toolchain.toml` で指定されているバージョンと一致せず、かつネットワークアクセスが明示的に許可されていない場合に、ステータスコード 2 で終了します。正確な Rust のバージョンをインストールするか、ダウンロード範囲を確認した後、スクリプトが指定された、チェックサムで検証されたブートストラップをダウンロードすることを許可してください。

```bash
ALLOW_NETWORK=1 ./build.sh
```

`DBWARP_BLUEPRINT_OFFLINE=1` が設定されているのにソースツリーに
`vendor-crates/` ディレクトリがない場合、`build.sh` はステータス 3 で終了します。
通常のソースチェックアウトではこのモードを満たせません。同じリリースの
vendored ソースアーカイブをダウンロードして検証し、展開したツリー内でビルドを
実行してください:

```bash
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

異なるリリース間で依存関係をコピーしないでください。オンラインとオフラインの
完全な手順は[ソースからのビルド](BUILD.md)を参照してください。

## 指定した言語が使用されない

ロケール選択を診断するときは、サポートされている値を明示的に指定してください:

```bash
dbwarp-blueprint --lang pl --help
```

サポートされている値は `en`、`de`、`fr`、`es`、`pl`、`ja`、`zh` です。
`--lang` を指定しない場合、ツールは `DBWARP_BLUEPRINT_LANG`、`LC_ALL`、`LC_MESSAGES`、
`LANG` の順に確認します。サポートされていない明示的な値は
`DBP1011E` で拒否されます。不完全な組み込みカタログは、英語へ
フォールバックせずに、起動時に `DBP1010E` で失敗します。

Windows では通常 locale 変数がないため、`--lang` を渡すか `DBWARP_BLUEPRINT_LANG` を設定してください。

## バナーの幅または色が正しくない

バナー幅は、設定されていれば `COLUMNS`、それ以外は Linux/macOS の console、さらにそれ以外は 80 列です。色機能は `NO_COLOR`、`TERM`、`COLORTERM` から決まり、Windows で通常のように `TERM` がなければ 16 色を使います。`--color always`、`--color never`、または `COLUMNS` で上書きできます。

## URI 内のパスワードが拒否される

症状:

```text
DBP1001E refusing to use URI-embedded password
```

対処: URI からパスワードを削除し、次のいずれかを使用します:

```bash
--password-file /path/to/pass
--password-env DBWARP_BLUEPRINT_PASSWORD
```

Unix では、ファイルモードによってグループやその他のユーザーに読み取りを許可してはいけません。

## パスワードファイルの権限エラー

症状: 権限が広すぎるため、ツールが `--password-file` または `--tls-key` を拒否する。

対処:

```bash
chmod 600 /etc/dbwarp/db.pass
chmod 600 /etc/dbwarp/client.key
```

これにより、同じホスト上のローカルユーザーへの誤った開示を防ぎます。

## TLS 検証が失敗する

正しい CA バンドルとホスト名を指定して `--tls-mode verify-full` を使用してください:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

証明書のホスト名が一致しない場合は、DNS 名または証明書を修正してください。`--tls-skip-verify` は、`--i-know-what-im-doing` も指定しない限り、loopback 以外のホストでは拒否されます。本番環境では使用しないでください。

## SQL Server の TLS 信頼ルート

SQL Server で証明書を検証するモードでは、`--tls-ca` を省略すると
オペレーティングシステムのトラストストアを使用します。指定する `.pem` または
`.crt` ファイルには CA 証明書を正確に 1 つだけ含める必要があり、そのルートを
置き換えます。ドライバーは `verify-ca` と `verify-full` の両方で接続先の
ホスト名を検証します。

## 同意が必要

ライブデータベースの実行で、確認プロンプトに `y` または `yes` と回答しなかった場合の症状:

```text
DBP1701E aborted (no consent)
```

構造化ファイルの圧縮サンプリングを `--yes` なしで要求した場合の症状:

```text
DBP1006E ... --measure-compression requires --yes (consent flag)
```

対処: 実行前の概要を確認してプロンプトに `yes` と回答するか、非対話実行では
`--yes` を指定して再実行します:

```bash
--measure-compression --yes
```

Tier 2 は、制限された行サンプルをメモリへ読み込み、その後破棄するため、意図的に明示的な指定を要求します。

## サンプリングに時間がかかりすぎる

次の一方または両方を小さくします:

```bash
--sample-rows 500
--max-wall-secs 120
```

初回の本番レビューでは、圧縮測定を行わないより、小さな Tier 2 サンプルを使用する方が適切です。結果に偏りがある、または不完全な場合は、より大きな予算を設定してレプリカから再実行してください。

## DBA がカタログ外の SELECT 1 プローブを禁止している

RTT プローブを無効にします:

```bash
--no-rtt-probe
```

既定の RTT プローブは 5 回の `SELECT 1` クエリであり、行データを読み取りません。ただし、一部のポリシーではカタログ外のすべてのクエリが対象外と分類されます。

## 出力に圧縮セクションがない

圧縮セクションは、次の両方のフラグがある場合にのみ出力されます:

```bash
--measure-compression --yes
```

カタログのみのBlueprintは有効ですが、圧縮の見積もりは列の型から推測されます。

## 一部の圧縮サンプルが biased と記録される

一部のエンジンでは、すべてのケースで均一なテーブルサンプリングが提供されないため、小さなテーブルでは`LIMIT`の代替手段が必要になる場合があります。Blueprintファイルには`sampled_with_bias`と`bias_reason`が記録されており、これによりレビュー担当者はそれらを考慮することができます。

偏りのあるサンプルも有用ですが、一様なサンプルほど強い根拠にはなりません。

## TOML からのデッキ生成が失敗する

`--from-toml` は `--deck` と組み合わせる必要があります:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

`--from-toml` と一緒にライブデータベース用フラグを指定しないでください。監査境界を単純に保つため、本ツールはライブモードとオフラインモードの混在を拒否します。

## Blueprintファイルが小さすぎるように見える

通常、Blueprintファイルはコンパクトです。構造メタデータ、丸められた数、インデックス、FK グラフの形状、任意の圧縮サマリーを含みます。行の値や識別子は含まれないはずです。


## アップロードが発生していないことを証明する必要がある

監査ログとネットワークツールを使用します:

```bash
./dbwarp-blueprint ... --audit-log audit.txt
strace -f -e trace=connect ./dbwarp-blueprint ...
tcpdump host db.internal
```

期待される実行時ネットワーク動作は、有効なモードによって異なります。ライブの `--connect` 実行では、要求されたデータベースセッションを開きます。DNS は構成済みのリゾルバーに接続する場合があり、統合 Kerberos/SSPI 認証は KDC またはドメインコントローラーに接続する場合があります。バッチモードはデータベースソースごとに 1 つのデータベースセッションを開きます。ローカルの TOML、Parquet、Avro、バンドル操作はアプリケーションからネットワーク接続を開始しませんが、ネットワークマウントされたパスはホストのストレージスタックの影響を受けます。
