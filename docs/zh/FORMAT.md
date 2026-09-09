# DBWarp Blueprint 文件格式 v6

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../FORMAT.md)。本译文不应被视为合同级文本。

**语言：** [English](../../FORMAT.md) | [Deutsch](../de/FORMAT.md) | [Français](../fr/FORMAT.md) | [Español](../es/FORMAT.md) | [Polski](../pl/FORMAT.md) | [日本語](../ja/FORMAT.md) | **简体中文**

人类可读。可比较差异。可进行取证审查。

> **此格式通过有界模式、基于秘密密钥的标识符和已记录的数值精度，降低隐蔽信道和直接披露
> 风险。匿名图结构和明确选择加入的精确字段仍可能形成工作负载指纹，因此请根据您
> 自己的数据分类政策审阅该文件。**

## 文件头

逐字节完全如下：

```
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

空行是规范文件头的一部分。Rust 采集器只输出该文件头，不输出其他注释。SQL 回退
规范化器会逐字保留该文件头，然后添加一条固定的
`Producer: blueprint_format.py SQL fallback` 注释并注明密钥来源，使接收方能够区分
生成器。但这并不表示其余结构化字段无法识别具有独特特征的模式或依赖关系图。

## 顶层字段

| 字段 | 类型 | 说明 |
|---|---|---|
| `schema_version` | int | 格式版本。目前为 `6`；版本 1 到 5 仍可读取。 |
| `generated_at` | ISO-8601 string | UTC 时间戳，秒级精度，无小数部分。可通过 CLI 标志 `--generated-at "2026-04-26T00:00:00Z"` **固定**。字节完全相同的实时采集还需要相同的受保护 `--anonymization-key-file`、源状态、选项和生成器。每当设置该标志时，审计日志都会记录 `generated_at_pin: ...`，使该固定操作在取证时可见。没有环境变量可以固定此值。 |
| `engine` | string | `"postgresql"`、`"mysql"`、`"sqlserver"`、`"parquet"` 或 `"avro"`。 |
| `engine_version` | string | 用于按版本感知实时数据库采集和生成的精简数字产品版本；结构化文件源为空。不包含生成器或发行版横幅。 |
| `source_kind` | string | `"production"`、`"staging"`、`"scrubbed-replica"`、`"synthetic"` 之一。由客户声明。 |
| `length_metadata` | string | 旧版兼容性标记：`"hybrid-v2"`、`"exact"`、`"rounded"` 或 `"not-captured"`。新消费者必须使用下面三个字段。 |
| `declared_length_fidelity` | string | PostgreSQL 声明字符容量以及默认 balanced/exact MySQL 模式为 `"exact"`；strict MySQL privacy 为 `"coarse-rounded-v1"`；不可用时为 `"not-captured"`。 |
| `index_length_fidelity` | string | 默认 balanced/exact MySQL 索引前缀为 `"exact"`；strict privacy 为 `"rounded-down-v1"`；不可用时为 `"not-captured"`。 |
| `observed_length_fidelity` | string | 已采样时默认为 `"relative-rounded-v2"`，exact 模式为 `"exact"`，strict 模式为 `"coarse-rounded-v1"`，未采样时为 `"not-sampled"`。采样覆盖率仍是单独的逐列要求。 |
| `[totals]` | inline table | 聚合计数（见下文）。 |
| `[network]` | table | 可选的客户端到数据库连接及查询 RTT 证据。 |
| `[database_topology]` | table | schema v6 数据库源必需。有界且无名称的部署、本地角色、可见性和目录证据。结构化文件不含此块。 |
| `[dataset_scope]` | table | 每个 schema v6 Blueprint 都必需。声明总计覆盖的范围，以及表、行和字节覆盖是否完整。 |
| `[tables.X]` | tables | 每个表一个，使用匿名化 ID。 |
| `[fk_edges]` | inline table | 匿名化表之间的 FK 图。可选。 |
| `[artifact_inventory]` | table | 有界且无名称的非表对象计数、可选匿名依赖图、外部前提条件和可选有界语言统计。仅适用于数据库源。 |

## `[totals]`

| 字段 | 类型 | 精度 |
|---|---|---|
| `table_count` | int | 精确 |
| `row_count` | int | 各表舍入后 `rows` 的总和 |
| `table_bytes` | int | 各表舍入后 `table_bytes` 的总和 |
| `index_bytes` | int | 各表舍入后 `index_bytes` 的总和 |

这些数字不会自动成为整个集群的总计。必须始终结合 `[dataset_scope]` 解读。
分片网关或协调器可能呈现看似完整的目录，却不持有底层分片。schema v6 会明确表示
这种不确定性，而不会默默地把本地目录统计当作全局事实。

## `[database_topology]`（schema v6 数据库源）

此块只记录通过已连接数据库端点可见的有界事实。它绝不存储节点名、主机名、IP 地址、
集群名、复制通道名、服务器标识符或端点。

| 字段 | 值 / 规则 |
|---|---|
| `contract` | 始终为 `dbwarp-blueprint-topology/v1`。 |
| `deployment` | `single-node`、`replicated`、`sharded`、`distributed` 或 `unknown`。 |
| `local_role` | `standalone`、`primary`、`secondary`、`coordinator`、`worker`、`member` 或 `unknown`。 |
| `visibility` | `full`、`partial` 或 `unknown`；描述拓扑证据，而非数据正确性。 |
| `member_count` | 通过成功证据查询可见的成员数量。`0` 表示未知，绝不表示没有成员。 |
| `identifiers_redacted` | 必须为 `true`。 |
| `role_counts` | 按封闭角色 token 统计的可选数量。完整可见性要求其总和等于 `member_count`。 |
| `features` | 排序后的封闭 token，例如 `citus`、`mysql-group-replication`、`mysql-galera`、`mysql-ndb`、`postgresql-streaming-replication`、`sqlserver-availability-group` 或 `vitess`。 |
| `catalogs_read` | 已成功读取的拓扑目录的排序封闭标签。 |
| `catalogs_unreadable` | 无法读取的拓扑目录的排序封闭标签。任何条目都会阻止完整可见性声明。 |

普通端点可以合理地报告 `deployment = "unknown"`，同时仍提供完整的本地全副本
表统计。Blueprint 不会仅仅因为未看到集群功能，就推断普通服务器是
`single-node`。

## `[dataset_scope]`（schema v6）

此块分别限定每个容量总计。只要任一必需的完整性维度为 `incomplete` 或
`unknown`，消费者就必须拒绝未经限定的全数据集运算。

| 字段 | 值 / 规则 |
|---|---|
| `contract` | 始终为 `dbwarp-blueprint-dataset-scope/v1`。 |
| `layout` | `full-copy`、`sharded`、`distributed`、`structured-dataset` 或 `unknown`。 |
| `table_inventory_completeness` | `complete`、`incomplete` 或 `unknown`。 |
| `row_count_completeness` | `complete`、`incomplete` 或 `unknown`。 |
| `size_completeness` | `complete`、`incomplete` 或 `unknown`。 |
| `row_count_method` | 封闭来源 token，例如 `postgres-planner-estimate`、`mysql-table-statistics`、`sqlserver-partition-counter` 或 `distributed-aggregate`。 |
| `size_method` | 封闭来源 token，例如 `postgres-local-relation-size`、`mysql-information-schema`、`sqlserver-partition-pages`、`citus-distributed-relation-size` 或 `distributed-aggregate`。 |
| `limitations` | 对覆盖不完整或未知原因进行排序后的封闭说明。除非所有维度都完整，否则至少需要一个。 |

`selection-limited` 表示总计和完整性声明仅覆盖通过可重复实时 `--schema` 选择器请求的 schema，并不声明覆盖整个已连接数据库。省略 `--schema` 时，会保留采集所有可见 schema 的行为。

原生 PostgreSQL、MySQL 和 SQL Server 收集器会先探测受支持的拓扑目录，再判断本地
统计能否代表逻辑数据集。已知的分布式网关在无法获得可靠聚合时会抑制不安全的总计。
SQL 回退格式化器没有拓扑探测，因此会保留有用的本地估算，但将所有范围维度标记为
`unknown`，并添加 `topology-unobserved` 和
`topology-visibility-unknown` 限制。

结构化 Parquet 和 Avro Blueprint 省略 `[database_topology]`，并使用
`layout = "structured-dataset"` 和 footer/container 来源信息。

Blueprint 在普通采集期间不会运行存储速度基准，也不会根据运行客户端的机器推断数据库
服务器硬件。数据库字节总计只描述指定目录方法得到的存储数据量；不声称磁盘类型、IOPS、
吞吐量、CPU、RAM 或目标迁移性能。

## `[network]`（可选）

从 Blueprint 工具到源数据库的客户侧实测网络往返统计信息。**不是**迁移源与目标之间的 RTT，这只是 Blueprint 工具在运行时与客户源数据库距离的证据。下游估算器仅将其用于对运维人员提供的迁移 RTT 进行合理性检查（例如，如果客户的本地探测值为 0.4 ms，而运维人员声称迁移 RTT 为 200 ms，则该说法不可信，Blueprint 工具可能运行在源数据库本身）。

探测在连接建立后、目录查询前运行，因此时间不会受到查询缓存预热的影响。它执行 **5× `SELECT 1`** 并输出延迟中位数。每次 `SELECT 1` 都返回常量整数 1，此探测绝不会读取任何行数据。

如果客户传递了 `--no-rtt-probe`，或探测本身在运行中失败，则此块不存在（会作为非致命警告记录到 stderr 和审计日志；Blueprint 文件仍会在不包含该块的情况下输出）。

| 字段 | 类型 | 精度 |
|---|---|---|
| `sample_count` | int | 精确（v1 中始终为 5） |
| `connect_total_ms` | int | 从 TCP 连接开始到经过身份验证的会话就绪的总墙钟时间，以毫秒计。包括 TCP 握手 + TLS 握手（如适用）+ 身份验证质询/响应。舍入到最接近的毫秒。通常为 `query_rtt_ms_p50` 的 3–6 倍。 |
| `query_rtt_ms_p50` | int | 5 个 `SELECT 1` 样本的单次往返延迟中位数，以毫秒计。舍入到最接近的毫秒。自然网络噪声下限（实践中 ≥ 1 ms）大于舍入粒度，因此既可消除任何低位隐蔽信道，又不会损失有用精度。低于毫秒的 LAN 值会归并为 0 或 1。 |
| `query_rtt_ms_p95` | int | 5 个样本采用最邻近秩法计算的第 95 百分位数（即最慢的观测值），以毫秒为单位，并舍入到最接近的毫秒。请与 p50 结合使用，以识别短暂的延迟峰值；5 个样本仅供粗略判断，不能视为工作负载基准测试。 |

这 5 个探测查询在审计日志中显示为标有 `5x SELECT 1 (RTT probe; constant integer 1, no row data)` 的**单个摘要条目**（而非 5 个单独行），符合不读取任何行内容的信任定位。

## `[tables.<id>]`

标识符为 `table-NNN`，其中 `NNN` 是对模式名和表名进行域分离 HMAC-SHA256 排序后、从 1 开始的序号。
默认密钥每个进程重新生成，且永不输出。传入相同的客户保管 `--anonymization-key-file` 可在经批准的对比运行之间保留该排序。

| 字段 | 类型 | 精度/值 |
|---|---|---|
| `rows` | int | 舍入：≤10k 时最接近 100，≤1M 时最接近 1000，>1M 时最接近 10000 |
| `table_bytes` | int | 按量级舍入到最接近的 1KiB / 1MiB / 100MiB |
| `index_bytes` | int | 舍入方式与 `table_bytes` 相同 |
| `schema` | string | 匿名化 ID `schema-A`、`schema-B`、...、`schema-AA` |
| `kind` | string | Schema v6 可选封闭标记：`partitioned`、`materialized-view`、`temporal-current`、`temporal-history`、`memory-optimized`、`external`、`graph-node` 或 `graph-edge`。普通表或证据未知时省略。 |
| `unlogged` | bool | Schema v6 可选 PostgreSQL 目录观测。未采集时省略；显式 `false` 表示目录已确认该表有日志。 |
| `partition_strategy` | string | Schema v6 中 `partitioned` 的可选标记：`range`、`list`、`hash`、`key` 或 `linear-hash`。 |
| `partition_count` | int | Schema v6 精确的正叶分区数；`kind = "partitioned"` 时必需。 |
| `partition_key_cols` | array of int | Schema v6 简单分区键的列序号。表达式键或目录证据不可用时省略；绝不序列化键表达式。 |
| `partition_rows_max` | int | Schema v6 可选的最大叶分区行数舍入估计。 |
| `temporal_history` | string | Schema v6 配对 `temporal-history` 表的 ID；`temporal-current` 必需。 |
| `counted_in_totals` | bool | Schema v6。省略表示计入全部汇总。`external` 必须显式为 `false`，从 `table_count`、`row_count`、`table_bytes` 和 `index_bytes` 中排除；其他显式值均非规范。 |
| `check_count` | int | Schema v6 可选的精确结构 CHECK 约束数。省略表示未知；`0` 表示相关目录确认没有约束。 |
| `has_clustered_index` | bool | PostgreSQL 始终为 `false` |
| `stats_freshness` | string | PostgreSQL 使用 `"fresh"` / `"stale"` / `"never_analyzed"`，根据 `pg_stat_all_tables` 中的分析时间戳得出；对于无法在不扩大访问权限的情况下确定统计信息新鲜度的 MySQL、SQL Server、结构化文件和 SQL 回退路径，该字段为空。刻意不使用 MySQL `UPDATE_TIME`，因为它描述的是源数据修改，而不是 `ANALYZE TABLE` 的执行时间距今多久。 |
| `[tables.<id>.cols.<cid>]` | sub-tables | 每列一个 |
| `[tables.<id>.idxs.<iid>]` | sub-tables | 每个索引一个 |
| `[tables.<id>.compression]` | sub-table | 仅 Tier 2 |

## `[tables.<id>.cols.<cid>]`

标识符为 `col-N`，其中 `N` 是列的自然属性顺序（从 1 开始，保留磁盘上的序号）。在多次运行之间保持稳定。

| 字段 | 类型 | 说明 |
|---|---|---|
| `ordinal` | int | 与 ID 相同的 N |
| `type` | string | 标准化类型系列，例如 `"integer"`、`"numeric(12,2)"`、`"text"`、`"json"`、`"binary"`、`"timestamp"`、`"uuid"`、`"array<integer>"` 或 `"user-defined"`。不会输出真实的 domain、enum、alias、composite 和用户定义类型名称。 |
| `nullable` | bool | |
| `value_source` | string | Schema v6 可选封闭标记：`identity-always`、`identity-default`、`auto-increment`、`identity`、`sequence-default`、`generated-stored`、`generated-virtual`、`computed-persisted`、`computed-virtual`、`system-time` 或 `rowversion`。普通输入值或证据未知时省略。 |
| `has_default` | bool | Schema v6 可选目录观测。省略表示未知；显式 `false` 表示目录确认没有默认值。 |
| `default_kind` | string | Schema v6 可选分类 `constant`、`function` 或 `expression`，仅在 `has_default = true` 时有效。绝不序列化默认值文本或字面量。 |
| `type_kind` | string | Schema v6 可选封闭标记：`enum`、`set`、`domain`、`composite`、`array`、`range` 或 `alias`。基础类型或证据未知时省略。 |
| `member_count` | int | Schema v6 精确的正结构成员数，仅 `enum` 和 `set` 必需；绝不序列化成员名。 |
| `domain_has_check` | bool | Schema v6 可选 domain CHECK 观测，仅 `type_kind = "domain"` 时有效。 |
| `hidden`, `masked`, `encrypted`, `sparse` | bool | Schema v6 可选目录观测。省略表示未知；显式 `false` 表示目录确认没有该属性。 |
| `has_check` | bool | Schema v6 可选单列 CHECK 观测。每个显式 `true` 都包含在表的 `check_count` 中。 |
| `null_fraction` | float | 可选的观测空值比例，范围为 `0.0` 到 `1.0`。仅保留舍入后的聚合值，不保留空值位图。 |
| `native_type` | string | 可选的净化后引擎基础类型，例如 `varchar` 或 `longtext`；不含标识符、enum 成员、默认值或表达式。由原生 MySQL 和 SQL Server 采集器输出。 |
| `declared_max_chars` | int | 可选的声明字符容量。PostgreSQL `character`/`character varying` 目录值以及默认 balanced/exact MySQL 模式下都精确；仅对 MySQL 使用 `--length-fidelity strict` 时粗略舍入。 |
| `declared_max_bytes` | int | 可选的声明字节容量。在默认 balanced/exact MySQL 模式下精确；仅使用 `--length-fidelity strict` 时粗略舍入。 |
| `numeric_precision`、`numeric_scale`、`datetime_precision` | int | 可选的引擎声明标量精度。 |
| `charset`、`collation` | string | 可选的净化后字符元数据。MySQL 输出其目录字符集和排序规则名称。SQL Server 对 `nchar`/`nvarchar`/`ntext` 输出 `utf-16le`，对代码页 65001 输出 `utf-8`，对 Windows 代码页 1250–1258 输出 `windows-N`，对其他正数目录代码页输出 `code-page-N`，并输出目录排序规则名称。这些是编码事实和目录名称，绝不是客户标识符或值。 |
| `len_avg` | int | 可变长度值的采样平均字节数。默认相对分桶的最大误差约为 3.2%，并精确保留不超过 32 字节的值；使用 `--length-fidelity exact --yes` 时精确；仅在 strict 模式下粗略舍入到最接近的 10。0 = 固定长度或未测量。 |
| `len_p95` | int | 使用相同默认相对分桶的采样第 95 百分位数；使用 `--length-fidelity exact --yes` 时精确；仅在 strict 模式下粗略舍入到最接近的 100。0 = 未测量。 |
| `style` | string | 仅 Tier 2。`"json"`、`"xml"`、`"natural-text"`、`"base64"`、`"hex"`、`"numeric-text"`、`"mixed"` 或 `"precompressed"` 之一；未分类时为空。只有带有可识别标准容器签名、且在字节量上占显著主导地位的二进制值样本才会输出 `"precompressed"`。它刻意不披露检测到的容器系列。 |
| `magnitude_min`, `magnitude_max` | int | Schema v6 可选有符号十进制指数，用于界定采样非 NULL 数值的数量级。与 `has_negative` 一同输出；绝不序列化精确值。 |
| `has_negative` | bool | Schema v6 可选符号观测，仅与两个数量级边界一同输出。 |
| `time_span` | string | Schema v6 可选采样日期/时间范围：`intraday`、`days`、`weeks`、`months`、`years` 或 `decades`。 |
| `time_recent_decade` | int | Schema v6 最新采样日期/时间所在十年，仅与 `time_span` 一同输出，且始终能被 10 整除。 |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | 仅 Tier 2。为已采样的文本/二进制候选列提供。字段布局与表级压缩相同，但范围限定为一个匿名化列。 |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Schema v3 采样值分布摘要。仅包含有界或舍入后的计数与频率。 |

### `[tables.<id>.cols.<cid>.cardinality]`（schema v3）

启用行采样时，采集器会在内存中为每列最多保留 8,192 个临时 64 位指纹，据此得出 NDV 和偏斜度聚合统计，然后丢弃这些指纹。值和指纹都不会被序列化。该块包含 `measured`、`sample_rows`、`non_null_rows`、`observed_distinct_count`、`estimated_distinct_count`、`top_value_fraction`、`frequency_p50`、`frequency_p95`、`frequency_p99`、`frequency_max`、`sample_method`、`sample_layout`、`sampled_with_bias` 和 `bias_reason`。`sample_layout` 是可选的机器可读枚举。当前输出值是 `primary-key-range-windows`；缺少该字段表示没有可用的排序契约。消费者不得通过解析面向人工的 `sample_method` 字段来推断生成语义。

计数和比例会在适当情况下按隐私要求舍入。这些统计信息用于在合成测试数据集中重现
重复密度、热点值偏斜和有限域。它们不包含采样值，但独特的分布可能形成工作负载
指纹；请勿将其视为不可逆，也不能据此证明无法通过外部知识推断业务含义。

### `[tables.<id>.cols.<cid>.compression]`（仅 Tier 2）

仅当使用 `--measure-compression --yes` 时，才会为有界文本/二进制候选列输出逐列压缩。这样，下游工具无需查看客户值，也能生成比仅使用表级比率更接近真实熵的合成文本/二进制数据。

该块具有与 `[tables.<id>.compression]` 相同的字段：`measured`、`sample_rows`、`sample_bytes`、`sample_method`、`sampled_with_bias`、`bias_reason`、`ratio_zstd_3`、`ratio_zstd_19`、`ratio_stddev` 和 `sample_encoding`。

示例：

```toml
[tables.table-001.cols.col-2]
ordinal = 2
type = "json"
nullable = false
len_avg = 430
len_p95 = 0
style = "json"

