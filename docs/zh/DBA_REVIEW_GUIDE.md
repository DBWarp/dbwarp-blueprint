# DBA 审查指南

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../DBA_REVIEW_GUIDE.md)。本译文不应被视为合同级文本。

**语言：** [English](../DBA_REVIEW_GUIDE.md) | [Deutsch](../de/DBA_REVIEW_GUIDE.md) | [Français](../fr/DBA_REVIEW_GUIDE.md) | [Español](../es/DBA_REVIEW_GUIDE.md) | [Polski](../pl/DBA_REVIEW_GUIDE.md) | [日本語](../ja/DBA_REVIEW_GUIDE.md) | **简体中文**

本指南面向正在决定是否在生产环境或类生产环境中运行 `dbwarp-blueprint` 的 DBA 和安全审查人员。

## 执行模型

`dbwarp-blueprint` 是本地命令行二进制文件。在实时模式下，它会针对您提供的 URI 打开一个数据库连接，并写入一个本地 TOML 文件。它不会联系 DBWarp 基础设施、云 API、遥测端点、许可证服务器或更新服务器。

在 `--from-toml` 演示文稿模式下，它完全不会连接数据库。

## 建议使用的账户

请使用专用的低权限账户：该账户具有目录元数据的读取权限；如果启用了 Tier 2 压缩，还需具有从用户表采样行的权限。

建议属性：

- 无写入权限；
- 无 DDL 权限；唯一例外是审查人员明确批准 MySQL enhanced 采集，因为其
  `TRIGGER` 和 `EVENT` 元数据权限可执行 DDL；
- 无超级用户/管理员角色；
- 读取权限仅限于正在评估的数据库；
- 密码或令牌通过文件或提示提供，不内嵌于 URI。

具体的权限因数据库引擎和您的策略而异。如果账户无法读取某些目录视图或抽取某些表的数据，则该工具会明确失败或生成一个简化的Blueprint；请保留审计日志。

请使用 [`../../sql/grants/README.md`](../../sql/grants/README.md) 中感知版本的脚本和注意事项。
经批准的采集完成后，请使用 `sql/revoke/` 下匹配的脚本删除专用采集器账户；
执行前，请审查准确的数据库、主机模式、角色和登录名目标。

## Tier 1：仅元数据（不采样行）

未提供 `--measure-compression` 时，Tier 1 为默认模式。

它读取：

- 引擎版本；
- 表列表和匿名化排序输入；
- 近似行数；
- 表和索引大小；
- 列类型系列、可空性，以及可用时的舍入长度统计信息；
- 索引类型、唯一性和匿名化列序号；
- 可用时的外键图结构；
- 数据库端点尽可能返回的粗粒度源容量区间；
- 在默认 `--artifact-detail summary` 下，从对象目录读取有界的非表对象和
  外部前提计数（不读取定义）；
- 可选的 RTT 探测，除非设置了 `--no-rtt-probe`。

它不读取行值。

## 源环境

schema v7 的 `[source_environment]` 块仅根据所选数据库连接返回的值生成。
采集器绝不会检查自身主机，也不会把该工作站当作数据库服务器。

PostgreSQL 和 MySQL 暴露了一个数据库缓冲区设置，该设置在正常的最小权限范围内，因此内存是部分证据，其依据是 `database-buffer-cache`，而 CPU 的情况仍然未知。

SQL Server 仅在 `--artifact-detail graph` 或 `analyzed`（增强型模式）下请求源环境的容量。基本模式和标准模式不会发出操作系统容量查询，并将容量范围记录为 `not-requested`。

增强的脚本授予所需的服务器范围内的`VIEW SERVER STATE` (2019) 或 `VIEW SERVER PERFORMANCE STATE` (2022/2025) 权限，这些权限在一个单独的批处理中授予，DBA 可以移除。如果增强的捕获无法读取 DMV，则捕获继续，并记录该目录为无法读取，而不是使用本地机器的值或虚构容量。

此采集路径不会连接任何云、Kubernetes、虚拟机监控程序或操作系统 API。

## 非表对象清单

蓝图独立于行采样来统计非表对象。默认情况下，`--artifact-detail summary` 读取对象目录，但不读取定义，并且只输出有限数量的计数和外部依赖类。

`--artifact-detail graph --yes` 添加匿名对象 ID 和依赖边。`--artifact-detail analyzed --yes` 还会临时读取可用定义，并且只输出有界的词法特征和复杂度区间。定义文本、源对象名称、端点、提供商字符串、主体、秘密、密钥、证书、包名称和二进制文件绝不会被序列化。

