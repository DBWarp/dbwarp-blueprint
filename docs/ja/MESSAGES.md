# オペレーターメッセージコード

> **翻訳に関する注意:** この文書は機械支援による翻訳であり、ネイティブによる技術レビューは未完了です。契約上の正式文書として扱わないでください。[英語の正本](../MESSAGES.md)を参照してください。

**言語:** [English](../MESSAGES.md) | [Deutsch](../de/MESSAGES.md) | [Français](../fr/MESSAGES.md) | [Español](../es/MESSAGES.md) | [Polski](../pl/MESSAGES.md) | **日本語** | [简体中文](../zh/MESSAGES.md)

`dbwarp-blueprint` は、DBWarp が所有する検証およびワークフローの失敗に、安定したオペレーターメッセージ識別子を使用します。
形式は IBM スタイルのオペレーターメッセージに着想を得ています。サブシステムプレフィックス、数値識別子、重大度サフィックスで構成されます。
IBM CICS の文書では、プログラム識別子、4 桁のメッセージ番号、重大度文字が説明されています。IBM MQ も同様に、component/prefix field、数値識別子、末尾の message type code を使用します。Microsoft の error-message guidance は、エラーが問題を説明し、ユーザーが実行できる対処を提示すべきだという実用的な原則を補強しています。

参考資料:

- IBM CICS message format: https://www.ibm.com/docs/en/cics-pa/5.3.0?topic=messages-message-format
- IBM CICS message information layout: https://www.ibm.com/docs/en/cics-ts/6.x?topic=messages-format-cics-message-information
- IBM MQ for z/OS message format: https://www.ibm.com/docs/SSFKSJ_9.2.0/com.ibm.mq.ref.doc/q050270_.htm
- Microsoft error-message guidance: https://learn.microsoft.com/en-us/windows/win32/uxguide/mess-error

## 形式

```text
DBPnnnnS message text. Next: corrective action.
```

フィールド:

- `DBP` は DBWarp Blueprint を意味します。
- `nnnn` は安定した 4 桁のメッセージ番号です。
- `S` は重大度です。`E` は error、`W` は warning、`I` は information を表します。

コードは安定しており、言語に依存しません。`--lang` またはプロセスの locale によって
サポートされる言語が選択されると、summary、cause、corrective action がローカライズされます。
動的なオペレーティングシステム、データベースドライバー、パス、因果チェーンの
詳細はそのまま残るため、サポートエンジニアは元の失敗を検索できます。
メッセージテキストに、シークレットまたは redaction されていない接続 URI を含めてはいけません。

## 範囲

| 範囲 | 領域 |
|---|---|
| `DBP0001E` | 因果チェーンを伴う、真に未分類のラップされた失敗 |
| `DBP10xxE` | コマンド、接続入力、収集ポリシーの検証 |
| `DBP11xxE` | バッチマニフェストとソース入力の検証 |
| `DBP12xxE` | バンドルセレクターと Blueprint URI セレクター |
| `DBP13xxE` | オフライン TOML/デッキ/スキーマの検証 |
| `DBP14xxE/W` | ライブデータベース収集の失敗と、致命的でないサンプリングの機能低下 |
| `DBP15xxE/W` | 構造化ファイル、Blueprint、デッキ、監査の出力 |
| `DBP16xxE/W` | 資格情報、認証、TLS、機密ファイルのポリシー |
| `DBP17xxE` | オペレーターの同意 |
| `DBP18xxE` | プロセスの runtime initialization |

## 現在のコード

