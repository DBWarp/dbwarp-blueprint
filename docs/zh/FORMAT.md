# DBWarp Blueprint 文件格式 v7

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../FORMAT.md)。本译文不应被视为合同级文本。

**语言：** [English](../../FORMAT.md) | [Deutsch](../de/FORMAT.md) | [Français](../fr/FORMAT.md) | [Español](../es/FORMAT.md) | [Polski](../pl/FORMAT.md) | [日本語](../ja/FORMAT.md) | **简体中文**

人类可读。可比较差异。可进行取证审查。

> **此格式通过有界模式、基于秘密密钥的标识符和已记录的数值精度，降低隐蔽信道和直接披露
> 风险。匿名图结构和明确选择加入的精确字段仍可能形成工作负载指纹，因此请根据您
> 自己的数据分类政策审阅该文件。**

## 文件头

逐字节完全如下：

```
# dbwarp-blueprint v7
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
| `schema_version` | int | 版本号。当前为 `7`。 1 到 6 的版本仍然可以读取。 |
| `generated_at` | ISO-8601 字符串。 | UTC时间戳，秒级精度，不包含小数。可以通过`--generated-at "2026-04-26T00:00:00Z"`命令行选项进行**固定**。字节级别的实时数据捕获也需要相同的保护措施`--anonymization-key-file`、源状态、选项和采集器版本。审计日志会在设置该选项时记录`generated_at_pin: ...`，以便对固定值进行取证分析。没有环境变量可以固定此值。 |
| `engine` | 字符串。 | `"postgresql"`、`"mysql"`、`"sqlserver"`、`"oracle"`、`"parquet"` 或 `"avro"`。`oracle` 只出现在 Oracle 预览的输出中。 |
| `engine_version` | 字符串。 | 源数据库的数值版本；对于结构化文件源，则为空。不包括分发版本信息。 |
| `source_kind` | 字符串。 | 数据库源使用操作员声明的 `"production"`、`"staging"`、`"scrubbed-replica"` 或 `"synthetic"`。结构化源使用 `"parquet"` 或 `"avro"`。 |
| `length_metadata` | 字符串。 | 摘要：为方便早期读者，保留了标记：`"hybrid-v2"`、`"exact"`、`"rounded"`或`"not-captured"`。以下三个字段为权威信息。 |
| `declared_length_fidelity` | string | PostgreSQL 声明字符容量以及默认 balanced/exact MySQL 模式为 `"exact"`；strict MySQL privacy 为 `"coarse-rounded-v1"`；不可用时为 `"not-captured"`。 |
| `index_length_fidelity` | string | 默认 balanced/exact MySQL 索引前缀为 `"exact"`；strict privacy 为 `"rounded-down-v1"`；不可用时为 `"not-captured"`。 |
| `observed_length_fidelity` | string | 已采样时默认为 `"relative-rounded-v2"`，exact 模式为 `"exact"`，strict 模式为 `"coarse-rounded-v1"`，未采样时为 `"not-sampled"`。采样覆盖率仍是单独的逐列要求。 |
| `[totals]` | inline table | 聚合计数（见下文）。 |
| `[network]` | table | 可选的客户端到数据库连接及查询 RTT 证据。 |
| `[database_topology]` | 表。 | 适用于 schema-v6 及更高版本的数据库源。schema v7 使用拓扑协议 v2，并记录每个成员的数量范围。对于结构化文件，此项不存在。 |
| `[dataset_scope]` | 表。 | 适用于所有 schema-v6 及更高版本的 Blueprint。声明总数涵盖的内容，以及表、行和字节的覆盖范围是否完整。 |
| `[structure_scope]` | 表。 | 在 v7 版本中是必需的。它分别限定了表、列、索引和关系完整性信息的收集范围。 |
| `[source_environment]` | 表。 | 在 v7 版本的数据库蓝图中是必需的，但对于结构化文件则禁止使用。它仅包含通过数据库端点或明确的提供者观察到的粗略 capacity/hosting 证据。 |
| `[statistics_evidence]` | 表。 | 在 v7 版本中是必需的。它需要精确地汇总每个表的行数、优化器统计信息以及大小相关的信息。 |
| `[activity_snapshot]` | 表。 | 未由 DBWarp Blueprint 1.6 编写。 |
| `[tables.X]` | tables | 每个表一个，使用匿名化 ID。 |
| `[fk_edges]` | inline table | 匿名化表之间的 FK 图。可选。 |
| `[artifact_inventory]` | 表。 | 在 v7 版本中，以下内容是必需的，因此“未请求”、“不适用”、“无法读取”，以及经过验证的零对象清单必须保持区分。它包含有限的、不带名称的对象计数，可选的带类型匿名关系，需求，以及有限的语言统计信息。 |

## `[totals]`

| 字段 | 类型 | 精度 |
|---|---|---|
| `table_count` | int | 精确 |
| `row_count` | int | 每个表的序列化数据的总和 `rows`；目录的估算值是四舍五入的，而经过验证的完整、有限的读取结果是精确的。 |
| `table_bytes` | int | 各表舍入后 `table_bytes` 的总和 |
| `index_bytes` | int | 各表舍入后 `index_bytes` 的总和 |

这些数字并非自动计算的整个集群的总和。 始终将它们与 `[dataset_scope]` 一起解读。 一个分片网关或协调器可能会暴露一个看起来完整的目录，但实际上它可能不包含任何底层分片的数据；schema v6 和 v7 明确地表示这种不确定性，而不是默默地将本地目录统计数据视为全局真理。

`row_count` 是每个表的序列化值的算术和，而不是第二个未四舍五入的测量值。低于第一个隐私桶的已知正数计数表示为 `100`，并包含每个表的 `row_count_quality = "engine-estimate"`；因此，包含许多此类小表的数据库可以具有一个保守的较高总和。在这种情况下，`dataset_scope.limitations` 也包含 `row-counts-statistical`。零仍然保留用于表示源表为空的情况。

## `[database_topology]` (数据库源)

此块只记录通过已连接数据库端点可见的有界事实。它绝不存储节点名、主机名、IP 地址、
集群名、复制通道名、服务器标识符或端点。

| 字段 | 值 / 规则 |
|---|---|
| `contract` | `dbwarp-blueprint-topology/v1` 位于 schema v6 中；`dbwarp-blueprint-topology/v2` 位于 v7 中。 |
| `deployment` | `single-node`、`replicated`、`sharded`、`distributed` 或 `unknown`。 |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member`, `physical-standby`, `logical-standby`, `snapshot-standby`, 或 `unknown`。 |
| `visibility` | `full`、`partial` 或 `unknown`；描述拓扑证据，而非数据正确性。 |
| `member_count` | 通过成功证据查询可见的成员数量。`0` 表示未知，绝不表示没有成员。 |
| `member_count_scope` | V7版本仅支持：`deployment`、`visible-subset`、`connected-member`或`unknown`。 要获得完整的拓扑结构可见性，需要`deployment`；`connected-member`需要一个计数。 |
| `identifiers_redacted` | 必须为 `true`。 |
| `role_counts` | 按封闭角色 token 统计的可选数量。完整可见性要求其总和等于 `member_count`。 |
| `features` | 排序后的闭合标记，例如 `citus`，MySQL replication/cluster 格式，`postgresql-streaming-replication`，`sqlserver-availability-group`，`oracle-non-cdb`，`oracle-cdb`，`oracle-pdb`，`oracle-rac`，`oracle-data-guard`，或 `vitess`。 |
| `catalogs_read` | 已成功读取的拓扑目录的排序封闭标签。 |
| `catalogs_unreadable` | 无法读取的拓扑目录的排序封闭标签。任何条目都会阻止完整可见性声明。 |
| `catalogs_not_applicable` | 仅适用于 V7 版本。已排序的封闭标签在此源中被证明不适用。它与可读和不可读的集合是互不相交的。 |

常规终端点可以合法地报告 `deployment = "unknown"`同时仍然报告完整的本地复制表统计数据. 蓝图并没有推断一个不值得注意的服务器是单节点的,仅仅是因为没有集群功能可见.

## `[dataset_scope]` (适用于 schema v6 及更高版本)

这个区块独立地限定了每个大小估算的总和。当任何必需的完整性维度为 `incomplete` 或 `unknown` 时，请不要将这些总和视为整个数据集的数值。

| 字段 | 值 / 规则 |
|---|---|
| `contract` | 始终为 `dbwarp-blueprint-dataset-scope/v1`。 |
| `layout` | `full-copy`、`sharded`、`distributed`、`structured-dataset` 或 `unknown`。 |
| `table_inventory_completeness` | `complete`、`incomplete` 或 `unknown`。 |
| `row_count_completeness` | `complete`、`incomplete` 或 `unknown`。 |
| `size_completeness` | `complete`、`incomplete` 或 `unknown`。 |
| `row_count_method` | 封闭的来源标记，例如 `postgres-planner-estimate`、`mysql-table-statistics`、`sqlserver-partition-counter`、`oracle-table-statistics` 或 `oracle-segment-statistics`。`bounded-complete-read` 和 `mixed-catalog-and-bounded-read` 用于标识从完整且经过验证的二级读取中恢复的总数，可以单独使用，也可以与目录计数一起使用。Oracle 在没有表位于复制总数中的非空库存的情况下，才会将 `not-applicable` 与相同的大小方法一起使用。`distributed-aggregate` 作为输入被接受，但此版本不会写入它。 |
| `size_method` | 封闭的溯源令牌，例如 `postgres-local-relation-size`、`citus-distributed-relation-size`、`mysql-information-schema`、`sqlserver-partition-pages`、`oracle-segment-bytes`、`oracle-table-logical-estimate`、`mixed` 或 `not-applicable`。 Oracle 在包含表的场景下，使用 `mixed` 来结合属性段计数器和带标签的逻辑估算值。 它仅在非空库存中没有表时才使用 `not-applicable`，以防止完全零总数错误地声称一种测量方法。`distributed-aggregate` 作为输入被接受，但此版本不会写入它。 |
| `limitations` | 对覆盖不完整或未知原因进行排序后的封闭说明。除非所有维度都完整，否则至少需要一个。 |

