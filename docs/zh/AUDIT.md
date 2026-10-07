# dbwarp-blueprint 读取和写入的内容

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../AUDIT.md)。本译文不应被视为合同级文本。

**语言：** [English](../../AUDIT.md) | [Deutsch](../de/AUDIT.md) | [Français](../fr/AUDIT.md) | [Español](../es/AUDIT.md) | [Polski](../pl/AUDIT.md) | [日本語](../ja/AUDIT.md) | **简体中文**

本文档描述应用程序运行时在网络、文件系统、环境、数据库和审计方面的行为。
请将每种活动模式和选项与您的安全策略交叉核对。数据库驱动程序、TLS 和身份
库、动态加载器以及操作系统还可能查阅平台配置、信任库、DNS、凭据缓存和网络
存储；应用程序审计无法完整观察这些支持层操作。

## 网络出口

PostgreSQL、MySQL 和 SQL Server 的实时 `--connect` 模式会与指定端点建立一个数据库驱动程序会话。批处理模式依次
处理来源，并为每个数据库来源建立一个会话。DNS 解析可能使用已配置的解析器，
集成式 Kerberos/SSPI 身份验证可能联系 KDC 或域控制器。离线 TOML、Parquet、
Avro 和捆绑包操作不会建立由应用程序发起的网络连接，但网络文件系统上的路径
仍受主机存储堆栈影响。

需要明确确认的 Oracle 预览只会启动通过 `--oracle-sqlplus` 明确指定的 SQL*Plus 可执行文件（如果可用，则包括一次有界的 `-V` 探测），并将其用于目录会话。子进程会收到一个空的私有目录作为 `TNS_ADMIN`；凭据写入标准输入，绝不放入进程参数。环境会在启动前清空。只转发现有的 `PATH`、`SystemRoot`、`WINDIR`、`ORACLE_HOME`、`LD_LIBRARY_PATH`、`DYLD_LIBRARY_PATH`、`LIBPATH` 和 `SHLIB_PATH`；采集器会另行设置固定的区域设置、时区和私有 `TNS_ADMIN` 值。

二进制文件中不存在遥测、许可证检查、版本更新、云 API 调用或上传路径。

您可以使用 `strace -f -e trace=connect,sendto,recvfrom`、`tcpdump` 或所选平台上的 eBPF 进行验证。

## 文件系统读取

工具读取活动模式选择的输入：

| 文件 | 使用时机 | 内容 |
|---|---|---|
| `--user-file PATH` | 如果提供 | 仅用户名。删除末尾空白；空文件会导致错误。 |
| `--password-file PATH` | 如果提供 | 读取一次。DBWarp 所有的 `Secret` 缓冲区会在销毁时归零；驱动程序可能按 SECURITY.md 所述保留自己的副本。在 Unix 上，如果组或其他用户可读，则拒绝。 |
| `--anonymization-key-file PATH` | 如果提供 | 您持有的 32 字节或 64 个十六进制字符的 HMAC 密钥。在 Unix 上，如果组或其他用户可读，则拒绝。密钥绝不会输出。 |
| `--azure-token-file PATH` | 如果提供 | SQL Server Entra ID 令牌。读取一次；DBWarp 所有的 `Secret` 缓冲区会在销毁时归零。在 Unix 上，如果组或其他用户可读，则拒绝。 |
| `--tls-ca PATH` | 如果提供 | 连接时读取的受信任 CA PEM。PostgreSQL/MySQL 接受捆绑包；SQL Server 只接受一个证书。提供的文件会替换引擎的默认根证书。 |
| `--tls-cert PATH` | 如果提供 | PostgreSQL/MySQL 客户端 TLS 证书（PEM），连接时读取。SQL Server 会以 `DBP1015E` 拒绝。 |
| `--tls-key PATH` | 如果提供 | PostgreSQL/MySQL 客户端 TLS 密钥（PEM）。在 Unix 上，如果权限模式允许其他用户或同组用户读取，则拒绝。连接时读取；SQL Server 会以 `DBP1015E` 拒绝。 |
| `--from-toml PATH` | 如果提供 | 现有 dbwarp-blueprint TOML 文件，在本地读取以构建演示文稿，不连接数据库。 |
| `--from-parquet PATH` | 如果提供 | Parquet 元数据，并且仅在明确同意采样时读取有界数量的已解码行。 |
| `--from-avro PATH` | 如果提供 | Avro 容器元数据和记录；为获取行数会遍历容器。 |
| `--batch-manifest PATH` | 如果提供 | 清单以及它引用的每个本地输入、凭据、令牌和 TLS 路径。 |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | 如果提供 | 捆绑包 TOML，以及列出、提取或打包所需的相对路径 Blueprint 文件。 |
| 控制终端/控制台（类 Unix 系统上的 `/dev/tty`） | 如果未提供密码来源 | 关闭回显的提示。 |
| （仅构建时）`rust-toolchain.toml`、`Cargo.toml`、`Cargo.lock`、vendored 发布中的 `.dbwarp-source-revision`、`vendor/*`、`vendor-crates/*` | 仅运行 `./build.sh` 时 | 工具链、源出处和标准 Cargo 构建输入 |