| コード | 意味 |
|---|---|
| `DBP0001E` | 未分類の失敗。後に因果チェーンが続く。 |
| `DBP1000E` | オフラインモード以外で `--connect` がない。 |
| `DBP1001E` | URI に埋め込まれたパスワードを拒否した。 |
| `DBP1002E` | サポートされていない `--connect` URI scheme。 |
| `DBP1003E` | サポートされていない TLS server-name override。 |
| `DBP1004E` | SQL Server 以外のエンジンで Azure token flag が使用された。 |
| `DBP1005E` | 選択したエンジンでは認証モードを使用できない。 |
| `DBP1006E` | 明示的な `--yes` なしで構造化ファイルの sampling が要求された。 |
| `DBP1007E` | その契約をまだ公開していないエンジンで、明示的な length-fidelity mode が要求された。 |
| `DBP1008E` | 従来の exact-length alias が strict length fidelity と競合している。 |
| `DBP1009E` | 明示的な `--yes` なしで、正確な sampled-length fidelity が要求された。 |
| `DBP1010E` | 組み込み localization catalog が不完全または不整合。 |
| `DBP1011E` | コマンドライン引数が無効。 |
| `DBP1012E` | サポートされるデータベース接続 URI の形式が不正。 |
| `DBP1013E` | `--source-kind` が空、またはサポートされていない。 |
| `DBP1014E` | 明示的な同意なしで匿名成果物グラフまたは定義解析が要求されました。 |
| `DBP1015E` | クライアント証明書を実装していない SQL Server ドライバーで TLS クライアント証明書オプションが使用された。 |
| `DBP1101E` | バッチマニフェストを読み取れない。 |
| `DBP1102E` | バッチマニフェストを parse できない。 |
| `DBP1103E` | バッチマニフェストに `[[source]]` entry がない。 |
| `DBP1104E` | バッチモードに明示的な `--yes` が必要。 |
| `DBP1105E` | バッチ内の 1 つのソースが失敗した。 |
| `DBP1106E` | サポートされていない batch source kind。 |
| `DBP1107E` | ファイルソースから入力ファイルが 1 つも解決されなかった。 |
| `DBP1108E` | サポートされていない file dataset mode。 |
| `DBP1109E` | batch source identifier に使用可能な ASCII letter または digit がない。 |
| `DBP1110E` | データベースソースの接続ソース数が正しくない。 |
| `DBP1111E` | `connect_env` 変数がない、または読み取れない。 |
| `DBP1112E` | `connect_file` がない、または読み取れない。 |
| `DBP1113E` | バッチ出力、監査、レポート、またはディレクトリを完了できなかった。 |
| `DBP1114E` | 構造化ファイルデータセットのメンバーに互換性がない。 |
| `DBP1115E` | すべてのバッチソースが失敗し、診断出力のみが公開されました。 |
| `DBP1116E` | 部分的なバッチバンドルが公開されました。 |
| `DBP1200E` | selector または `blueprint://` syntax が無効。 |
| `DBP1201E` | バンドルセレクターに一致するソースがない。 |
| `DBP1202E` | バンドルセレクターが複数のソースに一致した。 |
| `DBP1203E` | バンドルセレクターに一致する抽出可能なBlueprint/テーブルがない。 |
| `DBP1204E` | バンドル入力を読み取れなかった。 |
| `DBP1205E` | バンドルまたは参照先Blueprintの内容が無効。 |
| `DBP1206E` | バンドル出力を書き込めなかった。 |
| `DBP1301E` | `--from-toml` に `--deck` がない。 |
| `DBP1302E` | サポートされていない Blueprint TOML schema version。 |
| `DBP1401E` | PostgreSQL capture boundary が失敗した。 |
| `DBP1402E` | MySQL または MariaDB capture boundary が失敗した。 |
| `DBP1403E` | SQL Server capture boundary が失敗した。 |
| `DBP1404W` | PostgreSQL TLS `prefer` モードがループバックで平文にフォールバックしました。 |
| `DBP1405W` | 任意の database RTT probe を利用できなかった。 |
| `DBP1406W` | Tier 2 sampling time budget を使い切った。 |
| `DBP1407W` | compression sample を利用できなかった。 |
| `DBP1408W` | text-column style sample を利用できなかった。 |
| `DBP1409W` | PostgreSQL の asynchronous connection task がエラーを報告した。 |
| `DBP1410W` | 任意の成果物カタログを利用できなかったため、完全性を明示的に低下させました。 |
| `DBP1411W` | topology evidence を利用できず、deployment と local role は unknown のままです。 |
| `DBP1412W` | distributed または sharded layout を検出しましたが、完全な aggregate sizing を利用できません。 |
| `DBP1413W` | dataset の table、row、byte coverage が不完全または unknown です。 |
| `DBP1414W` | bundle source relationship が unknown のため、source 間算術は安全ではありません。 |
| `DBP1415W` | 宣言された replica が一致せず、平均せず deterministic representative を保持しました。 |
| `DBP1416W` | shard group が不完全で、aggregate total に寄与しません。 |
| `DBP1417W` | bundle aggregate total を抑止しました。 |
| `DBP1418W` | bundle 算術に含まれる source の dataset coverage が不完全または unknown です。 |
| `DBP1419E` | ライブ取得が `--max-wall-secs` を超えたため、クライアントは接続を切断し、エンジン固有のサーバー制限を報告しました。 |
| `DBP1420E` | 要求された `--schema` の少なくとも 1 つが表示されなかったため、範囲が曖昧な Blueprint は書き込まれませんでした。 |
| `DBP1421W` | SQL Server セッションのプリンシパル情報を取得できなかったため、識別情報を断定せず取得を続行しました。 |
| `DBP1501E` | structured-file capture boundary が失敗した。 |
| `DBP1502E` | Blueprintまたはバンドルの出力が失敗した。 |
| `DBP1503E` | PowerPoint デッキの生成が失敗した。 |
| `DBP1504W` | 監査ログを書き込めなかった。 |
| `DBP1601E` | 資格情報の取得に失敗した。 |
| `DBP1602E` | TLS 設定に失敗した。 |
| `DBP1603E` | データベースユーザー名の取得に失敗した。 |
| `DBP1604E` | データベース認証設定が無効。 |
| `DBP1605W` | このプラットフォームでは sensitive-file permission enforcement を利用できない。 |
| `DBP1606E` | 認証された SQL Server プリンシパルの検証がカタログ取得前に失敗しました。 |
| `DBP1607E` | 匿名化 HMAC キーを安全に初期化できませんでした。 |
| `DBP1701E` | 明示的な同意前に操作がキャンセルされた。 |
| `DBP1702E` | 標準入力から consent response を読み取れなかった。 |
| `DBP1801E` | asynchronous runtime を初期化できなかった。 |

広告するすべての言語には、現在のすべての DBP summary、cause、action が必要です。
バイナリは起動時にこれを検証し、英語へ暗黙にフォールバックせず、`DBP1010E` で失敗します。

予測可能な decision-boundary failure は、adversarial CLI matrix で検証されます。
既知の状態は、その固有コードを最初の operator code として出力し、`DBP0001E` に
フォールバックしてはいけません。renderer は error chain 全体も走査するため、
コードのない implementation context によってコード付きの inner cause が隠されることはありません。

致命的でないデータベースサンプリング警告は、安定した warning code とともに出力され、
実行監査に記録されます。これにより、optional probe failure を収集全体の失敗にせず、
完全な Tier 2 capture と、一部だけをサンプリングして成功した capture を区別できます。

## サポートチェックリスト

顧客から失敗が報告された場合、次を依頼してください:

- `DBP` コードを含む完全な端末出力
- `--audit-log` を使用していた場合は監査ログ
- redaction 済みのコマンドライン
- バンドルエラーの場合は `dbwarp-blueprint --bundle-list ...` の出力

パスワードファイル、トークンファイル、秘密鍵、生のデータベース行サンプルを依頼してはいけません。