`selection-limited` 表示总计和完整性声明仅覆盖通过可重复实时 `--schema` 选择器请求的 schema，并不声明覆盖整个已连接数据库。省略 `--schema` 时，会保留采集所有可见 schema 的行为。

一个可读的、选定的模式可能只包含非表对象，并且会被保留在相应的清单中。然而，当完整的采集结果中完全没有表时，采集器不应发布一个明确的空数据集：表的完整性、行的完整性和大小的完整性仍然不完整，并且 `table-inventory-visibility-unknown` 记录了保守的边界。

`row-count-evidence-incomplete` 和 `size-evidence-incomplete` 表示至少有一个包含的表缺少相应的目录值。数值总和是已知贡献的总和，而不是断言不可用的表包含零行或字节。每个表的统计信息表明哪些记录不可用。

对于 Oracle，`oracle-segment-bytes` 是首选的、精确的已分配大小的证据。如果无法从 `DBA_SEGMENTS` 中确定表的存储空间——例如，对于一个聚集表或一个索引组织表，其索引目录不可用——采集器可能会发出 `oracle-table-logical-estimate`，并使用已有的 `DBA_TABLES.NUM_ROWS * AVG_ROW_LEN` 值。然后，该表的证据将包含 `size_quality = "engine-estimate"`、`size_scope = "table-only"` 以及 `size_accounting = "logical-estimate"`、`size_visibility = "partial"`，数据集大小的完整性为 `incomplete`；它不声称包含 LOB 或索引的字节数。即使缺少细化目录，也不会丢弃已分配给该逻辑表的字节：测量的贡献仍然是 `oracle-segment-bytes`，具有部分可见性和不完整的聚合覆盖范围。无法归属的共享存储或索引组织存储不会被发布为精确的零。一个具有域索引的 Oracle 表也使用部分可见性和未知的范围，因为 Text、Spatial 以及其他域实现可能将字节存储在用户表清单中未包含的辅助对象中。在这种情况下，逻辑的回退是复制大小的证据，而不是已分配的字节计数器。当优化器的行计数在存储释放后仍然过时时（例如，在 `TRUNCATE ... DROP STORAGE` 之后），它可能会高估当前的分配，并且必须保留其估计来源。对于这种回退，不需要 `DBA_TABLESPACES` 权限。

对于 Oracle 索引存储，已完成的索引目录读取是逻辑快照的边界。 之后出现的 `DBA_SEGMENTS` 行，如果缺少匹配的索引标识，则不属于该范围，也不会被分配给任意表。 如果在边界时索引存在，但缺少其预期的段贡献，则该索引的表将无法完整显示其大小。

`logical-partition-root-unmeasured` 是 PostgreSQL 特有的证据，表明一个包含的逻辑分区根故意不贡献任何行或字节，因为这些值存在于它的物理叶节点上。与可以修复的 `row-count-evidence-incomplete` 统计数据缺失不同，即使完整读取另一个表，也无法在这样的根节点仍然存在于选定的清单中时，恢复数据集级别的完整性。

`table-inventory-visibility-unknown` 表示采集器无法读取引擎拥有的分类信息，该信息用于区分用户对象和支持对象。虽然可能仍然存在可见的记录，但表的完整性、行的完整性和大小的完整性将被撤销，而不是将该子集视为完整的数据。

`catalog-capture-truncated` 表示一个单源目录会话在读取所有预期的系列或选定的模式之前就已停止。 已经证明完整的数据记录可能仍然会被输出，但任何数据集或结构完整性的声明都不能扩展到未读取的剩余部分。

原生 PostgreSQL、MySQL 和 SQL Server 收集器会先探测受支持的拓扑目录，再判断本地
统计能否代表逻辑数据集。已知的分布式网关在无法获得可靠聚合时会抑制不安全的总计。
SQL 回退格式化器没有拓扑探测，因此会保留有用的本地估算，但将所有范围维度标记为
`unknown`，并添加 `topology-unobserved` 和
`topology-visibility-unknown` 限制。

结构化 Parquet 和 Avro Blueprint 省略 `[database_topology]`，并使用
`layout = "structured-dataset"` 和 footer/container 来源信息。

Blueprint 在普通采集期间不会运行存储速度测试，也不会根据运行客户端的机器推断数据库
服务器硬件。数据库字节总计只描述指定目录方法得到的存储数据量；不声称磁盘类型、IOPS、
吞吐量、CPU、RAM 或目标迁移性能。

## `[structure_scope]` (模式 v7)

这段代码的作用是，可以区分一个经过验证的空目录和一个被过滤、无法读取或未被检查的目录。

| 字段。 | 值 / 规则 |
|---|---|
| `contract` | 始终 `dbwarp-blueprint-structure-scope/v1`。 |
| `visibility` | `full`、`privilege-filtered` 或 `unknown`。完整性仅限于选定的模式和可见的权限范围；它并非对数据库完全可见性的声明。 |
| `table_inventory_completeness`, `column_inventory_completeness`, `index_inventory_completeness`, `relationship_inventory_completeness` | 独立地 `complete`、`incomplete` 或 `unknown`。 依赖型组件不能声明已完成，如果其所需的父级组件不完整。 |
| `catalogs_read`, `catalogs_unreadable`, `catalogs_not_applicable` | 排序后的、不相交的、封闭的目录标签。 `catalogs_read` 记录了成功的读取证据；在多所有者捕获中，即使另一个所有者没有成功读取，它也可能保留一个目录，只要至少一个预期所有者成功读取。 `catalogs_unreadable` 表示没有成功的读取。 一个完整的家族需要其特定于引擎的目录在 `catalogs_read` 中，并且每个预期的家族查询都必须完成，并且没有受影响的、针对每个对象的缺失。 |
| `limitations` | 已排序的关闭原因，例如 `selection-limited`、`metadata-visibility-privilege-filtered` 或 `table-kinds-not-inventoried`。 部分或未知的证据需要提供原因。 |

模式选择器是范围的一部分：`complete` 表示已解析的选定模式的完整状态，但并不一定表示服务中的所有模式。如果选择器解析结果为空，则表示错误，并且不能生成一个完整的、空的文件。

`catalog-capture-truncated` 在结构证据中具有相同的含义：已发布的表和列记录是经过验证的前缀或所有者子集，而不是表明剩余的预期目录工作已完成。如果在某个阶段未尝试的目录，则不会出现在三个目录集合中的任何一个；它不应被重新标记为无法读取或不适用。

对于多所有者读取，`index-inventory-unavailable` 或 `relationship-inventory-unavailable` 可能会与 `catalogs_read` 中的目录同时出现：目录标签保留至少一个预期所有者成功读取的正向证据，而完整性字段和限制记录表明，整个选定范围尚未完全观察。每表限制标识证据不足的对象；它们不能取代被拒绝或未尝试的所有者查询状态证据，即使该所有者没有输出任何表。

Oracle 将记录 `oracle-identity-columns` 和 `oracle-constraint-columns` 与其父列和约束目录分开存储。它们的存在或缺失描述了可选的身份生成和关系键信息；这不应被简化为父目录无法读取的说法。

## `[source_environment]` (schema v7 数据库源)

这段文字描述的永远不是运行 `dbwarp-blueprint` 的工作站。 `collector_machine_excluded` 必须是 `true`。 容量的证据只能来自连接的数据库端点，或者经过明确授权的提供者、协调者或操作员的证明。

| 字段。 | 值 / 规则 |
|---|---|
| `contract` | 始终 `dbwarp-blueprint-source-environment/v1`。 |
| `evidence_origin` | `database-endpoint`、`provider-api`、`orchestrator-api`、`operator-attested`、`mixed` 或 `none`。 |
| `hosting_model` | `managed-service`、`self-managed`、`orchestrated` 或 `unknown`。 |
| `infrastructure_location` | `cloud`、`on-premises`、`hybrid` 或 `unknown`。 |
| `capacity_scope` | `connected-instance`、`database-resource`、`cluster-aggregate`、`member-subset` 或 `unknown`。 |
| `capacity_visibility` | `capacity_visibility` 可能为 `full`、`partial`、`unknown` 或 `not-requested`。`not-requested` 需要未知的容量范围、基础和适用范围，并且没有分类的容量目录。非容量分类，例如 SQL Server 版本，可能仍然存在。 |
| `cpu_capacity_band` | `1`, `2`, `3-4`, `5-8`, `9-16`, `17-32`, `33-64`, `65-128`, `129-plus`, 或 `unknown`。 |
| `cpu_capacity_basis` | `logical-cpu-limit`、`database-resource-limit`、`operating-system-visible`、`physical-host` 或 `unknown`。`operating-system-visible` 不声称某个虚拟机、容器或托管服务实例就是底层的物理主机。 |
| `memory_capacity_band` | 来自 `under-2-gib` 到 `512-gib-plus` 之间的粗略范围，或者 `unknown`。 |
| `memory_capacity_basis` | `database-buffer-cache`、`database-resource-limit`、`operating-system-visible`、`physical-host` 或 `unknown`。`operating-system-visible` 是一个保守的基准，用于一个引擎的 DMV，其值可能描述的是一个虚拟机或容器，而不是裸机。一个 `database-buffer-cache` 范围表示配置的缓存分配，因此它只是总源内存的下限；它绝不能被误解为宿主机的容量，除非明确说明其基准。 |
| `member_capacity_uniform` | 可选的 observed/attested 布尔值；省略表示未知。 |
| `features` | 排序后的封闭事实，例如 `autoscaling`、`burstable`、`container-limits-visible`、`database-resource-governed`、`serverless` 或 `shared-host`。 |
| `limitations` | 已排序的封闭溯源限制。`oracle-client-version-mismatch` 或 `oracle-client-version-unreadable` 表明 Oracle SQL*Plus 客户端版本无法得到完整证明。`oracle-client-version-below-tested-floor` 表明已证明的客户端版本早于 12.1，即此契约中编码的比较下限。目录捕获仍会继续，因为客户端横幅的溯源信息并不决定数据库结构。 |
| 目录集合。 | 对尝试过的精确源环境目录进行排序且保持互不重叠，包括 SQL Server 版别分类，即使其可选的容量 DMV 无法读取。 |