应用程序没有显式路径读取：
- `~/.pgpass`、`~/.my.cnf`、`~/.aws/credentials`、`~/.azure/credentials`
- 任何 `~/.ssh/*` 文件
- 作为 Blueprint 输入的 `/etc/passwd` 或 `/etc/shadow`（但平台身份和认证库
  仍可能查阅操作系统账户数据）
- 除通过 `--password-env`、`--user-env` 或 `--azure-token-env` 指定的变量之外的数据库凭据变量。集成 Kerberos 构建还可能使平台 GSSAPI/Kerberos 栈查阅其自身配置、缓存、keytab 和环境变量。语言和终端显示变量见下文。

## 文件系统写入

工具只写入活动模式选择的输出：

| 文件 | 使用时机 | 内容 |
|---|---|---|
| `--out PATH`（默认 `./blueprint.toml`） | 实时数据库、Parquet、Avro、bundle-extract 和 bundle-pack 运行 | Blueprint 或已打包捆绑包 TOML。仅演示文稿、bundle-list、dry-run 或 help/version 模式不写入。 |
| `--deck PATH` | 仅在指定时 | 汇总匿名化 Blueprint 的 PowerPoint (.pptx) 演示文稿。在本地从相同的内存中 Blueprint 或 `--from-toml` 输入构建；不额外读取数据库、不使用网络、不使用第三方库。 |
| `--audit-log PATH` | 仅在指定时 | 输出到 stderr 的审计日志的原子替换副本；不会追加到现有内容。 |
| `--out-dir DIR` | 非 dry-run 批处理模式 | `bundle.toml`、各来源的 `blueprints/` 和 `audits/`、所有权标记，以及部分失败后的 `errors.txt`。发布使用同级暂存目录和恢复标记。 |
| （仅构建时）`./target/`、`./build/` | 仅运行 `./build.sh` 时 | 标准 Cargo 构建输出 |

应用程序没有显式写入以下位置的路径：
- `/var/log/*`
- `~/.cache/*`、`~/.local/*`、`~/.config/*`
- 隐式系统临时目录（用户仍可明确将输出或批处理目录指定到该位置）

## 读取的环境变量

审计仅列出 DBWarp Blueprint 自身查阅的变量。如果 `--lang` 未选择受支持的语言，语言选择可能读取
`DBWARP_BLUEPRINT_LANG`、`LC_ALL`、`LC_MESSAGES` 和 `LANG`（按此顺序）。终端呈现可能读取 `NO_COLOR`、`TERM`、
`COLORTERM` 和 `COLUMNS`；这些变量只影响显示。

指定 `--password-env VAR_NAME` 或 `--user-env VAR_NAME` 时，工具只读取该指定变量。不会回退到 `PGPASSWORD`、`MYSQL_PWD`、`MSSQL_PASSWORD`、`USER` 或 `LOGNAME` 等常见默认值；这些回退有意不予实现。

平台数据库、TLS、DNS 和集成认证库可能查阅本应用程序级列表之外的自身变量和
配置。如果策略要求完整的进程和库清单，请使用操作系统级跟踪。

