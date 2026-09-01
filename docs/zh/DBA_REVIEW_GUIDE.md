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
- 无 DDL 权限；
- 无超级用户/管理员角色；
- 读取权限仅限于正在评估的数据库；
- 密码或令牌通过文件或提示提供，不内嵌于 URI。

具体授权因数据库引擎和客户策略而异。如果账户无法读取某些目录视图或从某些表采样，工具应明确失败或输出信息较少的 Blueprint；请保留审计日志。

请使用 [`../../sql/grants/README.md`](../../sql/grants/README.md) 中感知版本的脚本和注意事项。
经批准的采集完成后，请使用 `sql/revoke/` 下匹配的脚本删除专用采集器账户；
执行前，请审查准确的数据库、主机模式、角色和登录名目标。

## Tier 1：仅目录

未提供 `--measure-compression` 时，Tier 1 为默认模式。

它读取：

- 引擎版本；
- 表列表和匿名化排序输入；
- 近似行数；
- 表和索引大小；
- 列类型系列、可空性，以及可用时的舍入长度统计信息；
- 索引类型、唯一性和匿名化列序号；
- 可用时的外键图结构；
- 可选的客户侧 RTT 探测，除非设置了 `--no-rtt-probe`。

它不读取行值。

## 非表对象清单

从模式 v4 开始，Blueprint 会独立于行采样清点非表对象。默认的 `--artifact-detail summary` 读取对象目录但不读取定义，只输出有界计数和外部前提类别。

`--artifact-detail graph --yes` 添加匿名对象 ID 和依赖边。`--artifact-detail analyzed --yes` 还会临时读取可用定义，并且只输出有界的词法特征和复杂度区间。定义文本、源对象名称、端点、提供商字符串、主体、秘密、密钥、证书、包名称和二进制文件绝不会被序列化。

目录权限会影响“不存在”的结论。请检查 `visibility`、`inventory_complete`、`dependencies_complete`、`catalogs_unreadable` 和 `families_not_inventoried`；当这些字段披露缺口时，不要把零计数解释为证据。`DBP1410W` 表示某个可选对象目录无法读取。

匿名依赖拓扑仍可能识别应用程序。只有在此风险可接受时才批准 `graph` 或 `analyzed`。参见 [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)。

## Tier 2：压缩测量

只有通过以下显式选项组合才能启用 Tier 2：

```bash
--measure-compression --yes
```

Tier 2 还会将有界行样本读入进程内存。采样字节会编码到内部行帧缓冲区中，使用 zstd 级别 3 在本地进行压缩，汇总为舍入后的比率，然后丢弃。

采样字节：

- 不会写入 `blueprint.toml`；
- 不会写入审计日志；
- 不会写入临时文件；
- 除数据库连接外，不会通过任何网络发送；
- 在汇总样本后不会保留。

Tier 2 很有价值，因为 DBWarp 性能和出口成本取决于压缩后字节数，而不是原始表字节数。

## RTT 探测

默认情况下，工具会在连接建立后运行五次 `SELECT 1` 查询。由此输出的 `[network]` 块包含 `connect_total_ms`、`query_rtt_ms_p50` 和 `query_rtt_ms_p95`。

该探测用于帮助运维人员了解 Blueprint 工具相对于源数据库的运行位置。它不是迁移 WAN RTT。

使用以下选项禁用：

```bash
--no-rtt-probe
```

## 读取的文件

运行时，工具只读取命令行上显式命名的文件，例如密码文件、用户文件、TLS CA/证书/密钥文件、Entra 令牌文件或 `--from-toml` 输入文件。

它刻意不读取 `~/.pgpass`、`~/.my.cnf`、云凭据文件、SSH 密钥、shell 历史记录或默认密码环境变量等常见的隐式凭据位置。

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

- 文件头是固定的 `dbwarp-blueprint v6` 文件头；
- 表 ID 的形式类似 `table-001`；
- 列 ID 的形式类似 `col-1`；
- 模式 ID 的形式类似 `schema-A`；
- 不包含真实的表名、列名、索引名、模式名或用户名；
- 不存在非表对象名、定义文本、端点字符串、凭据、密钥/证书材料、包名称或二进制文件；
- 不包含行值；
- 数值已按 [`../FORMAT.md`](FORMAT.md) 中记录的方式舍入；
- 可选压缩部分只包含比率和样本元数据。
- 对象完整性字段披露受限可见性、不可读目录和已知未建模类别。

默认 balanced MySQL 输出包含精确的声明容量和索引前缀长度，以及相对舍入的平均值/p95 样本。请显式审查这三个保真度标记。如果使用了 `--length-fidelity exact --yes`，还需批准精确采样统计信息。输出仍不得包含行值和真实对象名称。缺少保真度标记表示旧版/未知元数据，不得将其视为基准测试就绪的元数据。

该标记并不声称采样覆盖了每一个表。基准测试交接还必须在估算器清单中显示未采样的可变宽度索引列数量为零；如果此门控失败，请增加 `--max-wall-secs` 并重新采集。

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
