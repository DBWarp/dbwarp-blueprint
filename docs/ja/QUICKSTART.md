# クイックスタート

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../QUICKSTART.md)を参照してください。

**言語:** [English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | [Français](../fr/QUICKSTART.md) | [Español](../es/QUICKSTART.md) | [Polski](../pl/QUICKSTART.md) | **日本語** | [简体中文](../zh/QUICKSTART.md)

このクイックスタートは、データを公開することなく、共有可能なDBWarp Blueprintファイルを作成する必要があるDBA（データベース管理者）またはセキュリティ担当者向けです。

## 1. ツールの実行方法を選択する

次のいずれかを使用してください:

- リリースバイナリをダウンロードし、チェックサムを検証する。
- `./build.sh` でソースからビルドする。
- 厳格なオフライン依存関係レビューのため、vendored リリースバンドルからビルドする。

[`../BUILD.md`](BUILD.md) と [`../binaries/README.md`](BINARIES.md) を参照してください。

必要な場合は、表示言語を明示的に選択します:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --schema app --dry-run
```

サポートされている値は `en`、`de`、`fr`、`es`、`pl`、`ja`、`zh` です。
表示言語によって、ヘルプ、プロンプト、診断、進行状況テキスト、デッキの文章が変わります。
オプション名、受け付ける値、URI スキーム、セレクター、DBP コード、監査キー、
Blueprint TOML は決して変わりません。
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md)を参照してください。

## 2. 最小権限の専用アカウントを準備する

後でキャプチャに使用する `--dry-run` の例を含め、データベースへ接続する前に
この手順を実施してください。アプリケーション所有者、管理者、スーパー
ユーザー、`root`、`sa`、`db_owner` のアカウントから始めないでください。

1. 正確なエンジンとバージョン、データベース、承認済みのスキーマを特定します。
2. キャプチャのレベルを選択してください。`basic` はテーブルのカタログのみ、`standard` は行のサンプルを追加する場合、`enhanced` はテーブル以外のオブジェクトの分析も行う場合に選択します。
3. DBA は `sql/grants/<engine>/` にある該当スクリプトをコピーし、マークされた
   データベース、スキーマ、プリンシパル、パスワード、ロール切り替えの値を
   すべて編集したうえで、通常の変更管理手順に従って実行します。
4. そのスクリプトが作成した専用アカウントを使用し、すべての実接続コマンドで
   承認済みスキーマごとに `--schema NAME` を一つ指定します。
5. キャプチャの内容が確認された後、DBAにアカウントと権限を削除するための `sql/revoke/` のエンジン スクリプトを確認してもらい、実行してもらってください。

スクリプトは、厳密に範囲を限定した権限と便利な組み込みロールを意図的に区別し、
ロールが広すぎる場合を説明します。実行可能なスクリプトについては
[`../../sql/grants/README.md`](../../sql/grants/README.md)、バージョン別の DBA／
セキュリティ上の根拠については
[`../../sql/grants/DATABASE_PERMISSIONS.md`](../../sql/grants/DATABASE_PERMISSIONS.md)
を参照してください。コレクター自体はデータベースプリンシパルを作成、拡張、
削除しません。

## 3. 資格情報を安全に準備する

接続 URI にパスワードを含めないでください。本ツールは、プロセス一覧やシェル履歴への漏えいを避けるため、URI に埋め込まれたパスワードを拒否します。

推奨されるパスワードファイルのパターン（シークレットはエコーなしで入力され、
シェル履歴には残りません）:

```bash
sudo install -d -m 700 -o "$USER" -g "$(id -gn)" /etc/dbwarp
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

ユーザー名を URI エンコードしにくい場合は、それもファイルに保存します:

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\migration_user' > /etc/dbwarp/db.user
```

その後、`--user-file /etc/dbwarp/db.user` を使用します。

## 4. 最初にドライランする

ドライランは、接続せずに引数を検証し、予定されている操作を表示します:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

`--from-toml` デッキモードでは、ドライランはローカルの事前確認であり、データベースを読み取りません。

複数のソースの場合、バッチマニフェストに対してテスト実行を行ってください。

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 5. カタログのみのモードを実行する

カタログのみのモードは、メタデータと統計を読み取りますが、行サンプルは読み取りません:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

行サンプリングがポリシーで禁止されている場合、または最初のセキュリティレビューを行う場合に使用してください。

## 6. 非テーブル成果物の詳細度を選択する

既定の `--artifact-detail summary` は非テーブルカタログを読みますが、オブジェクト定義は読みません。有界件数と外部前提条件クラスを出力します。ポリシーがこれらのカタログを禁止する場合は `--artifact-detail none` を使用してください。件数だけを取得するトポロジプローブは引き続き実行されます。[権限リファレンス](../../sql/grants/README.md#topology-evidence)を参照してください。

匿名依存トポロジには `graph`、有界な言語機能および複雑度バンドには `analyzed` を使用します。どちらも明示的な同意が必要です:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


出力にはオブジェクト名、定義テキスト、エンドポイント、秘密、鍵、証明書、バイナリが含まれません。graph または analyzed モードを承認する前に [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) を参照してください。

## 7. Tier 2 圧縮測定を実行する

Tier 2 は、制限された行サンプルをメモリへ読み込み、圧縮、NULL 密度、
カーディナリティ/頻度、長さ、スタイルの集計測定値を計算して、サンプル値を
破棄します:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
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

可能な限り、Tier 2 を使用してください。これにより、より正確な転送サイズとデータ転送コストの見積もりが得られます。

## 8. デッキを生成する

ライブ実行中に生成する場合:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
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

## 9. 共有前にレビューする

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
- 圧縮、NULL 密度、カーディナリティ/頻度、長さ、スタイルの任意の集計測定値。
  サンプル値は含まれない

## 10. DBWarpと共有する。

共有する最小限の情報：

```text
blueprint.toml
```

複数のソースがある場合、作業ディレクトリを共有する代わりに、まとめて出力されたバンドルを作成して確認してください。

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

バンドルメタデータには、バッチマニフェストで選択した source id、tag、
dataset-group id が保持されます。匿名の値を使用し、転送前にレビューしてください。

複数のデータベースや、複数のParquetまたはAvroデータセットがある場合、または、特定のソースやテーブルのみを共有したい場合は、[バッチ収集とブループリントバンドル](BATCH_AND_BUNDLES.md) を参照してください。

### レビューと共有

既定では、レビュー済みの `blueprint.toml` またはパック済みバンドルのみを共有してください。デッキは、内容と機密区分を確認し、組織の方針に従って別途承認した場合に限り添付できます。

監査ログ、コマンド記録、および承認されていないプレゼンテーション資料は、ローカルに保存し、アクセスを制限してください。これらには、エンドポイント、認証されたユーザー、ローカルパス、タイミングデータ、およびマニフェスト識別子が含まれる場合があります。特定のサポートニーズがある場合に限り、承認された安全なチャネルを通じてのみ送信してください。パスワードファイル、トークンファイル、匿名化キー、CAの秘密鍵、データベースダンプ、またはデータベースログを、共有されたBlueprintと一緒に送信しないでください。