未知的容量不等于零容量。 远程连接并不授权读取采集主机的CPU或内存，并将读取结果标记为服务器容量。

对于 Oracle 数据库，即使一个容量目录只完成了预期查询集的一部分，它仍然是 `catalogs_read` 的有效证据，但其数值会被隐藏，并且 `capacity_visibility` 是 `unknown`。 必须避免将部分数据呈现为整个系统的 CPU 或内存限制。

Oracle SQL*Plus 的功能设置会优先从运行会话的数字版本中选择（如果可读），否则从可执行文件横幅中选择；如果二者都不可读，则使用既不选择 `ROWLIMIT` 也不选择 CSV 标记作为输出功能的保守协议下限。两次设置过程都始终尝试清除继承的 `ROWLIMIT` 和 CSV 模式；旧客户端的未知选项诊断仅在限定的重置窗口内被容忍。版本不匹配、部分证明、解析失败，或者已证明的客户端版本早于 12.1，只会削弱溯源信息。它们不会阻止目录捕获。

## `[statistics_evidence]` 和 `[tables.<id>.statistics]` (模式 v7)

每个 v7 表都包含一个统计信息块。顶层块包含精确的计数，分别对应 `statistics_state`、`row_count_quality` 和 `size_quality`；每个映射必须覆盖所有表，并且与表级别的分类完全一致。只有当所有被计数的表都具有完整的尺寸可见性、已知的行数据和已分类的统计信息状态，并且没有无法读取的统计信息目录，并且复制总数不为空时，聚合可见性才为 `full`。 故意排除的外部、临时和派生对象仍然会被记录；其受策略驱动的行数据和尺寸不可用性不会降低该复制总数的可见性，但未分类的统计信息状态仍然会影响。当所有表都被排除时，聚合可见性为 `unknown`，并且为 `statistics-visibility-unknown`；一个空集不应该空洞地获得 `full`。`catalog-capture-truncated` 记录了计划的统计信息目录工作在到达所有所有者之前停止，但保留了任何已获得的积极的统计信息目录读取证据。当一个所有者读取成功，而另一个被拒绝时，也适用相同的积极证据规则：目录仍然处于 `catalogs_read` 状态，而 `statistics-partial` 和聚合可见性记录了所选的集合未被完全观察到。

表级别的字段包括：

| 字段。 | 值 / 规则 |
|---|---|
| `row_count_method` | Engine/version-aware 目录方法，`bounded-complete-read` 当一个二级语句安全地枚举了可见的表、一个结构化文件计数器，或者 `unknown` 时；普通的采集不会静默地回退到 `COUNT(*)`。 |
| `row_count_quality` | `exact-counter`、`exact-read`、`engine-counter`、`engine-estimate`、`cached-engine-estimate`、`sample-extrapolation`、`unavailable` 或 `unknown`。 一个已知的、在 SQL Server 中为正值的计数器，其值低于第一个非零的隐私桶，在序列化 `rows = 100` 后使用 `engine-estimate`；这可以区分隐私桶，使其既不是精确计数器，也不是测量为零的值。 |
| `statistics_state` | `current`、`possibly-stale`、`known-stale`、`never-analyzed`、`locked`、`user-supplied`、`not-applicable` 或 `unknown`。 |
| `refresh_age_band` | `under-1h`、`1h-1d`、`1-7d`、`1-4w`、`1-3m`、`3m-plus`、`unknown` 或 `not-applicable`。 |
| `modification_ratio_band` | `none`, `under-1pct`, `1-5pct`, `5-10pct`, `10-20pct`, `20-50pct`, `over-50pct`, `unknown` 或 `not-applicable`。 |
| `sample_fraction_band` | `full`、`75-99pct`、`50-74pct`、`25-49pct`、`under-25pct`、`unknown` 或 `not-applicable`。 |
| `statistics_scope` | `global`, `partition`, `subpartition`, `session`, `local-member`, `logical-dataset`, `database-resource`, `structured-dataset`, `selected-object`, 或 `unknown`。 |
| `size_method`, `size_quality`, `size_scope`, `size_accounting`, `size_visibility` | 分别描述大小数据的来源，说明它是计数结果还是估算值，是否包含LOB/index存储空间，是已分配的还是逻辑上的，以及可见性是完全可见、部分可见、不可见还是未知。 |

顶层统计信息块使用 `visibility = "full"`、`"partial"` 或 `"unknown"` 来描述上述非空复制总数。它绝不会使用仅被排除的对象来获得 `full` 的可见性。

Oracle `oracle-segment-bytes` 的证据只有在 `exact-counter`、`allocated-segment`、完全或部分可见的情况下，以及一个 `segment_state` 证明了段的统计信息被归属 ( `created`、`deferred`、`mixed` 或 `mixed-table-and-index` ) 时才有效。 逻辑回退则使用 `oracle-table-logical-estimate`、`engine-estimate`、`logical-estimate`、部分可见性，以及不可用的段状态。 这可以防止未归属的存储类变成测量为零。 状态是从归属的目录证据派生而来，然后再进行隐私处理。 因此，`created` 可以伴随序列化的零表字节，当已知的正向原始表计数器低于第一个字节桶时。 部分测量的 Oracle 证据使用 `size_scope = "unknown"`：归属的字节仍然精确，但缺少 LOB、嵌套存储或索引映射意味着收集器无法诚实地声称具有完整的 table/LOB/index 范围。 对于 Oracle，`mixed` 意味着一个正向归属的索引分配被抑制，并序列化为 `index_bytes = 0` 通过四舍五入；它将零与一个段区分开来，该段的段统计信息未找到任何索引分配。 `mixed-table-and-index` 意味着表和索引的分配在四舍五入之前都是正向的，并且两个序列化计数器都是零，从而保留了这两个事实，而不会泄露子桶的字节值。 `deferred` 意味着归属的原始计数器为零，并且需要两个序列化字节值都为零。 请使用状态来区分四舍五入的子桶分配和证明未物化的存储。

顶层块也记录了排序后的、不相交的目录以及封闭的限制。零值 `rows` 或 `table_bytes` 只能作为观察到的零值使用，并且需要相应的 quality/visibility 证据支持；不要忽略来源信息块。对于一个计数表，如果其行或大小质量为 `unavailable` 或 `unknown`，则会使相应的完整数据集偏离 `complete`；验证器会拒绝将数值占位符作为完整覆盖来呈现。特别是，对于一个 PostgreSQL 表，如果既没有优化器统计信息，也没有经过验证的完整读取范围，则其行数未知，而不是一个测量到的零值。

Oracle Basic 会省略 `check_count`，当字典无法区分声明的 `NOT NULL` 约束和显式、文本上完全相同的 `CHECK` 时。它不会根据生成的约束名称或列的当前是否允许空值来推断。其他表，其约束行的含义明确，可能仍然包含精确的计数。

## `[activity_snapshot]`

DBWarp Blueprint 1.6 不会写入这个代码块。

## `[network]`（可选）

从运行采集器的机器到您的数据库的往返时间。 这不是迁移源和目标之间的往返时间。

探测在连接建立后、目录查询前运行，因此时间不会受到查询缓存预热的影响。它执行 **5× `SELECT 1`** 并输出延迟中位数。每次 `SELECT 1` 都返回常量整数 1，此探测绝不会读取任何行数据。

当使用 `--no-rtt-probe` 时，或者当探测本身在执行过程中失败时（记录为非致命警告，输出到标准错误流和审计日志；Blueprint 文件仍然会被生成，但该部分数据缺失）。

| 字段 | 类型 | 精度 |
|---|---|---|
| `sample_count` | int | 精确（v1 中始终为 5） |
| `connect_total_ms` | int | 从 TCP 连接开始到经过身份验证的会话就绪的总墙钟时间，以毫秒计。包括 TCP 握手 + TLS 握手（如适用）+ 身份验证质询/响应。舍入到最接近的毫秒。通常为 `query_rtt_ms_p50` 的 3–6 倍。 |
| `query_rtt_ms_p50` | int | 5 个 `SELECT 1` 样本的单次往返延迟中位数，以毫秒计。舍入到最接近的毫秒。自然网络噪声下限（实践中 ≥ 1 ms）大于舍入粒度，因此既可消除任何低位隐蔽信道，又不会损失有用精度。低于毫秒的 LAN 值会归并为 0 或 1。 |
| `query_rtt_ms_p95` | int | 5 个样本采用最邻近秩法计算的第 95 百分位数（即最慢的观测值），以毫秒为单位，并舍入到最接近的毫秒。请与 p50 结合使用，以识别短暂的延迟峰值；5 个样本仅供粗略判断，不能视为工作负载性能测试。 |

5 个探测查询会在审计日志中显示为 **一条汇总记录**（而不是 5 个单独行），标签为 `5x SELECT 1 (RTT probe; constant integer 1, no row data)`。这符合“不读取任何行内容”的信任原则。

## `[tables.<id>]`

标识符为 `table-NNN`，其中 `NNN` 是在域分隔的 HMAC-SHA256 排序中，模式和表名的 1-基于索引的序号。默认密钥是为该进程动态生成的，并且永远不会被输出。传递相同的受保护的 `--anonymization-key-file` 可以确保在经过批准的比较运行中保持排序不变。模式 v7 需要从 `table-001` 到输出的表计数之间的完整、连续的序号集合（宽度会自然增长到 `table-1000`）；跳过、零、非十进制或源导出的后缀是无效的。

