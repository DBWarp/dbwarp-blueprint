# 安全模型

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../SECURITY.md)。本译文不应被视为合同级文本。

**语言：** [English](../../SECURITY.md) | [Deutsch](../de/SECURITY.md) | [Français](../fr/SECURITY.md) | [Español](../es/SECURITY.md) | [Polski](../pl/SECURITY.md) | [日本語](../ja/SECURITY.md) | **简体中文**

`dbwarp-blueprint` 分别提供实时数据库、结构化文件、批处理、捆绑包和演示文稿
模式。所选模式决定其网络和文件系统范围。它不存在遥测、更新检查、许可证检查、
分析调用或上传路径。

本页说明安全边界，以便您的团队决定是否运行该工具。

## 报告漏洞

请通过 [GitHub 私密漏洞报告](https://github.com/DBWarp/dbwarp-blueprint/security/advisories/new)
私下报告可疑漏洞。请勿在公开 issue 中包含安全敏感详细信息。请附上准确的发布版本、操作系统、重现步骤，
以及评估报告所需的最少安全证据。

## 网络

| 模式 | 运行时网络使用 |
|---|---|
| PostgreSQL、MySQL 或 SQL Server 的实时 `--connect` 数据库连接 | 与指定数据库端点建立一个数据库驱动程序会话。DNS 解析可能联系已配置的解析器。集成式 Kerberos/SSPI 身份验证还可能联系 KDC 或域控制器等已配置的身份基础设施。 |
| 需要明确确认的 Oracle 预览 | 仅将操作员选择的 `--oracle-sqlplus` 可执行文件作为子进程启动（如果可用，则包括一次有界的 `-V` 探测），然后用于目录会话。凭据通过标准输入发送，绝不放入进程参数。子进程会收到一个空的私有目录作为 `TNS_ADMIN`；环境会被清空，并且只转发现有的 `PATH`、`SystemRoot`、`WINDIR`、`ORACLE_HOME`、`LD_LIBRARY_PATH`、`DYLD_LIBRARY_PATH`、`LIBPATH` 和 `SHLIB_PATH`。固定的区域设置、时区和私有 `TNS_ADMIN` 值会另行设置。 |
| `--batch-manifest` | 对清单中的每个数据库来源建立一个数据库驱动程序会话，并依次处理。本地 Parquet 和 Avro 来源不使用网络。上述 DNS 和集成式身份验证条件仍然适用。 |
| `--from-toml`, `--from-parquet`, `--from-avro`, `--bundle-list`, `--bundle-extract`, `--bundle-pack` | 不建立由应用程序发起的网络连接。网络挂载文件系统上的输入仍属于操作系统或存储方面的考虑。 |

该工具不调用 DBWarp 服务或云 API。数据库驱动程序和主机操作系统可能产生上述
协议支持流量，并可能读取系统信任存储、DNS 配置、动态库以及集成身份验证配置或
凭据缓存。

`--max-wall-secs` 设置两项独立保护。PostgreSQL 使用会话本地 `statement_timeout`，
MySQL 对采集器的只读 `SELECT` 使用会话本地 `max_execution_time`。SQL Server 没有等效的会话设置来限制语句总经过时间，
因此采集器设置会话本地 `LOCK_TIMEOUT` 来限制锁等待，并保留客户端实际时间截止来处理其他停顿。
如果客户端截止时间到期，工具会断开连接；它不会声称 SQL Server 已确认服务器端取消。重试前请确认服务器工作已停止。

## 读取的文件

运行时，应用程序只直接读取命令行上选择的输入或批处理/捆绑包输入引用的文件：

| 文件 | 使用时机 |
|---|---|
| `--user-file` | 用户名来源 |
| `--password-file` | 密码来源 |
| `--anonymization-key-file` | 可选的 HMAC 密钥，由您持有，用于在经过批准的运行中，由二进制文件或 SQL 备用归一化器来保留匿名对象标签；模式必须防止 group/other 在 Unix 系统上的读取。 |
| `--azure-token-file` | SQL Server Entra ID 令牌来源 |
| `--tls-ca` | 受信任 CA 捆绑包 |
| `--tls-cert` | 客户端 TLS 证书 |
| `--tls-key` | 客户端 TLS 私钥 |
| `--from-toml` | 用于离线构建演示文稿的现有 dbwarp-blueprint TOML 文件 |
| `--from-parquet` | Parquet 文件元数据，以及在明确同意采样时读取有界数量的已解码行 |
| `--from-avro` | Avro 对象容器元数据和记录；必须遍历 Avro 才能统计记录数 |
| `--batch-manifest` | 批处理清单，以及它引用的每个本地结构化文件、凭据文件、令牌文件和 TLS 文件 |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | 捆绑包 TOML，以及所选操作需要的任何相对路径 Blueprint 文件 |
| 控制终端或控制台 | 交互式密码提示（类 Unix 系统上为 `/dev/tty`） |

应用程序没有读取 `~/.pgpass`、`~/.my.cnf`、云凭据文件、SSH 密钥、shell 历史记录或默认数据库密码环境变量的显式回退路径。平台数据库、TLS、DNS 和身份库仍可能读取其自身的系统配置和凭据缓存；当策略要求完整的进程和库清单时，请使用操作系统级跟踪。

对于 PostgreSQL 和 MySQL，提供的 `--tls-ca` PEM 捆绑包会替换编译进二进制文件的 Mozilla 根证书。省略 `--tls-ca` 时，SQL Server 使用操作系统信任存储区；提供的 `.pem` 或 `.crt` 文件必须只包含一个 CA 证书，并替换这些根证书。SQL Server 在两种证书验证模式下都会验证主机名；由于其驱动未实现客户端证书身份验证，工具会以 `DBP1015E` 拒绝 `--tls-cert`/`--tls-key`。

## 写入的文件

运行时，工具可能写入：

| 文件 | 使用时机 |
|---|---|
| `--out` | 实时数据库、结构化文件、捆绑包提取或捆绑包打包模式的 Blueprint 输出 |
| `--deck` | 可选的 PowerPoint (.pptx) 摘要；在本地从匿名化 Blueprint 或 `--from-toml` 输入生成（不额外读取数据库、不使用网络、不使用第三方库） |
| `--audit-log` | 可选的审计日志副本 |
| `--out-dir` | 批处理目录，其中包含 `bundle.toml`、`blueprints/*.blueprint.toml`、`audits/*.audit.txt`、所有权标记，并在一个或多个来源失败时包含 `errors.txt`；原子发布期间会使用同级暂存目录，并在已处理的失败后删除该目录 |

审计日志还会打印到 stderr。

请将每份审计日志和批处理 `errors.txt` 都视为受访问控制的运维证据。它们可能包含
端点名称、本地路径、清单来源 ID、驱动程序错误和计时数据。对于 SQL Server，
审计包含确切的已验证登录名 (`ORIGINAL_LOGIN()`)、有效服务器主体
(`SUSER_SNAME()`) 和数据库主体 (`USER_NAME()`)，以及可选的预期主体和断言结果。
这些身份不会写入单一来源 Blueprint 或演示文稿。捆绑包元数据会保留操作员提供的
来源 ID、标签和数据集组 ID，因此请使用匿名值，并在传输前审阅捆绑包 TOML。

## 环境变量

默认情况下，不读取任何运行时环境变量作为凭据。

如果传入 `--password-env NAME`、`--user-env NAME` 或 `--azure-token-env NAME`，工具只读取该指定变量。它不会回退到 `PGPASSWORD`、`MYSQL_PWD` 或 `MSSQL_PASSWORD` 等常见默认值。

## 凭据

凭据封装在 `Secret` 类型中，该类型有意不实现 `Debug`、`Display`、`Clone` 或序列化。这使得会意外记录凭据的代码难以通过编译。

凭据只在建立连接时交给数据库驱动程序。它们不会写入输出文件或审计日志。审计日志记录凭据来源，例如 `file:/etc/dbwarp/db.pass`，而不记录其值。

## 驱动程序持有的凭据副本

清零只涵盖 DBWarp Blueprint 持有的 `Secret` 缓冲区，无法保证数据库驱动程序、TLS 库、操作系统身份验证提供程序或分配器创建的副本也被擦除。当前 MySQL 驱动程序 API 要求拥有一个 `String`，因此 `src/engine_mysql.rs` 会明确地把密码从 `Secret` 复制到 `OptsBuilder`。该副本不会清零，并会一直存在到 builder/options 被丢弃。PostgreSQL、SQL Server 和平台身份验证库也可能在 wrapper 之外创建内部副本。对于使用敏感凭据的主机，应适当限制进程检查和交换空间。

## 被拒绝的凭据模式

工具拒绝内嵌于连接 URI 的密码。例如，不接受以下形式：

```text
postgresql://user:password@host/db
```

请改用 `--password-file`、`--password-env` 或交互式提示。这可避免密码泄露到 shell 历史记录、进程列表或终端回滚内容中。

## 输出安全

Blueprint 文件设计为可供人工阅读和审阅：

- 真实标识符会替换为 `table-001` 和 `col-1` 等密钥化匿名名称；
- 数值使用每个字段所记录的精确或舍入精度；精确长度模式需要明确同意；
- 注释固定，不作为数据通道；
- 永不输出行值；
- 启用压缩采样时，样本在本地压缩后丢弃。

实时 Tier 2 将每个表投影出的采样值负载预算限制为最多 16 MiB。
对于异常宽的表，它会减少请求的行数，并通过引擎原生的服务器端截断来投影可变宽度单元格，
MySQL 和 SQL Server 的自适应重试也遵守这一限制。样式探测在其 SQL 投影中另行设限。
本地探针编码器也独立强制执行表级上限。

这并不是把网络流量或进程内存限制在 16 MiB：协议封装、单独返回的原始长度元数据、
PostgreSQL 的二进制十六进制表示、重试以及驱动缓冲区都会增加开销。
非常大的值可能只有有界前缀参与压缩和值汇总测量。
采集器会在服务器端另行获取原始采样值的长度，并对这些原始长度应用所选的长度保真度策略，
而不是只对返回的前缀长度应用该策略。采样来源信息会记录适用的限制；
输出有界并不能证明已测量完整值。

表、模式、索引以及非表对象的排序使用领域分隔的 HMAC-SHA256 算法。 默认情况下，该工具从操作系统获取一个新鲜的、仅在当前进程中有效的密钥，并且永远不会将其输出，从而防止离线读取器检查候选源名称。 仅当相同的匿名标签需要在经过批准的比较运行中保持一致时，才使用 `--anonymization-key-file`。 该文件必须包含正好 32 个原始字节或 64 个十六进制字符，并且必须像凭据一样进行保护。 审计记录中会记录是否使用了临时密钥或您提供的密钥，但永远不会记录密钥值。

仅使用标准库的 SQL 回退 `blueprint_format.py` 规范化器采用相同的密钥化排序契约。除非提供 `--anonymization-key-file`，否则它会默认从操作系统获取新的随机密钥，并在规范文件头后添加包含生成器和密钥来源的固定注释，使其输出不会与 Rust 采集器输出混淆。

规范化前，SQL 回退的中间 JSON 包含真实模式、表、列和索引名称。MySQL `COLUMN_TYPE` 还可能包含已声明的 enum/set 成员。SQL 脚本没有模式子集选择器，也不会生成应用程序审计日志。请将中间 JSON 作为敏感模式材料保留在源环境中，在本地进行规范化，并只传输经过审阅的 TOML。只批准选定模式或需要完整审计、拓扑、对象或采样证据时，请使用 Rust 采集器。

这会降低披露风险，但不能保证每项输出对每位接收者都安全。匿名化模式形状、依赖关系
图、引擎版本、明确选择加入的字段和异常大小分布都可能形成工作负载指纹。共享前，
请根据组织的数据分类政策审阅 Blueprint 和捆绑包输出。不要把审计或 `errors.txt`
当作匿名化 Blueprint 发送。

确切字段请参阅 [`FORMAT.md`](FORMAT.md)。

## 审计日志

每次运行都会输出一份审计日志，其中列出：

- 联系的数据库端点；
- 使用的凭据来源；
- 会话能够报告时的 SQL Server 已验证主体、有效服务器主体和数据库主体；
- TLS 模式；
- 读取的文件；
- 写入的文件；
- 执行的查询；
- 是否启用行采样；
- 最终结果。

请参阅 [`AUDIT.md`](AUDIT.md)。

## 源代码审查起点

若要进行重点审查：

- `src/secret.rs`：凭据包装器；
- `src/main.rs`：CLI、同意门控和审计输出；
- `src/audit.rs`：审计日志呈现；
- `src/format.rs`：匿名化输出格式；
- `src/tls.rs`：TLS 配置；
- `src/engine_pg.rs`、`src/engine_mysql.rs`、`src/engine_mssql.rs`：各数据库专用目录读取器。
