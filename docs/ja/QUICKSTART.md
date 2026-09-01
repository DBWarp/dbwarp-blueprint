# クイックスタート

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../QUICKSTART.md)を参照してください。

**言語:** [English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | [Français](../fr/QUICKSTART.md) | [Español](../es/QUICKSTART.md) | [Polski](../pl/QUICKSTART.md) | **日本語** | [简体中文](../zh/QUICKSTART.md)

このクイックスタートは、顧客データを公開せずに共有可能な DBWarp Blueprintファイルを作成する必要があるセールスエンジニア、DBA、またはセキュリティレビュー担当者向けです。

## 1. ツールの実行方法を選択する

次のいずれかを使用してください:

- リリースバイナリをダウンロードし、チェックサムを検証する。
- `./build.sh` でソースからビルドする。
- 厳格なオフライン依存関係レビューのため、vendored リリースバンドルからビルドする。

[`../BUILD.md`](BUILD.md) と [`../binaries/README.md`](BINARIES.md) を参照してください。

必要な場合は、表示言語を明示的に選択します:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --dry-run
```

サポートされている値は `en`、`de`、`fr`、`es`、`pl`、`ja`、`zh` です。
表示言語によって、ヘルプ、プロンプト、診断、進行状況テキスト、デッキの文章が変わります。
オプション名、受け付ける値、URI スキーム、セレクター、DBP コード、監査キー、
Blueprint TOML は決して変わりません。
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md)を参照してください。

## 2. 資格情報を安全に準備する

接続 URI にパスワードを含めないでください。本ツールは、プロセス一覧やシェル履歴への漏えいを避けるため、URI に埋め込まれたパスワードを拒否します。

推奨されるパスワードファイルのパターン（シークレットはエコーなしで入力され、
シェル履歴には残りません）:

```bash
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

ユーザー名を URI エンコードしにくい場合は、それもファイルに保存します:

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\\migration_user' > /etc/dbwarp/db.user
```

その後、`--user-file /etc/dbwarp/db.user` を使用します。

## 3. 最初にドライランする

ドライランは、接続せずに引数を検証し、予定されている操作を表示します:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

`--from-toml` デッキモードでは、ドライランはローカルの事前確認であり、データベースを読み取りません。

複数の顧客ソースがある場合は、代わりにバッチマニフェストをドライランします:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 4. カタログのみのモードを実行する

カタログのみのモードは、メタデータと統計を読み取りますが、行サンプルは読み取りません:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

行サンプリングがポリシーで禁止されている場合、または最初のセキュリティレビューを行う場合に使用してください。

## 5. 非テーブル成果物の詳細度を選択する

既定の `--artifact-detail summary` は非テーブルカタログを読みますが、オブジェクト定義は読みません。有界件数と外部前提条件クラスを出力します。ポリシーがこれらのカタログを禁止する場合は `--artifact-detail none` を使用してください。

匿名依存トポロジには `graph`、有界な言語機能および複雑度バンドには `analyzed` を使用します。どちらも明示的な同意が必要です:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


出力にはオブジェクト名、定義テキスト、エンドポイント、秘密、鍵、証明書、バイナリが含まれません。graph または analyzed モードを承認する前に [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) を参照してください。

## 6. Tier 2 圧縮測定を実行する

Tier 2 は、制限された行サンプルをメモリへ読み込み、ローカルで圧縮し、比率のサマリーだけを書き込み、サンプルバイトを破棄します:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log blueprint.audit.txt
```

可能な場合は Tier 2 を使用してください。DBWarp によるワイヤーバイト、エグレスコスト、合成テキスト/バイナリデータ生成の推定精度が向上します。

## 7. デッキを生成する

ライブ実行中に生成する場合:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --audit-log blueprint.audit.txt \
  --yes
```

または、レビュー後にデータベース接続なしで生成する場合:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

## 8. 共有前にレビューする

次をレビューします:

```bash
less blueprint.toml
less blueprint.audit.txt
unzip -l blueprint.pptx  # optional deck package inspection
```

期待される特性:

- 実際のテーブル名がない
- 実際の列名がない
- 行の値がない
- 固定ヘッダー以外のコメントがない
- 数とバイトサイズが丸められている
- `table-001`、`col-1`、`schema-A` などの匿名化 ID が使用されている
- 有界な成果物件数と、承認済みの場合は匿名成果物 ID
- 黙って省略せず、不完全または読み取り不能な成果物の明示的な証拠
- 任意の圧縮比だけがあり、サンプルバイトはない

## 9. DBWarp へ引き渡す

最小限の引き渡し:

```text
blueprint.toml
```

複数ソースの顧客レビューでは、作業ディレクトリを引き渡すのではなく、
パック済みバンドルを作成して確認します:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

バンドルメタデータには、バッチマニフェストで選択した source id、tag、
dataset-group id が保持されます。匿名の値を使用し、転送前にレビューしてください。

顧客に複数のデータベース、複数の Parquet または Avro データセットがある場合、またはベンチマーク生成用に選択したソース/テーブルだけを承認する場合は、`docs/BATCH_AND_BUNDLES.md` を使用してください。

既定では、次の項目をアクセス制御されたローカル証拠として保管します:

```text
blueprint.audit.txt
blueprint.pptx
command-used.txt
```

監査や保存したコマンドには、データベースエンドポイント、認証済みプリンシパル、
ローカルパス、タイミングデータ、マニフェストの source id が含まれる場合があります。
特定のサポート上の必要がある場合に限り、承認済みの安全な経路で送信してください。
パスワードファイル、CA 秘密鍵、顧客ダンプ、データベースログは送信しないでください。