| 字段 | 类型 | 精度/值 |
|---|---|---|
| `rows` | int | 目录估算值会四舍五入：舍入到最接近的 100（≤10k）、最接近的 1000（≤1M）或最接近的 10000（>1M）。一个已知为正的估算值，如果原本会四舍五入到零，则使用第一个非零的区间 (`100`)；零用于表示目录为空或无法获得有效数据，由相邻的统计质量标识。当一个有限制的二级读取证明它枚举了完整的可见表时，`rows` 是该样本已经显示的精确数值 `sample_rows`；这避免了在不增加新通道的情况下，对每个表、基数和聚合计数产生矛盾。 |
| `table_bytes` | int | 按量级舍入到最接近的 1KiB / 1MiB / 100MiB |
| `index_bytes` | int | 舍入方式与 `table_bytes` 相同 |
| `schema` | 字符串。 | 匿名标识符 `schema-A`, `schema-B`, ..., `schema-AA`。 Schema v7 需要对所有被输出表或 graph/analyzed 实体引用的 schema 使用连续的字母顺序集合；因此，仅包含非表对象的选定 schema 将被保留。 |
| `object_kind` | 字符串。 | V7 需要封闭的标记：`ordinary-table`、`materialized-view`、`external-table`、`temporary-table`、`nested-table` 或 `object-table`。对象标识与物理存储和分区无关。 |
| `storage_organization` | 字符串。 | V7 需要封闭的标记：`heap`、`index-organized`、`clustered`、`external` 或 `unknown`。 `external` 仅对 `object_kind = "external-table"` 有效。 |
| `partitioning` | 字符串。 | V7 需要关闭的标记：`none`、`range`、`list`、`hash`、`interval`、`reference`、`composite`、`system`、`key`、`linear-hash`、`linear-key` 或 `unknown`。 |
| `segment_state` | 字符串。 | V7 需要封闭标记：`created`、`deferred`、`mixed`、`mixed-table-and-index`、`unavailable` 或 `unknown`。 这将仅包含元数据的对象与实际存储的对象区分开来。 这是一个在字节舍入之前确定的分类证据，因此 `created` 可能会伴随零序列化的表字节，表示原始分配是正数但低于第一个字节桶。 借助 Oracle 的段计数证据，`mixed` 记录了正数的已归属索引分配，其序列化值 `index_bytes` 舍入为零；`mixed-table-and-index` 记录表和索引的原始分配都为正数，而两个序列化计数器都为零。 |
| `parent_table`, `child_tables` | 字符串 / 数组 | 可选的、双向的匿名表链接，用于嵌套、分区或其他包含的对象。子对象 ID 按照顺序排列且唯一；父对象图必须是无环图。 |
| `table_features` | 数组。 | 已排序的闭合令牌：`graph-edge`、`graph-node`、`memory-optimized`、`temporal-current` 或 `temporal-history`。 |
| `unlogged` | 布尔类型。 | 可选的 PostgreSQL 已记录状态观察。如果未捕获，则省略；明确的 `false` 表示该目录已证明该表已启用日志记录。 |
| `partition_count` | int | 精确的、在范围内的物理叶子分区数量，当 `partitioning` 指定一种已知的分区策略时，是必需的。PostgreSQL 报告递归的叶子分区，并排除在已解析模式之外的叶子分区（`selection-limited`）。MySQL 复合表会计算子分区，因为这些是它们的物理叶子；例如，四个顶层分区，每个分区有八个子分区，会报告 `32`。零值仅在逻辑分区根节点且 `segment_state = "unavailable"` 为空，即没有在范围内的叶子时才有效。 |
| `partition_key_cols` | 整型数组。 | 完整地记录简单分区键列的序号。如果分区键完全或部分基于表达式，或者当无法获取目录信息时，则不记录；部分序号列表和键表达式永远不会被序列化。 |
| `partition_rows_max` | int | 可选的、四舍五入后的最大叶子行数估算值。对于估算级别的表总数，如果已知为正值，则使用第一个非零的行桶，并以序列化的 `rows` 为上限。对于精确读取的表，如果最大叶子数的估算值，其隐私桶的值为零或超过该精确的表总数，则该估算值将被省略，而不是被限制到一个虚假的值。当存在时，如果 `rows` 为正数，它不能为零，并且绝不能超过 `rows`。 |
| `temporal_history` | 字符串。 | 与 `temporal-current` 功能相关的配对时间历史表的匿名表标识符，除非该表包含适用的、针对每个对象的 `table_limitations` 令牌。 全局选择仅捕获数据，并不影响连接。 |
| `table_limitations` | 数组。 | 已排序的、针对每个对象的证据。 `table-classification-unavailable` 标识了一个表，其对象类型输入不完整。 `column-inventory-unavailable` 标识了一个表，其中存在一个或多个缺失或无法读取的列记录；`dependent-structure-suppressed` 指出，对于该表，索引和关系结构都无法声明为完整，因为缺少一个已输出的列。 `index-inventory-unavailable` 和 `relationship-inventory-unavailable` 将仅限细化的差距缩小到受影响的依赖项，而不会撤回所需的列清单。 任何引用缺失已输出列的索引、分区键或关系都会被省略，而不是允许其使整个捕获过程无效。 `relationship-target-outside-selected-scope` 记录到，至少有一个在该表上声明的外部键的目标对象位于已解析的选定模式范围之外；它仅在 `selection-limited` 捕获中有效。 `relationship-target-visibility-unknown` 记录到，目录公开了一个无法在可见清单中解析的外部键目标；因此，关系完整性必须是不完整的。 `row-security-filter-active` 记录到一个可见的、已启用的 SQL Server 过滤器谓词。 `row-security-visibility-unknown` 记录到，无法证明完整的 SQL Server 安全策略目录可见性，因此，而不是将潜在的过滤子集视为整个表，而是抑制了二级采样。 `temporal-history-outside-selected-scope` 仅在 `selection-limited` 捕获中，对于一个未链接的、当前时间表有效，前提是收集器已在选择之外解析了历史模式。 `temporal-history-visibility-unknown` 记录到，目录公开了一个历史对象 ID，但没有足够的元数据来解析它。 |
| `counted_in_totals` | 布尔类型。 | 省略意味着包含。一个`external-table`、`materialized-view`、`temporary-table`或包含`memory-optimized`的表，需要明确指定`false`，从而排除来自`table_count`、`row_count`、`table_bytes`和`index_bytes`的外部、派生、会话范围或当前未测量的任何数据。每个对象的证据仍然可用于重建计划，而不会将不可用的值作为测量的总数呈现。没有其他明确的值是标准值。 |
| `check_count` | int | 可选的、精确的结构化 CHECK 约束数量。如果省略，表示未知；`0` 表示相关的目录中没有找到。 |
| `has_clustered_index` | bool | PostgreSQL 始终为 `false` |
| `[tables.<id>.statistics]` | 子表 | 需要 v7 版本的行数、优化器统计状态和大小信息。v6 版本的 `stats_freshness` 字段仅在读取旧文件时有效，并且永远不会在 v7 版本中输出。 |
| `[tables.<id>.cols.<cid>]` | sub-tables | 每列一个 |
| `[tables.<id>.idxs.<iid>]` | sub-tables | 每个索引一个 |
| `[tables.<id>.compression]` | sub-table | 仅 Tier 2 |

## `[tables.<id>.cols.<cid>]`

标识符为 `col-N`，其中 `N` 是该列的自然属性顺序（从 1 开始计数，保留磁盘上的顺序）。在每次运行中保持一致。在模式 v7 中，十进制后缀必须完全等于 `ordinal`；零、前导零以及源系统生成的标签无效。源系统可能在删除列后，保留物理列序号中的空缺。