运行 `./build.sh` 时，它会读取 `PINNED_RUST`（覆盖项）、`ALLOW_NETWORK`（选择性启用 rustup-init 下载）、`TARGET`（交叉编译目标），以及标准 cargo / rustup 变量。工具本身在运行时不会读取这些变量。

## 每次运行的审计日志

工具在每次运行时都会向 stderr 输出审计日志，采用稳定的纯文本布局。可使用 `2>audit.txt` 将其传送到文件，或使用 `--audit-log PATH` 显式保存副本。

示例（Tier 1）：

```
=== dbwarp-blueprint audit ===
build_source_revision: 0123456789abcdef0123456789abcdef01234567
build_source_dirty:    false
build_toolchain:     1.94.0 (vendored)
mode:                tier-1
started_at_unix_ms:  1745596800000
outcome:             ok
anonymization_key:   ephemeral-random
schema_selector_count: 1

connection:
  - postgresql://app@db.example:5432/payments
    auth: scram-sha-256-or-md5
    tls: yes (protocol version unavailable from driver)
    tls_ca_only: false

auth:
  user_source:        file:/etc/dbwarp/db.user
  password_source:    file:/etc/dbwarp/db.pass (mode 0o600)
  password_persisted: false
  password_logged:    false
  authenticated_principal: (not observed)
  effective_server_principal: (not observed)
  database_principal: (not observed)
  expected_server_principal: (not requested)
  principal_assertion: not-observed

topology_and_scope:
  topology:
    deployment: unknown
    local_role: unknown
    visibility: partial
    member_count: 2
    identifiers_redacted: true
    role_counts: primary=1, secondary=1
    features: postgresql-streaming-replication
    catalogs_read: pg-is-in-recovery, pg-stat-replication
    catalogs_unreadable: (none)
  dataset_scope:
    layout: full-copy
    table_inventory_completeness: complete
    row_count_completeness: complete
    size_completeness: complete
    row_count_method: postgres-planner-estimate
    size_method: postgres-local-relation-size
    limitations: row-counts-statistical

blueprint_fidelity_estimate:
  basis: evidence-coverage-v1
  overall_score: 79/100
  band: good
  structure_score: 90/100
  sizing_score: 100/100
  column_statistics_score: 68/100
  relationship_score: 75/100
  artifact_score: 50/100
  limitations: biased-column-sampling, cardinality-lower-bounds
  qualification: evidence estimate, not source-truth accuracy or a confidence interval

artifact_inventory:
  detail: summary
  visibility: full
  objects: 42
  dependency_edges: 0
  external_prerequisites: 3
  inventory_complete: false
  dependencies_complete: false
  requirements_complete: false
  analysis_complete: false

database_operations_observed:
  1. [succeeded, 14ms, 28 rows]   server version lookup
  2. [succeeded, 9ms, 312 rows]   column catalog lookup
  ... (every observed catalog operation enumerated)

wire_bytes_observed:
  catalog_responses: unknown (driver does not expose wire-byte totals)
  row_data:          unknown (driver does not expose wire-byte totals)

local_sample_processing:
  encoded_rowframe_bytes: 0 B

sampling_work:
  compression_workers: 0
  compression_queue_capacity: 0
  compression_jobs_submitted: 0
  compression_jobs_completed: 0
  compression_pipeline_wall_ms: 0
  compression_worker_ms: 0
  tables_skipped_proven_empty: 0
  chunk_level_3_attempts: 0
  table_level_3_attempts: 0
  column_level_3_attempts: 0

files_read_local:
  - /etc/dbwarp/db.pass        (mode 0o600 ✓)

files_written_local:
  - ./blueprint.toml         (12 KiB, sha256: 7f3e2af1...)

warnings:
  - (none)

network_egress:
  - db.example:5432 (database-driver session; DNS may use the configured resolver)

env_vars_read:
  - (none)

trust_assertions:
  - no row content was read
  - no telemetry was sent anywhere
  - length policy balanced: declared capacities and index prefixes exact; sampled lengths relatively rounded
  - identifier ordering uses domain-separated HMAC-SHA256 with a fresh process-local key; labels intentionally vary between runs
  - the anonymization key and source identifiers are not written to the Blueprint
  - artifact summary stores bounded counts and external-prerequisite classes; no object identities or definitions
  - artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries
  - credential entered through the Secret wrapper and its buffer is zeroized on drop; driver APIs may retain copies as documented under 'Driver-owned credential copies' in SECURITY.md

run_duration_ms:    142
finished_at_unix_ms: 1745596800142
=== end audit ===
```

