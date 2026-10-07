# 非表对象清单

> **翻译说明：** 本文为机器辅助翻译，仍需母语技术审校。[规范英文版](../ARTIFACT_INVENTORY.md)具有最高效力；本页不应作为合同文本。

**语言：** [English](../ARTIFACT_INVENTORY.md) | [Deutsch](../de/ARTIFACT_INVENTORY.md) |
[Français](../fr/ARTIFACT_INVENTORY.md) | [Español](../es/ARTIFACT_INVENTORY.md) |
[Polski](../pl/ARTIFACT_INVENTORY.md) | [日本語](../ja/ARTIFACT_INVENTORY.md) |
Blueprint 可以描述非表数据库对象以及部署的前提条件，而无需公开其源对象名称、定义、端点字符串、秘密、证书、密钥或二进制文件。这个清单可以帮助 DBWarp 评估迁移的复杂性，并识别需要软件包、基础设施、安全审批或协助转换的工作。

库存并非一项功能声明。报告某个对象并不意味着DBWarp可以自动重新创建或转换它。请与DBWarp确认哪些对象类型是支持的。

## 详细级别

使用 `--artifact-detail` 选择隐私与规划信息之间的平衡：

| 值 | 数据库读取 | Blueprint 输出 | 同意 |
|---|---|---|---|
| `none` | 不读取清单目录或定义（仍运行仅计数的拓扑探测） | 明确记录 v7 未请求清单；不输出计数或图 | 无需额外同意 |
| `summary` | 读取对象目录，但不读取定义 | 按对象种类和外部前置类别统计 | 默认；无需额外同意 |
| `graph` | 读取对象目录和依赖元数据，但不读取定义 | 计数、稳定匿名对象记录和依赖边 | 需要 `--yes` |
| `analyzed` | 读取目录、依赖和可用定义 | 图以及有界语言特征和复杂度区间 | 需要 `--yes` |

默认值为 `summary`。如果策略允许收集表结构但禁止非表目录，请使用 `none`。
如需在不读取定义的情况下进行依赖规划，请使用 `graph`；只有在批准临时读取定义后
才使用 `analyzed`。

```bash
./dbwarp-blueprint \
  --connect postgresql://blueprint_user@db.internal/appdb \
  --password-file /etc/dbwarp/blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --artifact-detail analyzed \
  --out appdb.blueprint.toml \
  --audit-log appdb.blueprint.audit.txt \
  --yes
```

## 隐私契约

对象输出只包含采用封闭词汇的有界元数据：

- `view-001`、`function-002` 和 `schema-A` 等在单次运行内一致的匿名 ID；要在多次
  运行之间保持稳定，必须重复使用同一个受保护的 `--anonymization-key-file`；
- 对象种类、子类、层级、可见性和安全模式的封闭标记；
- 仅通过匿名对象 ID 或表 ID 表达的类型化关系，以及封闭的证据和未解析原因标记；
- 用于迁移规划的有界、按引擎限定的功能要求；
- 计数和有界区间，而不是自由文本；
- `pg_proc`、`information_schema.views`、`sys.objects` 等标准目录标签；
- 外部前置条件类别，绝不包含其名称或材料。

输出不包含源对象名称、SQL 或过程源代码、模式名称、主体、端点、提供程序、
凭据、密钥、证书正文、程序集文件、扩展包名称或可加载库名称。

在 `analyzed` 模式中，定义只在移除注释和字面量并生成有界词法聚合期间保留。
定义由释放时清零的所有者持有，不会序列化、记录到日志或发送给其他服务。
这是进程内存最小化措施，并不声称操作系统换页或特权调试器不可能访问它。

即使匿名图也可能通过对象数量和拓扑识别应用。因此，如果操作员未提供 `--yes`，
`graph` 和 `analyzed` 会以 `DBP1014E` 失败。

## 完整性证据

`[artifact_inventory]` 块被设计为可自审计：

| 字段 | 含义 |
|---|---|
| `contract` | 独立版本化的契约；v7 使用 `dbwarp-blueprint-artifacts/v2`，旧 Blueprint 模式保留 v1 |
| `detail` | 请求的详细级别 |
| `scope` | v7 目录范围：`all-visible-schemas`、`selected-schemas`、`structured-source` 或 `unknown` |
| `visibility` | `full`、`privilege_filtered` 或 `unknown` |
| `inventory_complete` | 仅在完全可见、没有不可读目录且没有声明未建模类别时为真 |
| `dependencies_complete` | 仅在依赖来源可读且建模类别均可核算时为真 |
| `requirements_complete` | V7 聚合值：仅当已检查引擎版本和版本类别、选定范围的评估对象完整覆盖，且每个输出成果的 `requirement_status = complete | not_applicable` 时为 true；省略表示 false |
| `analysis_complete` | 仅在 `analyzed` 且所有可用分析均完整时为真 |
| `catalogs_read` | 已成功检查的标准目录类别 |
| `catalogs_unreadable` | 失败或不可用的目录类别；降低受影响的完整性声明，但不删除无关的逐对象要求证据 |
| `catalogs_not_applicable` | 已证明不适用的目录类别；与可读和不可读集合互不重叠 |
| `families_not_inventoried` | 本版本未清点的已知对象类别 |