| 字段 | 类型 | 说明 |
|---|---|---|
| `ordinal` | int | 与 ID 相同的 N |
| `type` | string | 标准化类型系列，例如 `"integer"`、`"numeric(12,2)"`、`"text"`、`"json"`、`"binary"`、`"timestamp"`、`"uuid"`、`"array<integer>"` 或 `"user-defined"`。不会输出真实的 domain、enum、alias、composite 和用户定义类型名称。 |
| `nullable` | bool |  |
| `value_source` | string | Schema v6 可选封闭标记：`identity-always`、`identity-default`、`auto-increment`、`identity`、`sequence-default`、`generated-stored`、`generated-virtual`、`computed-persisted`、`computed-virtual`、`system-time` 或 `rowversion`。普通输入值或证据未知时省略。 |
| `has_default` | bool | Schema v6 可选目录观测。省略表示未知；显式 `false` 表示目录确认没有默认值。 |
| `default_kind` | string | Schema v6 可选分类 `constant`、`function` 或 `expression`，仅在 `has_default = true` 时有效。绝不序列化默认值文本或字面量。 |
| `default_on_null` | 布尔类型。 | V7 可选的源目录观察结果，适用于 Oracle `DEFAULT ON NULL`；仅在存在默认值时有效。 省略表示未观察到。 |
| `type_kind` | string | Schema v6 可选封闭标记：`enum`、`set`、`domain`、`composite`、`array`、`range` 或 `alias`。基础类型或证据未知时省略。 |
| `member_count` | int | Schema v6 精确的正结构成员数，仅 `enum` 和 `set` 必需；绝不序列化成员名。 |
| `domain_has_check` | bool | Schema v6 可选 domain CHECK 观测，仅 `type_kind = "domain"` 时有效。 |
| `hidden`, `invisible`, `masked`, `encrypted`, `sparse` | 布尔类型。 | 可选的目录观察。 `invisible` 与引擎创建的隐藏列不同。 省略表示未知；明确的 `false` 表示该目录证明该属性不存在。 |
| `has_check` | bool | Schema v6 可选单列 CHECK 观测。每个显式 `true` 都包含在表的 `check_count` 中。 |
| `null_fraction` | 浮点数。 | `0.0` 到 `1.0` 范围内的可选观察到的空值比例。当存在基数时，该比例是从该块的隐私保护的公开计数中推导出来的；否则，该比例是独立计算的。不会保留任何空值位图。 |
| `native_type` | string | 可选的净化后引擎基础类型，例如 `varchar` 或 `longtext`；不含标识符、enum 成员、默认值或表达式。由原生 MySQL 和 SQL Server 采集器输出。 |
| `declared_max_chars` | int | 可选的声明字符容量。PostgreSQL `character`/`character varying` 目录值以及默认 balanced/exact MySQL 模式下都精确；仅对 MySQL 使用 `--length-fidelity strict` 时粗略舍入。 |
| `declared_max_bytes` | int | 可选的声明字节容量。在默认 balanced/exact MySQL 模式下精确；仅使用 `--length-fidelity strict` 时粗略舍入。 |
| `length_semantics` | 字符串。 | V7 可选的声明长度单位：`characters`、`bytes`、`not-applicable` 或 `unknown`。 这在不序列化声明的情况下，保留了 Oracle 的 CHAR 与 BYTE 语义。 |
| `numeric_model` | 字符串。 | V7 需要封闭的家族：`integer`、`fixed-decimal`、`unconstrained-decimal`、`decimal-float`、`binary-float`、`not-applicable` 或 `unknown`。 `not-applicable` 表示已知的非数值类型；`unknown` 预留用于数值类型或用户自定义类型，其语义尚未分类。 `decimal-float` 包含精确的 Oracle `FLOAT(p)` 值，并且不是 IEEE 浮点数。 |
| `numeric_precision` | int | 可选的正数声明精度，受源数据库引擎和模型限制：Oracle `NUMBER` 和 SQL Server 的十进制精度最高可达 38 位，Oracle `FLOAT(p)` 最高可达 126 位二进制数字，MySQL 的十进制精度最高可达 65 位，以及 PostgreSQL 的 numeric 精度最高可达 1,000 位。 |
| `numeric_scale` | int | 可选的、带有符号的、声明的比例，会针对源数据库引擎进行验证。Oracle `NUMBER` 使用 `-84..127`；PostgreSQL 支持其更广泛的、依赖于版本的声明范围，而 MySQL、SQL Server、Parquet 和 Avro 要求比例必须是非负数，且不能大于精度。如果数据库引擎允许，则保留负数的 Oracle/PostgreSQL 比例以及大于精度的比例。 |
| `numeric_precision_radix` | 字符串。 | `decimal` 或 `binary`，当数值模型需要时使用。Oracle `FLOAT(p)` 使用二进制精度，其精确值为 `decimal-float`；`BINARY_FLOAT` 和 `BINARY_DOUBLE` 使用 `binary-float`。 |
| `numeric_unsigned`, `bit_width` | 布尔类型/整数类型。 | 可选的整数语义，如果源引擎提供了这些语义。 |
| `datetime_precision` | int | 可选的，由引擎声明的date/time小数精度。 |
| `charset`、`collation` | string | 可选的净化后字符元数据。MySQL 输出其目录字符集和排序规则名称。SQL Server 对 `nchar`/`nvarchar`/`ntext` 输出 `utf-16le`，对代码页 65001 输出 `utf-8`，对 Windows 代码页 1250–1258 输出 `windows-N`，对其他正数目录代码页输出 `code-page-N`，并输出目录排序规则名称。这些是编码事实和目录名称，而不是您的标识符或值。 |
| `len_avg` | int | 可变长度值的采样平均字节数。默认相对分桶的最大误差约为 3.2%，并精确保留不超过 32 字节的值；使用 `--length-fidelity exact --yes` 时精确；仅在 strict 模式下粗略舍入到最接近的 10。0 = 固定长度或未测量。 |
| `len_p95` | int | 使用相同默认相对分桶的采样第 95 百分位数；使用 `--length-fidelity exact --yes` 时精确；仅在 strict 模式下粗略舍入到最接近的 100。0 = 未测量。 |
| `style` | string | 仅 Tier 2。`"json"`、`"xml"`、`"natural-text"`、`"base64"`、`"hex"`、`"numeric-text"`、`"mixed"` 或 `"precompressed"` 之一；未分类时为空。只有带有可识别标准容器签名、且在字节量上占显著主导地位的二进制值样本才会输出 `"precompressed"`。它刻意不披露检测到的容器系列。 |
| `[tables.<id>.cols.<cid>.lob_storage]` | 子表 | V7 可选数据库 LOB 存储的证据：`storage_class` (`basicfile`, `securefile`, `external`, `unknown`)，压缩 (`none`, `low`, `medium`, `high`, `not-applicable`, `unknown`)，去重 (`enabled`, `disabled`, `not-applicable`, `unknown`)，可选 in-row/encrypted 选项，以及可见性 (`full`, `partial`, `unknown`)。 外部内容需要这两个存储控制设置为 `not-applicable`，并且不包含数据库中的选项。 不保留任何路径或段名称。 |
| `magnitude_min`, `magnitude_max` | int | Schema v6 可选有符号十进制指数，用于界定采样非 NULL 数值的数量级。与 `has_negative` 一同输出；绝不序列化精确值。 |
| `has_negative` | bool | Schema v6 可选符号观测，仅与两个数量级边界一同输出。 |
| `time_span` | string | Schema v6 可选采样日期/时间范围：`intraday`、`days`、`weeks`、`months`、`years` 或 `decades`。 |
| `time_recent_decade` | int | Schema v6 最新采样日期/时间所在十年，仅与 `time_span` 一同输出，且始终能被 10 整除。 |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | 仅 Tier 2。为已采样的文本/二进制候选列提供。字段布局与表级压缩相同，但范围限定为一个匿名化列。 |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Schema v3 采样值分布摘要。仅包含有界或舍入后的计数与频率。 |

`numeric_model` 对数值语义具有权威性。 `type` 保持引擎系列的拼写：Oracle 的 `NUMBER` 系列，以及 `type = "number"` 和 `FLOAT(p)` 系列，分别对应 `"float"`，而 `native_type` 保留了原始声明的经过处理版本。

### `[tables.<id>.cols.<cid>.cardinality]`（schema v3）

当启用行采样时，采集器在内存中最多保留每个列 8,192 个临时 64 位指纹，计算聚合 NDV/skew 统计信息，然后丢弃这些指纹。 数值和指纹都不会被序列化。 该块包含 `measured`、`sample_rows`、`non_null_rows`、`observed_distinct_count`、`estimated_distinct_count`、`top_value_fraction`、`frequency_p50`、`frequency_p95`、`frequency_p99`、`frequency_max`、`sample_method`、`complete_source_read`、`sample_layout`、`sampled_with_bias` 和 `bias_reason`。 `complete_source_read = true` 是机器可读的证据，表明一个限定范围的语句观察到了完整的可见源数据，并且保留了该列，而没有进行任何值级别的截断。 一个经过验证的完整行读取，即使在单元上限或有限的指纹存储区导致 `complete_source_read` 为假的情况下，也能将 `sample_rows` 保留在表的确切行范围内。 表格数据的具体内容已经在 `tables.<id>.rows` 中已经存在，因此这并没有透露任何额外的信息。 `non_null_rows` 首先进行隐私保护处理，然后 `null_fraction` 从 `(sample_rows - non_null_rows) / sample_rows` 中推导得出。 因此，该比例值与公开的统计数据完全一致，并且可以在不泄露其他信息的情况下，从独立的 0.005 比例网格中移除。 精确的零值和所有非空值端点都会被保留； 一个混合的统计数据，保持非空且为正值的总人口数量，并且保持在 `sample_rows` 以下。 混合模式 `non_null_rows` 使用与其它基数计数相同的相对数量网格。 在数据分布的尾部，这可能会导致计数比实际保留的数据量少一个完整的计数区间（例如，`9,728` 对于 `9,999`）。 它不是一个非常精确的计数。 该精确的、所有字段均非空（all-non-NULL）的端点明确表明，在保留的行中未观察到任何空值（NULL），而如果观察到任何空值，则计数将保持在 `sample_rows` 以下。 即使在限定范围内，不同的数据项和频率统计仍然遵循其已记录的隐私规范。 对于数据库源，如果读取不完整，则会将`sample_rows`限制为四舍五入后的目录行数估计值。 对于 Parquet 和 Avro 格式，精确的尾部行数是上限。 在这两条路径中，除非该上限更低，否则 `sample_rows` 是表级压缩块的 `sample_rows` 已公开的、精确的保留行数。 它绝不能被向上限制，以暗示完全覆盖。 数据截断会保持数据的独特性和频率统计的准确性，并且绝不能将其夸大到超过表中的实际数据量。 不要通过解析 `sample_method` 来推断完整性。 `sample_layout` 是一个可选的、机器可读的枚举类型。 当前输出的值为 `primary-key-range-windows`； 缺失意味着没有可用的排序协议。 不要通过解析人类可读的 `sample_method` 字段来推断其含义。

计数和比例在适当情况下进行了隐私处理。这些统计数据描述了重复数据的密度、热点值的倾斜以及有限的范围。它们不包含任何抽样值，但独特的分布可能会识别工作负载；不要将它们视为不可逆转的，或者作为证明无法从外部知识推断业务含义的证据。一个有上限的陈述可以证明可见的行数据，而无需证明每个抽样单元都完整保留。如果服务器端的单元上限截断了列，即使表行计数是从完整的、有上限的读取中记录的，其基数仍然是一个有上限的、有偏差的估计；不受影响的列可能仍然保留完整的读取基数信息。

### `[tables.<id>.cols.<cid>.compression]`（仅 Tier 2）

针对每一列的压缩仅在有限的text/binary候选对象上生成，当使用`--measure-compression --yes`时。它提供针对每一列的压缩估算值。

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
sample_method = "TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "server_side_cell_cap"
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

## `[tables.<id>.idxs.<iid>]`