MySQL 运行会输出一个特定于模式的 `length policy balanced|strict|exact` 断言。它分别说明结构长度和采样长度是精确值还是舍入值，因此对于 balanced 或 exact 运行，审计不会声称所有数值都已舍入。

审计日志：

- 仅记录可重复实时 `--schema` 选择器的数量；其值会显示在交互式预检查中，但不会添加到审计。现有的已脱敏连接 URI 仍会标识已连接数据库，而在 MySQL 中该数据库名也是 schema 名称。受选择器限制的 Blueprint 会在 `dataset_scope` 中标记 `selection-limited`。
- 记录编译时嵌入的源修订版本以及工作树是否有改动；二进制文件无法嵌入自身的最终哈希，因此最终 SHA-256 仍由发布或注册表外部校验和提供。
- 记录凭据的**来源**（文件路径、环境变量名称、TTY），绝不记录值。
- 对于 SQL Server，记录 `ORIGINAL_LOGIN()`、`SUSER_SNAME()` 和
  `USER_NAME()` 返回的确切会话身份。若提供 `--expect-server-principal`，
  还会记录预期值及服务器端比较在目录采集前是否匹配。
- 列出每个观察到的数据库操作及其结果、耗时和驱动提供的行数；终止失败使用不含标识符的有界标签。
- 当驱动不公开网络字节数时报告 `unknown`，并单独报告本地编码的样本字节数。
- 报告本地写入的总字节数（包含每个文件的 sha256）。
- 使用稳定的 DBP 警告代码记录非致命采集和采样降级；空部分表示未观察到已知降级。
- 将经过验证的 `[database_topology]` 和 `[dataset_scope]` 证据复制到 `topology_and_scope`，且只使用封闭 token 和计数；节点名、端点、集群标识符和数据库标识符均无法出现。
- 在拓扑或数据集覆盖不完整时保留 `DBP1411W`、`DBP1412W` 和 `DBP1413W`，确保成功采集不会隐藏容量限定条件。
- 记录确定性的、按维度拆分的 Blueprint 保真度估计。该分数描述结构、容量估算、列统计、关系和工件方面已捕获证据的覆盖度；它不是相对于源数据真实值的实测误差，也不是统计置信区间。报告为过时或从未分析过的 PostgreSQL 统计信息会降低容量估算维度的分数，并以明确的 `table-statistics-stale` 或 `table-statistics-never-analyzed` 限制记录下来。统计信息的新鲜度不能弥补行数覆盖的缺失。对于使用持续更新计数器的引擎，或无法确定统计信息新鲜度的引擎，不会凭空赋予其新鲜度证据。
- 根据模式（Tier 1 或 Tier 2）声明相应的信任断言。
- 使用稳定的文本格式，但值可能随数据库状态、时间、警告以及默认生成的新匿名化
  密钥而变化。对于获准的比较，请重复使用受保护的 `--anonymization-key-file` 并固定
  `--generated-at`；计时字段仍会变化。

**信任断言的条件输出。** 只有在实际读取过凭据的运行中，才会输出 "credential entered through the Secret wrapper..." 行。在获取凭据前就中止的失败路径（URI 解析错误、拒绝 URI 内嵌密码、试运行等）有意不输出此行，因为没有对从未获取的凭据作出断言。可通过该行是否存在以及 `auth.password_source` 来判断给定运行是否执行了凭据处理。

**审计信息会在操作成功和失败时发出，**包括启动后命令行解析失败。 Help/version 退出以及在嵌入式本地化模块加载之前发生的失败不会生成完整的审计信息。 如果工具在执行过程中失败（例如，身份验证被拒绝，网络错误），审计日志仍然会输出到标准错误流（以及如果指定了 `--audit-log PATH`），并且会包含 `outcome: error: <stage>` 信息，这样您始终可以记录在失败之前尝试的内容。 示例失败情况的输出行：