可选目录失败不会静默删除对象。运行会发出 `DBP1410W`，记录受影响目录，并将相应
完整性声明强制设为假。因此，低权限账户可以生成有用的部分清单，而不会把不可见误报为不存在。

`object_count` 统计输出的对象记录，而不是某个本机目录的行数。Oracle 包和对象类型按
规范、主体和成员建模，主体持有合并的语言分析。同一对象即使出现在多个目录中也只计数一次。
例如，触发器元数据和源代码会在匿名化之前按本机对象标识合并。

## 汇总复杂度契约

模式 v7 为 `graph` 和 `analyzed` 定义仅汇总的
`[artifact_inventory.complexity]` 记录。它不增加数据库读取或权限；评估来自已批准的
匿名图和语言普查。该记录在 `graph` 和 `analyzed` 中必需，在 `none` 和 `summary` 中省略。

评估报告涵盖七个维度：定义量、控制流、功能广度、依赖纠缠度、环境耦合、不透明性以及方言耦合。结果以等级区间呈现，而非数值评分。`overall_score` 字段保留为空，不进行填充，因为 0-100 的数值会暗示并不具备的精度。

每个维度都有针对合格总体的一维精确直方图。规模使用语言普查的大小区间，其余六个维度
使用计数区间。两者都添加 `not_applicable` 和 `unknown` 桶，并满足
`eligible = assessed + not_applicable + unknown`。格式不包含逐对象复合值，也不按对象类型、
功能或模式交叉制表。已明确识别的引擎生成对象和辅助对象不参与评估，但仍保留在清单中；
临时对象和缺少标志的对象仍符合条件。站点安装的插件、CLR 程序集、Java 和库即使主体不可读，
也仍是真实迁移工作；只有明确的引擎生成标志才排除这类对象。

`assessment_population_complete` 表示策略下所有合格对象是否都已知。它独立于
`inventory_complete`，省略表示假。总体不完整时，全局区间为 `unknown`，除非已知下限
已经是 `very-high`。

覆盖率按维度记录。`not_applicable` 是完成的评估。部分评估表示至少一个适用维度已知、
另一个未知；未评估表示没有任何适用维度已知。未知证据绝不视为低复杂度。部分维度为
`unknown`，除非已知下限已经是 `very-high`；只有上下限一致时才输出全局区间。由于不读取
定义，`graph` 不会为非空总体输出全局结论。完整的空总体为 `not-applicable`。

被包装的对象会影响不透明度直方图的 `unknown` 桶，即使无法生成任何部分的语言统计数据。请将不透明度范围与其覆盖范围一起读取，以确保即使观察到较小的范围，也不会在不知道其总体情况的情况下读取。如果评估本身失败，则会保留完整的清单，并使用一个标准的“全部未知”汇总值。被包装或省略的定义、不完整的图表、不完整的需求证据、选定的范围边界以及不支持的语言或方言仍然是明确的限制。这些限制的声明是根据工件和统计证据得出的，如果可能的话。 `unsupported-dialect` 仍然是独立的，因为统计数据可以命名一个方言并报告 `unavailable`，但没有 `unsupported` 状态；这意味着定义已被读取，但指定的分析器不支持该方言，而不是源被省略或包装。

该记录包含唯一的分析器版本以及在符合条件的数据集中存在的分析范围、方言和语法配置文件集合。只有当这些集合、合同、评估器、范围和人口政策都完全匹配时，两个记录才可比较。一个集合保留了每个子源的复杂性，并且永远不会在引擎或分析器之间进行聚合。

复杂性评估模块和评估器版本是独立的。有关精确的字段和不变性规则，请参阅[格式参考](FORMAT.md)。

## 引擎覆盖

当前采集器对以下类别建模：

| 引擎 | 已建模对象类别 |
|---|---|
| PostgreSQL | 视图、物化视图、序列、例程、聚合、enum/domain/composite/range 类型、触发器、默认值、检查、策略、规则、事件触发器、扩展、外部表/服务器、发布、订阅、表空间和本机函数 |
| MySQL | 视图、存储函数和过程、触发器、计划事件、视图依赖、FEDERATED 表和可加载 UDF 注册 |
| SQL Server | 视图、存储过程、标量/表函数、CLR 模块、触发器、默认值、检查、规则、同义词、序列、用户定义类型、CLR 程序集、外部数据对象、全文目录、分区对象、非 PRIMARY 文件组、证书、密钥、数据库范围凭据、链接服务器和 SQL Server Agent 作业 |

每个 Blueprint 都列出已知但未建模的类别。除非 `visibility`、完整性字段和未采集类别列表
共同支持该结论，否则不能把零计数解释为不存在。

## 要求证据

工件需求是基于限定范围的目录列或专门的、具有引擎感知能力的语法检查而得出的引擎事实。通用的词法分析不会产生具有引擎特定性的需求。当分析出的语言特性反映相同的已知事实时，需求具有优先性，而该特性仍然是一个词法观察。缺少某个需求并不意味着所有需求标记都已检查。