标识符为 `idx-N`，其中 `N` 是表中索引的 1-基于索引的序号，按照域名分隔的 HMAC-SHA256 哈希值对索引名称进行排序。Schema v7 要求每个表都包含一个连续的集合 `idx-1` 到 `idx-N`；零、前导零、空隙以及非十进制后缀都是无效的。

| 字段 | 类型 | 值 |
|---|---|---|
| `type` | string | 标准化索引方法系列，例如 `"btree"`、`"hash"`、`"gin"`、`"gist"`、`"brin"`、`"spgist"`、`"fulltext"`、`"spatial"`、`"clustered"`、`"nonclustered"`、`"clustered columnstore"`、`"nonclustered columnstore"` 或 `"other"`。不会输出扩展/自定义方法名称。 |
| `primary` | bool | 可选；主键索引输出为 `true`。否则省略/为 false。 |
| `unique` | bool |  |
| `cols` | array of int | 按索引列顺序参与索引的列序号 |
| `prefix_lengths` | array of int | 可选的 MySQL 索引前缀长度，与 `cols` 对齐；零表示完整列。默认精确；仅使用 `--length-fidelity strict` 时向下舍入。 |
| `include_cols` | array of int | 可选；源引擎公开的非键 INCLUDE 列序号。 |
| `expression` | bool | 可选；存在无法表示为简单列序号的表达式/函数键材料时为 true。 |
| `filtered` | bool | 可选；对于 filtered/partial 索引为 true。 |
| `descending` | bool | 可选；任何键列显式降序时为 true。 |
| `partitioning` | 字符串。 | V7 可选的物理分区：`none`、`local`、`global` 或 `unknown`。 |
| `visibility` | 字符串。 | V7 可选的源可见性：`visible`、`invisible` 或 `unknown`。 |
| `state` | 字符串。 | V7 可选的操作状态：`usable`、`unusable`、`in-progress`、`failed` 或 `unknown`。 |
| `prefix_distinct_counts` | array of int | Schema v3 对从一列到 N 列的每个键前缀估算的不同元组数。零表示该前缀不可用。 |
| `cardinality_sample_method` | string | `prefix_distinct_counts` 的有界来源；推断得到的乘积会被明确标记，不会作为直接元组样本呈现。 |

## `[tables.<id>.compression]` 和 `[tables.<id>.cols.<cid>.compression]`（仅 Tier 2）

仅当文件是使用 `--measure-compression --yes` 生成时才显示。 表级别的块是对完整样本的无偏列投影，并且是整个表传输估算的权威比率。 列级别的块是从相同的样本行投影而来，一次投影一列，并显示哪些列可以很好地压缩，而不会暴露样本值。 它们不会触发额外的数据库读取。

受活动行级别安全策略控制的 PostgreSQL 表，包括继承或分区的子表，其祖先策略会被直接子查询绕过，以及受启用的安全过滤器谓词控制的 SQL Server 表，不会被采样。它们的目录信息会被保留，并且运行记录 `DBP1407W` 会记录这些信息，而不是将经过策略过滤的子集作为整个表的代表。

| 字段 | 类型 | 精度 |
|---|---|---|
| `measured` | bool | 如果块存在，则始终为 `true` |
| `sample_rows` | int | 精确 |
| `sample_bytes` | int | 内存中样本缓冲区的大小，经过**分桶**：低于 1 MiB 时舍入到最接近的 **64 KiB**，低于 1 GiB 时舍入到最接近的 **1 MiB**，高于该值时舍入到最接近的 **100 MiB**。字节绝不会写入磁盘。分桶消除了精确 `buf.len()` 原本会暴露的逐表低位隐蔽信道。 |
| `sample_method` | string | 特定引擎的有限范围抽样描述，例如 `"TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"`、`"LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"` 或 `"SELECT TOP N bounded projection FROM <table> (compression sample; server-side cell cap)"`。 |
| `sampled_with_bias` | bool | 如果样本不均匀（例如仅使用 LIMIT 的回退），则为 true |
| `bias_reason` | string | 当 `sampled_with_bias = false` 时，此字段为空。否则，它包含一个标签，例如 `"unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"`。 |
| `ratio_zstd_3` | float | 按契约的 zstd 级别 3 测量策略舍入到最接近的 **0.05**。在通过 `sample_encoding` 编码的字节上测量。 |
| `ratio_zstd_19` | 浮点数。 | 此版本未编写；可能出现在早期版本的文件中。 |
| `ratio_stddev` | float | 舍入到最接近的 **0.05**，有界表探针帧的级别 3 比率标准差。列级投影块目前输出 `0.0`，因为它们是建议性熵提示，而不是方差模型。 |
| `sample_encoding` | 字符串。 | 用于字节级别编码和压缩会话策略的标识符，该策略用于测量。PostgreSQL 实时表块使用 `"blueprint-columnar-transfer-probe-v2"`。MySQL 和 SQL Server 使用 `"blueprint-columnar-transfer-probe-v3"`，该策略还会在 256 KiB 的探测块边界处刷新。SQL Server 的 `nvarchar`/`nchar`/`ntext` 数据包保留原始的 UTF-16LE 字节分布；`varchar`/`char`/`text` 保留其采样的字节宽度，并且 `charset` 字段标识了数据库的代码页。V1 可以在输入时接受。每个列的块使用 `"blueprint-compression-probe-v2"`。使用不同的 `sample_encoding` 值测量的比例不可比较。 |

PostgreSQL 使用 v2 版本，而 MySQL 和 SQL Server 使用 v3 版本；请仅在同一编码范围内比较比例。

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

`ratio_zstd_3` 描述了名为 `sample_encoding` 的对象；它不是数据库协议或迁移过程的字节数据的捕获。 此仓库中的测试套件验证了确定性编码、有限采样和序列化，但并不声称对每条提取路径都具有普遍的跨引擎百分比误差。

在使用该比例进行重要容量决策之前，请务必将该比例与代表性的源数据以及预期的提取机制进行比较。记录比较方法、样本大小、二进制哈希值、引擎版本以及观察到的错误，并将其与生成的计划一起记录。该基本关系是`compressed_bytes ≈ sample_bytes / ratio_zstd_3`，它基于记录的编码所产生的字节分布。

## `[fk_edges]`

可选的内联表，其中每个键都是一个映射到边列表的 `table-NNN` ID。Schema v3 会保留父列序号、引用操作、匹配模式、可延迟性、验证/信任状态，以及可选的有界、无名称关系摘要。边先按目标排序，再按列列表排序。

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

可选的 `statistics` 块会记录采样或推断得到的 `non_null_rows`、`distinct_parent_values`、`parent_coverage_fraction`、扇出 p50/p95/p99/max 和 `orphan_rows`，以及来源和偏差字段。经过验证的源约束意味着孤立记录数为零。由逐列样本得出的复合估算会明确标记为推断值。

## `[artifact_inventory]` (自 schema 版本 4 起；在版本 7 中为必需)

Schema v7 使用独立版本化的 `dbwarp-blueprint-artifacts/v2` 协议来描述非表对象，而不会序列化源名称或定义。较旧的 schema 版本保留 v1 协议。V7 始终输出此块：`--artifact-detail none` 明确记录了一个未请求的数据库清单，而结构化文件源则输出一个明确的“不适用”清单。因此，缺失的块永远不会被误认为是经过验证的空目录。

默认的 `--artifact-detail summary` 输出 `object_count`、
`external_prerequisite_count`、`counts_by_kind` 和
`counts_by_external_class`。`graph` 还为每个对象输出匿名对象记录和依赖边。
`analyzed` 添加从可用定义临时派生的有界
`dbwarp-language-feature-census/v1` 记录。图拓扑可能识别应用程序，因此
`graph` 和 `analyzed` 都明确需要 `--yes`。

`object_count` 是采集器输出的工件记录数量，而不是任何一个原生目录返回的行数。因此，一个包或类型可以贡献独立的规范、主体和成员记录。一个在多个目录中出现的原生对象仍然是一个记录：例如，来自触发器和源目录的 Oracle 触发器行，通过它们的原生对象标识符进行关联，源行是对触发器记录的补充，而不是重复。

Oracle 的包和对象类型使用相同的记录结构：一个 `specification` 记录，一个作为其实现的 `body` 记录，以及每个目录成员一个 `package_member` procedure/function 记录，其中规范作为父项。 只有主体拥有组合的源代码和语言统计信息；成员保留其目录信息，但使用不适用的定义分析。 这可以防止词法分析器假装它可以将包源代码拆分为成员主体。

清单级证据包括：

| 字段 | 值 / 规则 |
|---|---|
| `detail` | `none`、`summary`、`graph` 或 `analyzed` |
| `scope` | V7: `all-visible-schemas`、`selected-schemas`、`structured-source` 或 `unknown`；它必须与文件中其他地方的模式选择证据相符。 |
| `visibility` | `full`、`privilege_filtered` 或 `unknown` |
| `inventory_complete` | 仅在可见性完整、没有不可读目录且没有已声明的未建模类别时才可为 true |
| `dependencies_complete` | 仅在已建模依赖目录可读时才可为 true |
| `requirements_complete` | V7 聚合值：仅当选定范围的评估对象完整覆盖，且每个输出成果的 `requirement_status = complete | not_applicable` 时为 true；省略表示 false，空的要求列表不构成完整性证据 |
| `analysis_complete` | 仅对 analyzed 详细级别且每项输出分析都完整时才可为 true |
| `catalogs_read` | 已成功检查的标准引擎目录的封闭标签 |
| `catalogs_unreadable` | 未能通过验证的目录标签；每个条目都会影响该目录提供的完整性声明，而与其他对象相关的、独立的验证信息可能仍然完整。 |
| `catalogs_not_applicable` | 被证明不适用的 V7 目录标签；与可读和不可读的目录集合互不相交。 |
| `families_not_inventoried` | 本版本未记录的已知对象类型 |