```
outcome:             error: parsing --connect URI (value redacted to avoid logging embedded credentials)
```

终端输出还包含带因果链的编码运维摘要，例如 `DBP1001E` 或 `DBP0001E`。审计结果长度有界，可能会截断过长文本；请结合终端输出和消息代码进行支持分诊。请参阅 `docs/MESSAGES.md`。

可选 RTT、压缩和文本样式探测的失败不会使主要目录采集无效。这些情况会以 `DBP1405W` 到 `DBP1408W` 打印并保留在 `warnings:` 下，因此可以区分成功但不完整的 Tier 2 结果与完整结果，而不会把可选探测失败变为整体采集失败。重复的相同警告会被去重，多行驱动程序详细信息会被展平，使审计保持有界且便于机器扫描。

## 非表对象读取

对象采集独立于 Tier 2 行采样：

- `--artifact-detail none` 会跳过对象清单目录和定义；仅计数的拓扑探测仍会运行。
- `summary` 读取已建模对象目录，但不读取定义文本。
- `graph` 还会读取依赖目录，但不读取定义文本。
- `analyzed` 还会把可用的 SQL/过程定义读入有界进程内存以进行词法分析。

审计会记录所请求的详细级别、可见性、对象/依赖/外部前提计数以及全部完整性标志。每个对象目录操作都会出现在 `database_operations_observed` 中。可选目录读取失败会发出 `DBP1410W`，记录在 `warnings` 中，并阻止不准确的完整性声明。

在分析模式下，定义会被封装在一个零化所有者中，进行清理，并被简化为有限范围的段落和封闭的特征标记。定义文本、源对象名称、外部端点、工件主体、凭据、密钥/证书材料、软件包/库名称以及二进制文件，绝不会写入到 Blueprint 或审计日志中。唯一保留的精确主体名称是显式 `auth` 审计块中的三个 SQL Server 会话身份；它们绝不会写入到 Blueprint、演示文稿或捆绑包文件中。图形模式和分析模式需要 `--yes`，因为即使是匿名拓扑也可能识别出应用程序。

- summary：仅有界计数，不含对象身份或定义；
- graph：匿名依赖图，不含定义；
- analyzed：临时读取定义，只保留有界特征区间。

