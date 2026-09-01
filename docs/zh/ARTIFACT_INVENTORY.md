# 非表对象清单

> **翻译说明：** 本文为机器辅助翻译，仍需母语技术审校。[规范英文版](../ARTIFACT_INVENTORY.md)具有最高效力；本页不应作为合同文本。

**语言：** [English](../ARTIFACT_INVENTORY.md) | [Deutsch](../de/ARTIFACT_INVENTORY.md) |
[Français](../fr/ARTIFACT_INVENTORY.md) | [Español](../es/ARTIFACT_INVENTORY.md) |
[Polski](../pl/ARTIFACT_INVENTORY.md) | [日本語](../ja/ARTIFACT_INVENTORY.md) |
**简体中文**

从模式 v4 开始，Blueprint 可以描述非表数据库对象和部署前置条件，而不公开源对象名称、定义、
端点字符串、秘密、证书、密钥或二进制文件。该清单帮助 DBWarp 估算迁移复杂度，
并识别需要软件包、基础设施、安全审批或辅助转换的工作。

清单不是能力声明。对象被报告并不表示 DBWarp 能自动重建或翻译它。迁移能力必须
另外依据 DBWarp 路由与对象能力矩阵进行检查。

## 详细级别

使用 `--artifact-detail` 选择隐私与规划信息之间的平衡：

| 值 | 数据库读取 | Blueprint 输出 | 同意 |
|---|---|---|---|
| `none` | 不读取对象目录或定义 | 不输出对象计数或图 | 无需额外同意 |
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

- `view-001`、`function-002` 和 `schema-A` 等稳定匿名 ID；
- 对象种类、子类、层级、可见性和安全模式的封闭标记；
- 仅通过匿名对象 ID 或表 ID 表达的依赖关系；
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
| `contract` | 独立版本化的契约；当前为 `dbwarp-blueprint-artifacts/v1` |
| `detail` | 请求的详细级别 |
| `visibility` | `full`、`privilege_filtered` 或 `unknown` |
| `inventory_complete` | 仅在完全可见、没有不可读目录且没有声明未建模类别时为真 |
| `dependencies_complete` | 仅在依赖来源可读且建模类别均可核算时为真 |
| `analysis_complete` | 仅在 `analyzed` 且所有可用分析均完整时为真 |
| `catalogs_read` | 已成功检查的标准目录类别 |
| `catalogs_unreadable` | 失败或不可用的目录类别 |
| `families_not_inventoried` | 当前采集契约之外的已知对象类别 |

可选目录失败不会静默删除对象。运行会发出 `DBP1410W`，记录受影响目录，并将相应
完整性声明强制设为假。因此，低权限账户可以生成有用的部分清单，而不会把不可见误报为不存在。

## 引擎覆盖

v1 采集器对以下类别建模：

| 引擎 | 已建模对象类别 |
|---|---|
| PostgreSQL | 视图、物化视图、序列、例程、聚合、enum/domain/composite/range 类型、触发器、默认值、检查、策略、规则、事件触发器、扩展、外部表/服务器、发布、订阅、表空间和本机函数 |
| MySQL | 视图、存储函数和过程、触发器、计划事件、视图依赖、FEDERATED 表和可加载 UDF 注册 |
| SQL Server | 视图、存储过程、标量/表函数、CLR 模块、触发器、默认值、检查、规则、同义词、序列、用户定义类型、CLR 程序集、外部数据对象、全文目录、分区对象、非 PRIMARY 文件组、证书、密钥、数据库范围凭据、链接服务器和 SQL Server Agent 作业 |

每个 Blueprint 都列出已知但未建模的类别。除非 `visibility`、完整性字段和未采集类别列表
共同支持该结论，否则不能把零计数解释为不存在。

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

`analyzed` 为可用 SQL 和过程定义添加 `dbwarp-language-feature-census/v1` 块。
首个分析器是 `lexical-v1`，并报告 `status = "partial"`；它不是解析器、编译器、
语义绑定器，也不保证翻译成功。

它记录定义大小、语句数、标记数、嵌套、圈复杂度和不透明/动态区域的有界区间。
封闭词汇涵盖控制流、连接、子查询、CTE、聚合、窗口、DML、DDL、临时对象、
动态 SQL、JSON、XML、空间、向量和安全模式。引擎上下文包含规范化语法配置、
MySQL SQL 模式，以及 SQL Server 兼容级别、`ANSI_NULLS` 和 `QUOTED_IDENTIFIER`。

词法分析器会移除注释、字面量和带引号标识符。上下文规则处理触发器事件、
PostgreSQL `EXECUTE FUNCTION` 和 SQL Server 模块选项。结果仍是粗粒度规划证据。
未来基于语法的分析器可以使用新版本而不更改外层对象契约。

## 建议审查流程

1. 与常规目录审查一起运行 `summary`。
2. 检查计数、外部类别、可见性、不可读目录和未建模类别。
3. 只有在匿名依赖拓扑可接受时才批准 `graph`。
4. 只有在临时读取定义可接受时才批准 `analyzed`。
5. 将审计日志作为受访问控制的证据保存在本地。仅当指定接收者需要端点、身份、路径和
   降级详情时，才通过获批的安全通道共享。
6. 在承诺自动重建或翻译前，将清单与 DBWarp 能力矩阵比较。

精确序列化字段参见[格式参考](FORMAT.md)。运行时读取、写入、警告和信任声明参见
[审计参考](AUDIT.md)。