### `[artifact_inventory.complexity]` (模式 v7)

`dbwarp-blueprint-artifact-complexity/v1` 块是对匿名数据对象的汇总评估。它在 `none` 和 `summary` 详细信息中不存在，但在 `graph` 和 `analyzed` 详细信息中需要存在，其中存在表示已尝试进行该评估。计算失败会产生一个“失败”状态的未知结果，而不是中止 Blueprint 的执行。

顶层字段是固定的：

| 字段。 | 值 / 规则 |
|---|---|
| `contract` | `dbwarp-blueprint-artifact-complexity/v1` |
| `assessor_version` | `1` |
| `scope` | 必须完全等于 `artifact_inventory.scope`。 |
| `population_policy` | `exclude-known-engine-generated-and-secondary`; 缺失分类标志的对象仍有资格纳入总体，临时对象也仍有资格。 |
| `assessment_population_complete` | 只有当符合人口策略的每个对象都已知的时，该条件才为真；如果缺少任何对象，则该条件为假，并且这个声明与更广泛的`inventory_complete`字段无关。 |
| `eligible_object_count` | 该策略评估的对象。 |
| `fully_assessed_object_count` | 每个维度要么已知，要么已证明 `not-applicable`。 |
| `partially_assessed_object_count` | 至少有一个已知的适用维度，并且至少有一个未知的维度。 |
| `unassessed_object_count` | 没有已知的适用维度。 |
| `excluded_object_count` | 受记录策略排除的对象。 |
| `analyzer_version` | v7版本的数据采集使用的单一分析器为：`lexical-v2`，或者在图形模式下为`not-applicable`。 |
| `analysis_spans` | 符合条件的普查记录中，已排序且唯一的封闭范围标记为：`executable-body`、`not-applicable` 或 `unknown`；在图表模式下为空。 |
| `dialects` | 符合条件的普查记录中，已排序、去重且为封闭方言的唯一标识符。 |
| `grammar_profiles` | 符合条件的普查记录中，已排序并去重的语法特征。 |
| `overall_band` | `trivial`、`low`、`moderate`、`high`、`very-high`、`not-applicable` 或 `unknown` |
| `overall_score` | 未由本次发布实现。 |
| `limitations` | 以下是已排序的、已关闭的原因，具体描述如下。 |

这两个人口计算公式都使用了校验算术：

```text
artifact_inventory.object_count = eligible_object_count + excluded_object_count
eligible_object_count = fully_assessed_object_count
                      + partially_assessed_object_count
                      + unassessed_object_count
```