对象系列覆盖范围和完整性解释参见 [`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)。

## Tier 2 附加操作

交互式接受压缩测量，或以非交互方式传入 `--measure-compression --yes` 后，工具还会：

- 对每个未被证明为空的表运行引擎特定的有界采样路径。PostgreSQL 先根据估算行数
  和请求的样本大小，为 `TABLESAMPLE SYSTEM` 计算自适应百分比，并配合 `LIMIT N`，
  需要时回退到 `LIMIT N`；如果存在安全的
  访问路径，MySQL 会使用四个有界的数值主键范围窗口，否则使用 `LIMIT N`；
  SQL Server 使用 `TOP N`。有偏差的路径会在输出中标记
  `sampled_with_bias = true`。
- 将采样行读取到本地内存缓冲区。
- 数据库读取保持串行。`--compression-workers N` 可运行 1–32 个有界本地压缩
  worker（默认值为 1，以尽量减少对源主机的影响）。要使用更多本地 CPU，请显式
  增大该值。每个 worker 拥有自己的 zstd 上下文，不使用共享的 zstd 锁。
- 使用 zstd 在级别 3 进行压缩。
- 记录所得比率和标准差。
- **每个有界本地压缩作业完成后丢弃其缓冲区**。这些字节不会写入磁盘，也不会
  传输。worker 池最多保留 N 个排队样本和 N 个正在压缩的样本。

`local_sample_processing.encoded_rowframe_bytes` 显示为压缩在本地编码的字节数，而不是数据库网络字节数。驱动不公开的值仍为 `unknown`。`[compression]` 块包含比率。`--max-wall-secs` 是整个实时采集的硬性截止时间，包括连接、目录、RTT 和 Tier 2。
PostgreSQL 还会设置会话 `statement_timeout`；MySQL 对只读 `SELECT` 设置会话
`max_execution_time`；SQL Server 因没有等效的会话范围语句经过时间限制，而设置会话
`LOCK_TIMEOUT`。到达外层截止时间时，客户端会断开连接。审计不会将该断开视为 SQL Server 已确认取消的证明，因此操作员必须在重试前确认服务器工作已停止。

`sampling_work` 是不含标识符的运行证据。它记录本地 worker 和队列上限、每表 16 MiB 的投影负载上限、已提交
和已完成的作业、压缩尝试，以及由于引擎目录在读取时证明表为空而跳过采样的表。
`compression_worker_ms` 是 worker 的累计墙钟时间，不是进程 CPU 时间；当 worker
重叠运行时，它可能大于 `compression_pipeline_wall_ms`。管道墙钟时间可以与仍然
串行的数据库读取重叠。这些计数器描述已完成的工作，不是数据库行数、网络字节
测量或源准确性声明。

## 验证协议

下载的平台归档和从源代码重现构建是两种不同的信任检查。请先使用同一发布版本
提供的校验和验证归档及其中的可执行文件。平台归档是供操作人员使用的包，而不是
源代码树；其中的文档和 `verify.sh` 是与匹配源代码检出配合使用的参考资料。
在尝试源代码审计或重新构建之前，请从公共仓库获取确切的发布标签，或使用该发布
版本的带依赖源代码归档。不要期望在平台归档中直接重新构建。

如果您希望*证明*工具只执行本文档说明的操作：

1. **下载完整性**：使用 `SHA256SUMS.txt` 中的对应条目验证平台归档，然后使用
   相应的 `*.binary.sha256` 文件验证解压后的可执行文件。两个校验和文件必须来自
   同一个不可变发布标签。请参阅[下载二进制文件](BINARIES.md)。
2. **源代码审计**：在匹配的源代码检出或带依赖源代码归档中阅读
   `src/secret.rs`，然后在该文件之外搜索 `\.expose\(\)`。如果安装了 `rg`，
   可使用以下简洁命令：
   ```
   $ rg -n '\.expose\(\)' src --glob '!secret.rs'
   ```
   否则，请使用您的平台批准的递归文本搜索工具；`rg` 不是构建或验证的先决条件。
   生产调用点会立即将暴露的 `&str` 交给驱动程序的连接构建器。MySQL 还会调用
   `.to_string()`，因为 `mysql_async` 的 API 要求 `String`。该副本不会归零，
   而是移入驱动程序持有的选项中；它可能在 `OptsBuilder` 被丢弃后继续存在，
   在选项／连接的生命周期内保留。丢弃构建器不能证明副本已被擦除。
   Tier 1 和 Tier 2 复用同一个 MySQL 连接。
   完整说明请参阅 SECURITY.md 中的**驱动程序持有的凭据副本**。
3. **从源代码构建**：`./build.sh`。使用带依赖源代码归档时，请运行
运行 `DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh`。 每个版本都会构建两次，如果字节不匹配，则该版本会失败。 仅当使用相同的源版本、目标、功能、固定的 Rust 工具链、链接器和构建选项时，本地比较才有意义。
4. **与发布版本比较**：在匹配的源代码检出或源代码树中运行
   `./verify.sh /path/to/extracted/dbwarp-blueprint`。所需目标、功能、工具链、链接器、
   源日期纪元和构建标志请参阅 BUILD.md 中的**重现发布二进制文件**。
5. **运行时跟踪**：在 Linux 沙箱中使用
   `strace -f -e trace=open,connect,read,write` 运行。如果 `strace` 或 `rg` 不可用，
   请使用您的平台批准的等效文件/网络跟踪和递归文本搜索工具。与上述列表进行比较。
6. **网络跟踪**：在主机上运行 `tcpdump`。在密码身份验证的实时运行中，验证数据库
   会话及预期的 DNS 流量。对于集成式身份验证，还应计入预期的 KDC/域控制器流量。
   在批处理模式中，核对每个数据库来源对应一个数据库会话。

如果任何内容与此处记录的内容不符，请通过 SECURITY.md 中的渠道报告差异，并提供重现问题的最小、最安全的追溯信息。请勿在公开问题中包含凭据、身份信息或敏感的驱动程序输出。