每个 v7 graph/analyzed 对象记录 `requirement_status` 为 `complete`、`partial`、`unavailable` 或 `not_applicable`。 只有 `complete` 才能使一个空列表成为该对象零需求的证明。 `partial` 记录某个事实或生产者成功，但没有完全覆盖；`unavailable` 记录没有生产者建立可用的覆盖范围，因此无法伴随已知的需求或外部前提条件证据。 这种证据需要 `partial`。 两者都贡献了未知的、与需求相关的复杂性观察，而完备的对象仍然可以被评估。 `not_applicable` 禁止记录需求和外部前提条件。 只有在引擎版本和版本检查、完整的评估填充，并且每个已输出的对象都完整或不适用时，库存级别的 `requirements_complete` 才为真。 PostgreSQL、MySQL 和 SQL Server 仅在尝试了每个适用的工件目录并且证明了所选范围的完整性后，才设置聚合值。 拒绝的或无法读取的目录，或者无法证明其范围完整性的选择边界，会使聚合值保持为假，而不会从已读取的目录中删除完整的、每个对象的证据。 Oracle 不会报告工件需求。

## 外部前置条件

依赖可移植表 DDL 之外资源的对象会携带匿名外部前置类别：

| 类别 | 操作员需要解决的事项 |
|---|---|
| `postgresql_extension` | 兼容扩展包和目标版本 |
| `postgresql_native_function` | 本机库和 ABI 兼容性 |
| `mysql_loadable_udf` | 可加载 UDF 二进制文件和源服务器 ABI 假设 |
| `sqlserver_clr_assembly` | CLR 启用、程序集、运行时和信任策略 |
| `foreign_endpoint` | 网络、提供程序、远程数据库和身份验证 |
| `replication_topology` | 发布/订阅拓扑和目标策略 |
| `physical_storage` | 文件组或物理布局设计 |
| `server_feature` | 服务器或托管服务功能可用性 |
| `certificate_material` | 按目标策略签发或导入证书 |
| `encryption_or_credential_material` | 密钥、凭据、外部密钥存储和秘密处理 |
| `sqlserver_agent` | Agent 可用性、运行环境和作业治理 |

Blueprint 会记录是否需要但未采集二进制、秘密或端点材料。外部对象必须成为明确的迁移任务，
不能作为尽力而为的静默遗漏。

## 语言特征普查

`analyzed` 详细信息增加了 `dbwarp-language-feature-census/v1` 个块。Schema v7 产生 `lexical-v2`，它只分析可执行或声明性主体，并记录 `analysis_span = "executable-body"`。它排除了外部创建包装、身份、签名、返回声明以及模块选项。如果主体无法安全隔离，则收集器会记录一个未知范围和不可用的证据；没有定义维度的对象使用 `not-applicable`。省略的范围被解释为未知，而不是从引擎中推断。分析器报告 `status = "partial"` 用于支持的定义，因为它不是一个解析器、编译器、语义绑定器，也不是翻译成功的保证。缺失或不支持的定义证据是 `unavailable`，而已证明不适用的分析是 `not_applicable`。

它记录定义大小、语句数、标记数、嵌套、圈复杂度和不透明/动态区域的有界区间。
封闭词汇涵盖控制流、连接、子查询、CTE、聚合、窗口、DML、DDL、临时对象、
动态 SQL、JSON、XML、空间、向量、引发错误、事务控制、ref cursors、锚定类型、interval、
time zone、Boolean、LOB 和安全模式。引擎上下文包含规范化语法配置、
MySQL SQL 模式，以及 SQL Server 兼容级别、`ANSI_NULLS` 和 `QUOTED_IDENTIFIER`。

词法分析器在计数之前，会移除注释、引号引起来的字面量和引号引起来的标识符。它具有针对触发器事件声明、PostgreSQL `EXECUTE FUNCTION` 和 SQL Server 模块选项的上下文规则。即使如此，所有结果仍然是粗略的规划依据。 包装的 PL/SQL 会被拒绝； 混淆的字节永远不会成为可信的身体测量值。

## 建议审查流程

1. 对默认的 `summary` 层级进行对象目录审查。如果策略只允许表目录，请改用
   `--artifact-detail none`；v7 会明确记录该决定，而不是省略清单状态。
2. 检查计数、外部类别、可见性、不可读目录和未建模类别。
3. 只有在匿名依赖拓扑可接受时才批准 `graph`。
4. 只有在临时读取定义可接受时才批准 `analyzed`。
5. 将审计日志作为受访问控制的证据保存在本地。仅当指定接收者需要端点、身份、路径和
   降级详情时，才通过获批的安全通道共享。
6. 不要假设清单中的对象可以自动重新创建或转换；请向 DBWarp 确认。

有关准确的序列化字段，请参阅[格式参考](FORMAT.md)。有关运行时读取、写入、警告和信任声明，请参阅[审计参考](AUDIT.md)。