`dimensions` 包含精确的 `volume`、`control_flow` 和 `feature_breadth`。 `entanglement`、`environment_coupling`、`opacity` 和 `dialect_coupling`。 每个维度都有一个封闭的 `band`，一个 `coverage` 值 (`complete`, `partial`, `not-applicable` 或 `unknown`，以及一个固定的直方图。 `volume` 维度 `band`，与其他维度结果一样， 使用 `trivial`、`low`。 `moderate`, `high`, `very-high`, `not-applicable`, 或 `unknown`。 它的直方图使用以下大小键：`0`、`1-255`、`256-1k`、`1k-4k`。 `4k-16k`、`16k-64k` 和 `64k+`。 另外六个直方图使用计数键 `0`、`1`、`2-4`、`5-8`、 `9-16`、`17-32` 和 `33+`。 每个直方图也都有 `not_applicable` 和 `unknown` 桶。 对于每一个维度，验证过的算术运算需要：

```text
eligible_object_count = assessed evidence-band counts
                      + not_applicable
                      + unknown
```

因此，覆盖范围是按维度划分的，而不是单个整体的指标。一个 `not_applicable` 人口普查结果是确定的证据，并有助于该维度的 `not_applicable` 分类；它并不意味着该对象没有被评估。顶层 fully/partially/unassessed 对象的计数是一个汇总结果：所有“不适用”的对象都被完全评估；“部分”意味着至少有一个适用的维度已知，而另一个未知；“未评估”意味着没有已知的适用维度。

一个部分维度被评估为上下限。它的`band`是`unknown`，除非已知的下限已经是`very-high`，因为任何未知的观测值都可能占据最高的区间。这可以防止部分直方图将观察到的下限作为最终结论。

`external_binary` 是一种定义可见性状态，而不是一个排除标志。站点安装的插件、CLR 模块、Java 对象以及外部库仍然属于可以进行迁移的工作，并且通常会贡献与定义相关的未知信息。只有明确设置的 `generated_by_engine = true` 标志才会排除引擎提供的对象（在评估器 v1 版本下）。

直方图是故意设计为一维的。按类型、特征、模式或其他属性进行的交叉分析不属于产品范围。精确计数不会提供超出分析模式下序列化的、针对每个对象的统计信息之外的任何信息，而固定的结构避免了发布任何官方的“资产指纹”辅助工具。

关闭限制的原因包括：`definition-analysis-not-requested`、`definitions-withheld`、`unsupported-dialect`、`wrapped-source`、`graph-incomplete`、`requirements-incomplete`、`outside-selected-scope`、`computation-limit` 和 `computation-failed`。`requirements-incomplete` 表示需求捕获不完整。具有 `partial` 或 `unavailable` 需求状态的对象，会产生未知的环境和方言耦合观察结果，而不是零；独立完整的对象仍然会被评估。 `unsupported-dialect` 表示定义已获取，但其语言或方言没有支持的分析器；它既不被保留，也不被故意隐藏。图表细节使用 `definition-analysis-not-requested`；它不能声称存在定义读取限制，因为没有尝试进行任何读取操作。

未知的证据对于每个受影响的维度都是独立限制的。只有当下限和上限评估结果一致时，才会发出总体的范围。一个完整的、空的、符合条件的总体是`not-applicable`，而不是`trivial`。图形模式始终使用`unknown`作为非空总体的一个总范围，因为它不读取总评估所需的定义。缺失的图形连接会导致受影响的关联证据未知。 `computation-limit` 在此版本中未实现。意外的计算失败会记录`computation-failed`，保留完整的工件清单，并将聚合评估标记为“失败关闭”，而不是抑制Blueprint。

`assessment_population_complete`，而不是广泛的`artifact_inventory.inventory_complete`，决定了该有限结果是否具有确定性。如果评估对象不完整，则总体范围为`unknown`，除非已知的下限已经达到`very-high`；对于可能不可见的的对象，不应假设任何有限的上限。

完全包含的对象对不透明度贡献 `unknown`；即使无法生成完整的统计数据，它们也不会被排除在不透明度直方图中。将不透明度与其覆盖范围一起呈现，这样一个小的不透明区域就不会掩盖一个大的未知区域。

`unsupported-dialect` 仍然是一个明确的限制，因为人口普查可以识别一种方言并报告 `unavailable`，但没有 `unsupported` 状态。 只有当定义可用时，并且记录的方言不受指定分析器的支持时，才会出现这种情况。 其他定义方面的限制同样源于定义的可视性、人口普查状态和相关证据，而不是作为独立的断言来维护。

比较资格的计算基于以下文件：复杂性合约、评估器版本、分析器版本、精确的分析范围、方言和语法配置文件集、范围以及人口策略。一个粗略的 homogeneous/mixed 选项不会被序列化，因为不同的混合集合不一定可以比较。

复杂性始终是源端的。一个包会保留每个子蓝图的评估结果，并且永远不会在引擎、分析器版本、方言或语法配置之间创建包级别的复杂性指标或直方图。

每个对象的ID格式为`<kind>-NNN`，例如`view-001`、`package-002`或`procedure-003`。V7识别常见的对象类型，以及Oracle的包、调度器对象、数据库链接、目录、库、Java对象、操作符、索引类型、域、注释和属性图，以及与引擎无关的`queue`和`edition`类型。最小值为三位数，并用零填充，每种类型都有自己的密集序数集合，从`001`开始；当值超过999时，宽度会增加。该记录仅包含封闭的kind/subkind/tier标记、匿名的schema/parentID、定义visibility/security模式、可选的有效性和目录标志、封闭的需求覆盖范围、可选的外部前提条件以及可选的语言统计信息。父对象可能是一个匿名表或其他对象，因此可以在不使用名称的情况下保留包到过程的层次结构；父对象图必须是无环的。

V7 版本在所有引擎中都使用一个封闭的 `subkind` 词汇表：

```text
ordinary, other, materialized, integer_sequence, stored_procedure,
stored_function, scalar_function, inline_table_function, table_function,
user_defined_aggregate, table_trigger, ddl_event_trigger, before_insert,
before_update, before_delete, after_insert, after_update, after_delete,
generated_column, column_default, default_constraint, check_constraint,
row_security, rewrite_rule, legacy_rule, enum, domain, composite, range,
alias_type, table_type, clr_type, clr_procedure, clr_scalar_function,
clr_table_function, clr_aggregate, clr_trigger, clr_assembly,
server_extension, loadable_udf, foreign_data_wrapper_server, foreign_table,
federated_table, external_table, external_data_source, external_file_format,
logical_replication_publication, logical_replication_subscription,
full_text_catalog, partition_scheme, partition_function, tablespace, filegroup,
database_certificate, symmetric_key, asymmetric_key, column_master_key,
column_encryption_key, database_scoped_credential, linked_server,
enabled_event, disabled_event, enabled_agent_job, disabled_agent_job,
database_synonym, specification, body, package_member, public, private,
java_source, java_class, java_resource, external_library,
user_defined_operator, domain_indextype, scheduler_job, scheduler_program,
scheduler_schedule, scheduler_chain, advanced_queuing, service_broker
```

V7 替换了模糊的 v1 依赖项列表，使用排序后的带类型的 `relationships`。 关系类型区分调用、读取、写入、table/object 引用、触发器所有权、实现、物理位置、安全、扩展使用、外部 binaries/services 和远程 database/server 使用。 每一个关系都记录一个封闭的证据标记 (`catalog-confirmed`, `dependency-confirmed`, `syntax-confirmed`, `lexical-hint` 或 `unresolved`)。 `dependency_edge_count` 必须与生成的图完全一致。

`requirements` 使用限定引擎的令牌，并采用有限的计数范围。它们用于识别兼容性需求，例如：Oracle 包装的源、复合触发器、包状态、动态 SQL、自主事务、pipelined/parallel/aggregate 过程、外部库、数据库链接、域索引、调度器、object/collection/spatial/vector 类型、Java 对象或属性图。 它们仅作为规划的依据。

需求来源于有限的目录事实或专门的、具有引擎感知能力的语法检查。通用的词法分析永远不会生成具有引擎特定性的需求。每个模式 v7 图表或分析的工件都包含 `requirement_status = complete | partial | unavailable | not_applicable`。`complete` 表明该列表对于该工件是完整的；`not_applicable` 表明需求模型不适用，因此禁止需求和外部前提条件记录。`partial` 和 `unavailable` 使该对象的环境和方言相关的观察结果未知。`partial` 表示至少有一个事实来源成功，但没有完全覆盖；`unavailable` 表示没有需求来源建立可用的覆盖范围，因此无法包含已知的需求或外部前提条件证据。此类证据需要 `partial`。这允许一个无法访问的对象在本地降级，而不是擦除对其他部分的有用覆盖。

库存级别 `requirements_complete` 是一个汇总指标。它只能在所有已生成的成果都为 `complete` 或 `not_applicable`，并且所选范围内的评估对象完整时才为真；即使所有成果都为 `complete`，它也可能仍然为假。切勿将一个空的 `requirements` 数组解释为零耦合，除非该成果的状态为 `complete`。

`unresolved_relationships` 是一个有界的计数映射。它区分远程和跨数据库引用、选定的模式边界、权限隐藏的目标、加密或省略的定义、动态 SQL、模糊的绑定、缺失或不完整的本地身份、未建模的目标类型以及未知情况。完整的依赖关系证据要求此映射为空。源对象名称、SQL 文本、安全主体、端点、凭据、密钥、证书和二进制文件不是合同中的字段。

外部前提条件记录一个封闭的 `class`、部署范围、是否需要但未采集二进制/
秘密/端点材料，以及一个有界兼容性类别。其计数是迁移规划证据，并不表示
DBWarp 能自动配置或转换它们。

V7 语言的统计记录使用 `analyzer_version = "lexical-v2"` 和 `analysis_span`。 分析器只接收可执行或声明性的主体部分，不包括外部创建包装、身份、签名、返回声明以及模块选项。 头部信息仍然是目录要求或选项。 无法安全隔离主体部分的收集器会记录 `analysis_span = "unknown"` 以及不可用的证据，而不是分析包装部分。 经过验证的、不适用的定义使用 `analysis_span = "not-applicable"`。 省略的范围证据被保守地解释为 `unknown`；它永远不会从引擎或对象类型中推断出来。 由此词法实现分析的受支持定义使用 `status = "partial"`；缺失或不支持的定义证据使用 `unavailable`，并且经过验证的、不适用的对象可能使用 `not_applicable`。 计数、大小、嵌套、复杂性和不透明区域的值是范围，而不是精确的源指纹。 特性是从一个封闭的词汇表中选择的。 分析器会移除注释、字面量和带引号的标识符；它不是一个解析器、语义绑定器，也不是翻译成功的保证。

Wrapped PL/SQL 永远不是可执行代码的证据。采集器会将其标记为加密状态，并阻止对其字节进行分析；共享分析器也会拒绝包含“wrapped”标记的 PL/SQL 单元，以防止因错误分类而产生看似合理但实际上是错误的统计数据。

操作指南和引擎覆盖范围参见[非表对象清单](ARTIFACT_INVENTORY.md)。

## 按向量划分的隐写防护

| 向量 | 防护方式 |
|---|---|
| 标识符排序。 | 使用与特定进程相关的密钥进行域分隔的 HMAC-SHA256 验证，可以防止在离线状态下进行候选名称的检查。 仅当需要稳定的跨运行标签时，才重复使用您持有的密钥。 |
| 数值低位 | 默认将统计信息舍入到有文档记录的精度。精确长度模式必须显式启用、经同意门控并记录到审计日志中，而且必须作为更敏感的元数据处理。 |
| 亚秒级时间戳 | 顶部只有一个 UTC 时间戳，且仅有秒级精度 |
| TOML 格式 | 标准的输出使用固定的键顺序和缩进，不会发生变化。它只包含标准的头部和生产者注释，不包含从输入中推导出的任何注释。 |
| 抽样随机性。 | 采样使用固定的种子 (PG 的确定性 `TABLESAMPLE SYSTEM`)。 另外，标识符匿名化会从操作系统 CSPRNG 中获取一个密钥，除非您提供一个。 |
| 未使用字段 | 每个字段均在上文记录；不存在承载无界数据的“metadata”/“comment”/“reserved”字段 |
| 对象源文本和外部材料 | 定义是临时的，并在有界分析后清零；名称、SQL 文本、端点、提供商字符串、凭据、密钥、证书、包名称和二进制文件都没有可序列化字段 |

## 模式版本兼容性

当前版本会输出模式版本 7。版本 1 到 6 仍然被接受，以保持向后兼容。一个 v1/v2 文件没有分发块。一个 v3 文件包含分发元数据，但没有工件清单。一个 v4 文件可能包含工件清单，但早于当前的 Blueprint 合约标识符。读取器在输入时会规范化以前的 v4 标识符，并重新输出该文档，使用标准的 Blueprint 标识符。一个 v5 文件早于在 v6 中添加的拓扑和数据集范围证据。V6 使用拓扑合约 v1，工件合约 v1，组合表 kind/partition 字段，无符号十进制比例，以及可选的统计新鲜度。V7 使用拓扑合约 v2 和工件合约 v2，需要明确的 structure/environment/statistics 证据，分离正交的表语义，支持带符号的十进制比例和 Oracle 数字模型，并且需要明确的工件清单状态。它还保留了固定的 `dbwarp-blueprint-artifact-complexity/v1` 聚合合约，而没有添加每个对象的评分或交叉表字段。读取器会拒绝未知的未来模式版本，并显示清晰的升级消息，而不是静默地丢弃字段。读取器会对独立的和嵌入式的 Blueprint 应用相同的严格 v7 验证，而不是在解析期间重写无效的证据。

## 为何选择 TOML 而非 JSON

- TOML 能更清晰地将结构部分与叶数据分开（`[tables.table-001.cols.col-2]` 对比嵌套 JSON）。
- 更容易比较差异（每行一个键；基于标识符的子表保持连续）。
- 在分享之前，请根据贵组织的数据分类政策进行审查。

JSON 在 SQL 回退路径中用作**中间格式**。每个 `sql/blueprint.*.sql` 脚本都会生成 JSON，
由 `blueprint_format.py` 将其规范化为 TOML。中间 JSON 包含真实源标识符；MySQL 还可能
通过 `COLUMN_TYPE` 包含 enum/set 声明，因此必须将其保留在源环境内并加以保护。
规范化器默认使用新的秘密密钥，并为获批的跨运行比较接受同一受保护的
`--anonymization-key-file` 契约。经审阅后与 DBWarp 分享的最终文件始终为 TOML。

## 结构化文件来源扩展

当 `engine` 或 `source_kind` 为 `"parquet"` 或 `"avro"` 时，模式版本 3 或更高版本也可能输出以下受限字段。 读者必须保持源文件存储和受限解码样本测量之间的区别；不支持文档模式版本的读者应拒绝该文档，并显示升级提示，而不是丢弃未知字段。

V7 结构化文件蓝图需要完整的 `[structure_scope]` 和 `[statistics_evidence]` 块，省略 `[database_topology]`、`[source_environment]` 和 `[activity_snapshot]`，并输出明确的“不适用”标记 `[artifact_inventory]`。 它们绝不会从采集主机推断出数据库拓扑或服务器机器的容量。

结构化文件 Blueprint 使用与数据库 Blueprint 相同的匿名标识符：按基于秘密密钥的顺序使用
`table-NNN`，按模式序号使用 `col-N`。文件名主干、Parquet 路径、Avro 字段名以及
清单中的 `logical_table` 不会作为表或列标识符输出。

在表级别，`table_bytes` 是逻辑传输大小的估算值，而 `storage_bytes` 是源对象在磁盘上的实际大小。仅元数据的 Parquet 使用未压缩的列块字节来表示 `table_bytes`；可选的解码采样会用预测的 `blueprint-compression-probe-v2` 字节替换该估算值。Avro 从其解码后的完整扫描中得出该值。可选的 `source_partitions`、`row_group_count` 和 `source_codec` 字段描述了文件布局。多文件数据集会汇总这些值。`row_group_count` 是 Parquet 特有的；`source_partitions` 是单个输入对象的 `1`。

列级 `null_fraction` 是从 `0.0` 到 `1.0` 的观测值。`length_sample_rows` 和
`length_sample_method` 说明 `len_avg` 与 `len_p95` 的获得方式。
`source_semantics` 保存 `"repeated-leaf"`、`"nested-json"` 或
`"multi-type-union"` 等有界兼容信息；它绝不包含您的字段名称或值。十进制精度和小数位数、时间戳精度及 UTC/本地语义、
UUID 和固定二进制大小由现有标量字段及 `native_type` 保存。

表级 `ratio_storage` 比较 `table_bytes` 与源对象的实际字节数；Parquet 列级值
比较页脚中的未压缩与压缩列块字节。这两者都是文件规划信号，不是解码样本估算。
`ratio_zstd_3` 和 `ratio_zstd_19` 仅在 `sample_encoding` 为
`"blueprint-compression-probe-v2"` 时可比较。不得把 Parquet 页脚或 Avro 容器比率
复制到这些 zstd 字段中。
