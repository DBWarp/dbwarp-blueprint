# 批量采集和 Blueprint 捆绑包

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../BATCH_AND_BUNDLES.md)。本译文不应被视为合同级文本。

**语言：** [English](../BATCH_AND_BUNDLES.md) | [Deutsch](../de/BATCH_AND_BUNDLES.md) | [Français](../fr/BATCH_AND_BUNDLES.md) | [Español](../es/BATCH_AND_BUNDLES.md) | [Polski](../pl/BATCH_AND_BUNDLES.md) | [日本語](../ja/BATCH_AND_BUNDLES.md) | **简体中文**

`dbwarp-blueprint` 同时支持单源 Blueprint 文件和多源捆绑包目录。

当您共享一个数据库、一个表子集、一个Parquet文件或一个Avro文件时，请使用单个`blueprint.toml`。当您拥有多个数据库、多个结构化文件数据集，或者希望获得一个完整的审查包时，请使用一个集合。

## 捆绑包布局

批量运行会写入一个目录：

```text
customer-blueprint-bundle/
  bundle.toml
  blueprints/
    erp_pg.blueprint.toml
    billing_mysql.blueprint.toml
    orders_parquet.blueprint.toml
  audits/
    erp_pg.audit.txt
    billing_mysql.audit.txt
    orders_parquet.audit.txt
```

`bundle.toml` 包含源级元数据和指向子 Blueprint 文件的相对路径。这是首选工作形式，因为每个源都能保持可独立审阅、审计和重新运行。

为了将一个包作为一个文件共享，可以将该目录打包成一个嵌入式的 TOML 文件：

```bash
dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

打包形式会将每个子 Blueprint 嵌入其来源条目下。它会保留操作员提供的来源 ID、标签、
数据集组 ID 和审计路径元数据，因此请使用匿名清单值，并在传输前检查打包后的文件。
工作目录更便于审阅，但也包含详细审计和可能存在的 `errors.txt`；默认不要将其整体传输。

## 捆绑包契约

当前捆绑包使用 `schema_version = 3` 和
`kind = "dbwarp-blueprint-bundle"`。目录捆绑包通过 `blueprint_path` 引用每个
子 Blueprint；打包后的捆绑包则将其嵌入 `blueprint`。写入端仅输出这些规范标识符。

v1 或 v2 版本的打包文件仍然会被读取，并会被转换为 v3 格式。这些版本不会记录数据源是独立的、副本还是分片，因此它们之间的关系会被标记为 `unknown`，并且跨数据源的聚合总数会被隐藏。子路径必须是相对路径，并且在规范化处理后必须仍然位于打包文件的目录内。

Bundle v3 将物理采集源与逻辑数据集分开。每个源都有
`dataset_relationship`、`dataset_group` 和 `dataset_scope_completeness`。
顶层 `dataset_groups` 表记录关系、成员以及所声明成员集合是否完整。

聚合采用 fail-closed 策略：

- `independent`：组中恰好有一个源；总计只加一次。
- `replica`：一致副本只计算一次。若声明的副本不一致，则保留一个确定性代表，不做
  平均，并将结果标记为不完整。
- `shard`：只有在 `members_complete = true` 且所有声明成员均成功时才相加。不完整的
  分片组不贡献任何总计。
- `unknown`：抑制所有跨源表、行和字节总计。
- 即使关系已知，只要某个源的 `[dataset_scope]` 不完整或未知，聚合证据就会标记为
  不完整。

每个源自身的总计始终保留。抑制只影响跨源聚合，从而避免复制集被重复计算，或将部分
分片呈现为完整数据集。

## 批处理清单

创建一份清单：

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
dataset_relationship = "independent"
tags = ["critical", "erp"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
dataset_relationship = "independent"
tags = ["billing"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
dataset_relationship = "independent"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
dataset_relationship = "independent"
tags = ["lake"]
```

若省略关系，默认值为 `unknown`；运行仍会成功，但会发出 `DBP1414W` 和
`DBP1417W`，并抑制聚合总计。这比假定两个端点就是两个独立数据集更安全。