[tables.table-001.cols.col-2.compression]
measured = true
sample_rows = 1000
sample_bytes = 65536
sample_method = "column TABLESAMPLE SYSTEM(0.1) LIMIT N (text format)"
sampled_with_bias = false
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "blueprint-compression-probe-v2"
```

Blueprint 文件中不会写入任何采样列值。

对于二进制列，同一有界 Tier 2 样本还可能输出粗粒度的
`style = "precompressed"` 特征。识别只在采样值边界上进行，并且要求该观察在
字节量上占显著主导地位。Blueprint 不会解析或解压该值，不会保留其签名，也不会在
这一高置信度标签之外区分图像、归档、压缩媒体、加密或随机载荷。文本和 base64
编码仍按文本样式分类，不会被当作预压缩二进制容器。

确定性生成器会把该粗粒度特征映射为一个中立、有效的 ZIP 容器，其中包含以存储方式
保存的高熵成员。这样可保留实际压缩行为，同时不会声称源数据包含 ZIP 文件，也不会
透露所识别的源值是图像、归档还是媒体容器。普通二进制生成保持不变。

## `[tables.<id>.idxs.<iid>]`

标识符为 `idx-N`，其中 `N` 是表内索引按对索引名进行域分离 HMAC-SHA256 排序后、从 1 开始的序号。

| 字段 | 类型 | 值 |
|---|---|---|
| `type` | string | 标准化索引方法系列，例如 `"btree"`、`"hash"`、`"gin"`、`"gist"`、`"brin"`、`"spgist"`、`"fulltext"`、`"spatial"`、`"clustered"`、`"nonclustered"`、`"clustered columnstore"`、`"nonclustered columnstore"` 或 `"other"`。不会输出扩展/自定义方法名称。 |
| `primary` | bool | 可选；主键索引输出为 `true`。否则省略/为 false。 |
| `unique` | bool | |
| `cols` | array of int | 按索引列顺序参与索引的列序号 |
| `prefix_lengths` | array of int | 可选的 MySQL 索引前缀长度，与 `cols` 对齐；零表示完整列。默认精确；仅使用 `--length-fidelity strict` 时向下舍入。 |
| `include_cols` | array of int | 可选；源引擎公开的非键 INCLUDE 列序号。 |
| `expression` | bool | 可选；存在无法表示为简单列序号的表达式/函数键材料时为 true。 |
| `filtered` | bool | 可选；对于 filtered/partial 索引为 true。 |
| `descending` | bool | 可选；任何键列显式降序时为 true。 |
| `prefix_distinct_counts` | array of int | Schema v3 对从一列到 N 列的每个键前缀估算的不同元组数。零表示该前缀不可用。 |
| `cardinality_sample_method` | string | `prefix_distinct_counts` 的有界来源；推断得到的乘积会被明确标记，不会作为直接元组样本呈现。 |

## `[tables.<id>.compression]` 和 `[tables.<id>.cols.<cid>.compression]`（仅 Tier 2）

仅当文件使用 `--measure-compression --yes` 生成时才存在。表级块测量完整样本的中立列式投影，并且仍是全表传输估算的权威比率。列级块从相同采样行逐列投影而来，用于帮助下游合成测试数据集生成器在不查看客户值的情况下调整每列熵。它们不会触发额外的数据库读取。

| 字段 | 类型 | 精度 |
|---|---|---|
| `measured` | bool | 如果块存在，则始终为 `true` |
| `sample_rows` | int | 精确 |
| `sample_bytes` | int | 内存中样本缓冲区的大小，经过**分桶**：低于 1 MiB 时舍入到最接近的 **64 KiB**，低于 1 GiB 时舍入到最接近的 **1 MiB**，高于该值时舍入到最接近的 **100 MiB**。字节绝不会写入磁盘。分桶消除了精确 `buf.len()` 原本会暴露的逐表低位隐蔽信道。 |
| `sample_method` | string | 引擎特定的有界采样说明，例如 `"TABLESAMPLE SYSTEM(0.1) LIMIT N"`、`"LIMIT N (fallback after empty TABLESAMPLE)"` 或 `"SELECT TOP N"` |
| `sampled_with_bias` | bool | 如果样本不均匀（例如仅使用 LIMIT 的回退），则为 true |
| `bias_reason` | string | 如果 `sampled_with_bias = false` 则为空，否则为类似 `"unordered_limit_after_empty_TABLESAMPLE"` 的标签 |
| `ratio_zstd_3` | float | 按契约的 zstd 级别 3 测量策略舍入到最接近的 **0.05**。在通过 `sample_encoding` 编码的字节上测量。 |
| `ratio_zstd_19` | float | 从旧捕获中接受的遗留 zstd 级别 19 比率；工具不再测量或输出它 |
| `ratio_stddev` | float | 舍入到最接近的 **0.05**，有界表探针帧的级别 3 比率标准差。列级投影块目前输出 `0.0`，因为它们是建议性熵提示，而不是方差模型。 |
| `sample_encoding` | string | 测量所使用的字节级编码和压缩会话策略标识符。PostgreSQL 实时表块当前使用 `"blueprint-columnar-transfer-probe-v2"`。MySQL 和 SQL Server 使用 `"blueprint-columnar-transfer-probe-v3"`，它还会在 256 KiB 探针分块边界刷新。SQL Server 的 `nvarchar`/`nchar`/`ntext` 载荷保留原生 UTF-16LE 字节分布；`varchar`/`char`/`text` 保留采样字节宽度，`charset` 字段标识消费者所需的目录代码页。V1 仅作为输入兼容性契约保留。逐列块使用 `"blueprint-compression-probe-v2"`。dbwarp 估算器在使用该比率前**必须**验证此字符串，因为不同编码或会话策略**不可**互换。旧版 Blueprint 文件可能不包含此字段；只有在存在且能够识别编码标签时，估算器才应使用实测比率。 |

构建合成测试数据集时，dbwarp 估算器应优先选择可识别的逐列压缩块，然后回退到表级压缩，最后回退到类型/样式默认值。

### `blueprint-compression-probe-v2` 字节级编码

Tier 2 采样器使用此格式将行或采样列值连接到内存缓冲区，然后对其运行 zstd 级别 3。
缓冲区会被丢弃。Blueprint 只保留已记录的聚合压缩、空值密度、基数/频率、长度和
样式字段。

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

类型标签是探针契约的一部分；若没有新的版本化探针标识符，就不会重新编号。

| 标签 | 名称 | 用途 |
|---|---|---|
| 0x00 | Null | SQL NULL（无长度、无有效负载） |
| 0x01 | TextUtf8 | UTF-8 文本 |
| 0x02 | TextUtf16Le | UTF-16LE 字节，主要用于 SQL Server `nvarchar`/`nchar`/`ntext` |
| 0x03 | TextOther | 另一字符集中的字节 |
| 0x04 | NumberText | 数值的十进制文本表示形式 |
| 0x05 | BoolText | 文本形式的布尔值 |
| 0x06 | TimestampText | ISO-8601 时间戳文本 |
| 0x07 | DateText | ISO-8601 日期文本 |
| 0x08 | TimeText | `HH:MM:SS[.fff]` 文本 |
| 0x09 | UuidText | 规范的 36 字符 UUID 文本 |
| 0x0F | JsonText | JSON UTF-8 |
| 0x10 | BinaryRaw | `bytea`、`varbinary`、`image` 或 blob 字节 |
| 0xFE | UnknownText | 数据库提供的后备文本表示形式 |

### `blueprint-columnar-transfer-probe-v1`、`v2` 和 `v3` 字节级编码

实时数据库表比率会把相同的有界逐列 v2 样本转换为中立的 1,000 行帧。每个帧都有
版本化探针头；对于每一列，还包括序号、一个类型标签、每行四字节长度，以及随后列连续
排列的载荷字节。长度 `0xffffffff` 表示 NULL。三个版本共享同一字节表示。V1 将拼接的
帧序列作为一次已声明输入大小的 zstd 级别 3 操作进行压缩。V2 通过一个持久 zstd
级别 3 上下文传入各帧，并在每个帧之后刷新。V3 保留该上下文和中立行组表示，但还会在
行组内每个 256 KiB 探针压缩分块边界刷新。MySQL 和 SQL Server 采集使用 v3；
v2 仍是当前 PostgreSQL 测量方式。SQL Server Unicode 文本按 UTF-16LE 测量。
SQL Server 窄文本保留源字节宽度，
并记录由排序规则代码页推导出的封闭、净化后字符集。外层行组输出提供
`ratio_stddev` 观察值。版本化标签防止把一种成帧或刷新策略默默解释为另一种。

该表示建模列式批量传输中与压缩相关的通用属性。它不是数据库协议采集、迁移线路格式或
编码数据导出。样本字节仅保留在内存中，并在得出聚合测量值后丢弃。

### 准确度界限

`ratio_zstd_3` 描述指定的 `sample_encoding`；它不是对数据库协议或迁移传输字节的测量。公开自动化测试套件会验证确定性编码、有界采样和序列化，但不会声称对所有引擎和提取路径都具有通用的百分比误差。

在将该比率用于重要容量决策之前，请使用具有代表性的源数据和预期提取机制，对当前二进制文件和引擎版本进行验证。请将比较方法、样本大小、二进制哈希、引擎版本和观测误差与生成的计划一起记录。原始关系是：在已记录 `sample_encoding` 产生的字节分布下，`compressed_bytes ≈ sample_bytes / ratio_zstd_3`。

## `[fk_edges]`

可选的内联表，其中每个键都是一个映射到边列表的 `table-NNN` ID。Schema v3 会保留父列序号、引用操作、匹配模式、可延迟性、验证/信任状态，以及可选的有界、无名称关系摘要。边先按目标排序，再按列列表排序。

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

可选的 `statistics` 块会记录采样或推断得到的 `non_null_rows`、`distinct_parent_values`、`parent_coverage_fraction`、扇出 p50/p95/p99/max 和 `orphan_rows`，以及来源和偏差字段。经过验证的源约束意味着孤立记录数为零。由逐列样本得出的复合估算会明确标记为推断值。生成器使用这些聚合值重现空值覆盖率和扇出，同时将每个复合子键映射到一个一致的合成父元组。

## `[artifact_inventory]`（自模式 v4 起，数据库源）

独立版本化的 `dbwarp-blueprint-artifacts/v1` 契约描述非表对象，不会序列化源名称
或定义。结构化文件源以及选择 `--artifact-detail none` 时不包含此项。

默认的 `--artifact-detail summary` 输出 `object_count`、
`external_prerequisite_count`、`counts_by_kind` 和
`counts_by_external_class`。`graph` 还为每个对象输出匿名对象记录和依赖边。
`analyzed` 添加从可用定义临时派生的有界
`dbwarp-language-feature-census/v1` 记录。图拓扑可能识别应用程序，因此
`graph` 和 `analyzed` 都明确需要 `--yes`。

清单级证据包括：

| 字段 | 值 / 规则 |
|---|---|
| `detail` | `none`、`summary`、`graph` 或 `analyzed` |
| `visibility` | `full`、`privilege_filtered` 或 `unknown` |
| `inventory_complete` | 仅在可见性完整、没有不可读目录且没有已声明的未建模类别时才可为 true |
| `dependencies_complete` | 仅在已建模依赖目录可读时才可为 true |
| `analysis_complete` | 仅对 analyzed 详细级别且每项输出分析都完整时才可为 true |
| `catalogs_read` | 已成功检查的标准引擎目录的封闭标签 |
| `catalogs_unreadable` | 读取失败的目录标签；任何条目都会阻止完整性声明 |
| `families_not_inventoried` | 当前采集器契约之外的已知对象类别 |

每个对象 ID 的形式为 `<kind>-NNN`，例如 `view-001` 或 `function-002`。
记录仅包含封闭的 kind/subkind/tier 标记、匿名模式/父对象 ID、匿名依赖关系、
未解析依赖计数、有界定义可见性/安全模式、可选外部前提条件以及可选语言统计。
源对象名称、SQL 文本、主体、端点、凭据、密钥、证书和二进制文件都不是契约字段。

外部前提条件记录一个封闭的 `class`、部署范围、是否需要但未采集二进制/
秘密/端点材料，以及一个有界兼容性类别。其计数是迁移规划证据，并不表示
DBWarp 能自动配置或转换它们。

语言统计记录使用 `analyzer_version = "lexical-v1"` 和
`status = "partial"`。计数、大小、嵌套、复杂度和不透明区域值都是区间，
不是精确的源指纹。特征从封闭词汇中选择。分析器会删除注释、字面量和
带引号标识符；它不是解析器、语义绑定器，也不保证转换成功。

操作指南和引擎覆盖范围参见[非表对象清单](ARTIFACT_INVENTORY.md)。

## 按向量划分的隐写防护

| 向量 | 防护方式 |
|---|---|
| 标识符顺序 | 使用秘密进程本地密钥的域分离 HMAC-SHA256 可防止离线检查候选名称。仅在需要稳定的跨运行标签时才重用客户保管的密钥。 |
| 数值低位 | 默认将统计信息舍入到有文档记录的精度。精确长度模式必须显式启用、经同意门控并记录到审计日志中，而且必须作为更敏感的元数据处理。 |
| 亚秒级时间戳 | 顶部只有一个 UTC 时间戳，且仅有秒级精度 |
| TOML 格式 | 规范化：键按字母排序、固定缩进，只允许固定的文件头/生成器注释；不允许输入派生注释 |
| 采样随机性 | 采样使用固定种子（PG 的确定性 `TABLESAMPLE SYSTEM`）。另外，除非客户提供密钥，标识符匿名化会有意从操作系统 CSPRNG 获取秘密密钥。 |
| 未使用字段 | 每个字段均在上文记录；不存在承载无界数据的“metadata”/“comment”/“reserved”字段 |
| 对象源文本和外部材料 | 定义是临时的，并在有界分析后清零；名称、SQL 文本、端点、提供商字符串、凭据、密钥、证书、包名称和二进制文件都没有可序列化字段 |

## 模式版本兼容性

当前生产者输出模式版本 6。为保持向后兼容，版本 1 到 5 仍可读取。v1/v2 文件
不含分布块，因此生成器会对类型、宽度和均匀关系使用确定性回退，并报告保真度
损失。v3 文件包含分布元数据，但不包含对象清单。v4 文件可能包含对象清单，但
早于当前 Blueprint 契约标识符。读取端会在输入时规范化旧版 v4 标识符，并使用
规范 Blueprint 标识符重新输出文档。v5 文件早于 v6 中新增的拓扑和数据集范围
限定信息。消费者必须拒绝未知的未来模式版本，并给出明确的升级提示，而不能静默
丢弃字段。

## 为何选择 TOML 而非 JSON

- TOML 能更清晰地将结构部分与叶数据分开（`[tables.table-001.cols.col-2]` 对比嵌套 JSON）。
- 更容易比较差异（每行一个键；基于标识符的子表保持连续）。
- 如果客户希望在分享前对某个特定字段脱敏，可以手动删除或遮蔽其中的敏感信息。

JSON 在 SQL 回退路径中用作**中间格式**。每个 `sql/blueprint.*.sql` 脚本都会生成 JSON，
由 `blueprint_format.py` 将其规范化为 TOML。中间 JSON 包含真实源标识符；MySQL 还可能
通过 `COLUMN_TYPE` 包含 enum/set 声明，因此必须将其保留在源环境内并加以保护。
规范化器默认使用新的秘密密钥，并为获批的跨运行比较接受同一受保护的
`--anonymization-key-file` 契约。经审阅后与 DBWarp 分享的最终文件始终为 TOML。

## 结构化文件来源扩展

当 `engine` 或 `source_kind` 为 `"parquet"` 或 `"avro"` 时，模式版本 3 及更高版本还可以输出以下有界字段。较旧的读取方必须忽略自己无法理解的字段；较新的读取方必须保留源文件存储与有界的解码样本测量值之间的区别。

结构化文件 Blueprint 使用与数据库 Blueprint 相同的匿名标识符：按基于秘密密钥的顺序使用
`table-NNN`，按模式序号使用 `col-N`。文件名主干、Parquet 路径、Avro 字段名以及
清单中的 `logical_table` 不会作为表或列标识符输出。

表级 `table_bytes` 是传输大小的逻辑估计值，而 `storage_bytes` 是源对象在磁盘上的
实际大小。仅使用元数据的
Parquet 将未压缩列块字节作为 `table_bytes`；可选的解码采样会将其替换为推算的
`blueprint-compression-probe-v2` 字节。Avro 从完整解码扫描中得出该值。
`source_partitions`、`row_group_count` 和 `source_codec` 描述文件布局与调度来源；
多文件数据集会聚合这些值。`row_group_count` 仅适用于 Parquet；单个输入对象的
`source_partitions` 为 `1`。

列级 `null_fraction` 是从 `0.0` 到 `1.0` 的观测值。`length_sample_rows` 和
`length_sample_method` 说明 `len_avg` 与 `len_p95` 的获得方式。
`source_semantics` 保存 `"repeated-leaf"`、`"nested-json"` 或
`"multi-type-union"` 等有界兼容信息；其中绝不包含客户的字段名或字段值。十进制精度和小数位数、时间戳精度及 UTC/本地语义、
UUID 和固定二进制大小由现有标量字段及 `native_type` 保存。

表级 `ratio_storage` 比较 `table_bytes` 与源对象的实际字节数；Parquet 列级值
比较页脚中的未压缩与压缩列块字节。这两者都是文件规划信号，不是解码样本估算。
`ratio_zstd_3` 和 `ratio_zstd_19` 仅在 `sample_encoding` 为
`"blueprint-compression-probe-v2"` 时有效。不得把 Parquet footer 或 Avro 容器比率
复制到这些 zstd 字段中。