目录权限会影响“不存在”的结论。请检查 `visibility`、`inventory_complete`、
`dependencies_complete`、`requirements_complete`、`catalogs_unreadable` 和
`families_not_inventoried`；当这些字段披露缺口时，不要把零计数或空要求列表解释
为证据。在 graph/analyzed 详细级别，还应检查每个对象的 `requirement_status`：
只有 `complete` 才能使空列表成为该对象零要求的证据。`partial` 会保留已知事实，
但不会声称覆盖完整；`unavailable` 表示未建立可用覆盖。在这两种情况下，根据该
对象要求得出的耦合评估仍为未知。`DBP1410W` 表示某个可选对象目录无法读取。

匿名依赖拓扑仍可能识别应用程序。只有在此风险可接受时才批准 `graph` 或 `analyzed`。参见 [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)。

## Tier 2：压缩测量

只有通过以下显式选项组合才能启用 Tier 2：

```bash
--measure-compression --yes
```

第二层级还会将有限数量的行样本读取到进程内存中。这些样本的字节被编码到内存缓冲区中，并用于推导出聚合压缩率、空值密度、cardinality/frequency、长度和样式等指标，然后在丢弃这些值和临时指纹之前进行计算。

采样字节：

- 不会写入 `blueprint.toml`；
- 不会写入审计日志；
- 不会写入临时文件；
- 除数据库连接外，不会通过任何网络发送；
- 在汇总样本后不会保留。

第二层（Tier 2）很有价值，因为传输时间和出站成本取决于压缩后的字节数，而不是原始表的大小。

## RTT 探测

默认情况下，工具会在连接建立后运行五次 `SELECT 1` 查询。由此输出的 `[network]` 块包含 `connect_total_ms`、`query_rtt_ms_p50` 和 `query_rtt_ms_p95`。

该探测用于帮助运维人员了解 Blueprint 工具相对于源数据库的运行位置。它不是迁移 WAN RTT。

使用以下选项禁用：

```bash
--no-rtt-probe
```

## 读取的文件

运行时，工具只读取命令行上显式选择的文件，或由显式选择的批处理清单或
捆绑包引用的文件。其中可包括密码文件、用户文件、匿名化密钥文件、TLS
CA/证书/密钥文件、Entra 令牌文件、结构化文件输入，以及 Blueprint 或
捆绑包输入。

它刻意不读取 `~/.pgpass`、`~/.my.cnf`、云凭据文件、SSH 密钥、shell 历史记录或默认密码环境变量等常见的隐式凭据位置。

上述说明仅涵盖应用程序自身的凭据发现。数据库、TLS、DNS 和集成身份验证
库仍可能读取操作系统信任存储、配置和凭据缓存。主机策略要求时，应单独
审查或跟踪这些平台依赖项。

完整列表请参阅 [`../AUDIT.md`](AUDIT.md)。

## 写入的文件

工具只写入当前模式所选择的路径：

- 实时模式下的 `--out` Blueprint TOML；
- 请求时的 `--deck`；
- 请求时的 `--audit-log`；
- 批处理模式下的 `--out-dir`：`bundle.toml`、`blueprints/`、`audits/`、
  所有权标记，以及需要报告部分失败时的 `errors.txt`；
- 每次运行时输出到 stderr 的审计日志。

它不会使用操作系统的隐式临时目录。原子批处理发布可能会在
`--out-dir` 旁创建相邻的暂存或恢复目录；发生可处理的故障时，会删除
该目录或恢复先前的捆绑包。

## 输出审查清单

分享 `blueprint.toml` 前，请验证：

- 文件头是固定的 `dbwarp-blueprint v7` 文件头；
- 表 ID 的形式类似 `table-001`；
- 列 ID 的形式类似 `col-1`；
- 模式 ID 的形式类似 `schema-A`；
- 不包含真实的表名、列名、索引名、模式名或用户名；
- 不存在非表对象名、定义文本、端点字符串、凭据、密钥/证书材料、包名称或二进制文件；
- 不包含行值；
- 数值使用 [`../FORMAT.md`](FORMAT.md) 中记录的精确或舍入精度；将精确的
  选择性加入字段视为更敏感的数据进行审查；
- 可选的采样派生部分只包含聚合的压缩、空值密度、基数/频率、长度、
  样式和采样来源元数据，绝不包含采样值；
- 对象完整性字段披露受限可见性、不可读目录和已知未建模类别。

默认的平衡输出 MySQL 包含精确声明的容量和索引前缀长度，以及相对四舍五入的平均值/p95 样本。请仔细审查三个精确度标记。如果使用了 `--length-fidelity exact --yes`，请也批准精确的样本统计数据。行值和实际对象名称仍然必须被省略。一个没有精确度标记的 Blueprint 是由旧版本生成的；请重新采集它。

该标记不声称采样覆盖了所有表。如果报告了`DBP1406W`，请增加`--max-wall-secs`的值，并重新采集数据。

## 运行安全

建议的首次运行：

```bash
--sample-rows 500 --max-wall-secs 120
```

批准后的建议生产式运行：

```bash
--sample-rows 1000 --max-wall-secs 300
```

如果生产策略禁止在主库上采样，请从只读副本运行。