使用同一个组声明复制成员：

```toml
[[source]]
id = "orders_primary"
kind = "postgresql"
connect_env = "ORDERS_PRIMARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true

[[source]]
id = "orders_secondary"
kind = "postgresql"
connect_env = "ORDERS_SECONDARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true
```

对于分片系统，请在同一个组中列出每个已知分片，并且只有当清单枚举了完整逻辑数据集时
才设置 `dataset_group_complete = true`。成员失败会使该组在本次运行中不完整。

首先进行试运行：

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

运行批处理：

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

非试运行批处理需要 `--yes`，因为它可能连接多个数据库或解码结构化文件样本。每个子源都会获得自己的审计文件。

设置 `continue_on_error = true` 时，工具会继续处理其余源，并原子发布包含 `errors.txt` 的诊断包。命令仍以非零状态退出：全部源失败时为 `DBP1115E`，部分源失败时为 `DBP1116E`。部分包用于审查和重试，不代表完整采集成功。

试运行和实际执行都会在访问任何源之前验证完整清单。未知字段、重复 ID、经过安全文件名规范化后发生冲突的 ID、与源类型不兼容的字段、含糊的数据库连接源、无效的数据集模式以及为零的压缩采样预算都会被拒绝。每个 `source.id` 必须唯一、前后无空格，并且规范化后不超过 120 个 ASCII 字节。

## 结构化文件数据集模式

对于 Parquet 和 Avro 源：

- `single_file` 要求解析后恰好得到一个文件，并将其保留为一个逻辑表。
- `one_table_per_file` 将每个文件映射为一个子 Blueprint 文件中的单独匿名标签表。
- `merge_same_schema` 在列数匹配时，将多个文件合并为一个逻辑表。
- `partitioned_dataset` 的行为类似于 `merge_same_schema`。

合并检查被刻意设计得较为保守。它要求匿名化列布局、规范类型和原生类型、可空性、声明宽度、精度和小数位数、无符号和 `BIT(n)` 语义、时间戳精度、字符集和排序规则，以及结构化源语义均保持一致。对于高风险的数据湖规划，即使该结构检查通过，也应按已知模式对数据集进行分组。

## 捆绑包操作

列出源：

```bash
dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

开头几行报告 `aggregation`、物理 `sources`、`logical_datasets`、聚合总计和
`limitations`。组行显示 `relationship`、`members_complete` 和源 ID。源行显示
`dataset_relationship`、`dataset_group` 和 `dataset_scope`。应把
`aggregation=suppressed` 视为检查或修正清单的指令，而不是零大小环境。

列出一个带标签的源子集：

```bash
dbwarp-blueprint \
  --bundle-list customer-blueprint-bundle/bundle.toml \
  --select tag=erp
```

提取一个源：

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

从一个源提取一个表：

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

支持的选择器键为：

- `source=ID`
- `table=ID`
- `engine=postgresql|mysql|sqlserver|parquet|avro`
- `tag=NAME`

选择器可以作为一个以逗号分隔的字符串传递，也可以使用重复的 `--select` 标志。相同键的冲突值会被拒绝。

## 组件复杂性在包中。

每个子项目的复杂性仍然是该项目自身的属性。它永远不会被汇总、平均，或者分配到整个项目组的级别：因为不同的源可能使用不同的引擎、分析器版本、方言、语法配置、范围和评估对象，因此进行汇总是没有实际意义的。请独立比较每个源。

## 隐私和审阅边界

捆绑包不会放宽隐私模型：

- 实时数据库源仍会生成由秘密密钥保护的匿名表/列/索引 ID；
- 仅当启用 `--measure-compression --yes` 时才会解码 Parquet 值；Avro 采集始终遍历容器记录以得出计数、长度和空值比例，即使未进行压缩测量也是如此；
- 解码后的样本仅保留在内存中；
- bundle 元数据使用您选择的源 ID 和标签；
- 任何捆绑包命令都不会发送遥测或上传文件。

您可以在分享捆包之前，移除任何子蓝图或源条目。
