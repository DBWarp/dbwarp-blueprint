# 压缩测量

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../COMPRESSION_MEASUREMENT.md)。本译文不应被视为合同级文本。

**语言：** [English](../COMPRESSION_MEASUREMENT.md) | [Deutsch](../de/COMPRESSION_MEASUREMENT.md) | [Français](../fr/COMPRESSION_MEASUREMENT.md) | [Español](../es/COMPRESSION_MEASUREMENT.md) | [Polski](../pl/COMPRESSION_MEASUREMENT.md) | [日本語](../ja/COMPRESSION_MEASUREMENT.md) | **简体中文**

`dbwarp-blueprint` 可以选择测量有代表性的表数据的压缩效果。这能提高 DBWarp 估算的准确性，因为 WAN 传输时间和出口流量成本取决于压缩后字节数，而不是原始表大小。

压缩测量为选择加入功能，并且需要明确同意。交互式实时运行可接受预检提示；无人值守运行和结构化文件使用：

```bash
--measure-compression --yes
```

禁用压缩测量时，实时数据库采集不会采样用户表的行值。
结构化文件的行为有所不同：Avro 仍必须遍历记录，以收集行数、长度和空值元数据；
请参阅[结构化文件](STRUCTURED_FILES.md)。

## 采样内容

对于每个未被安全证明为空的合格用户表，工具会将有界数量的行读入内存，编码为稳定的
瞬时探针缓冲区，在本地使用 zstd 级别 3 压缩这些缓冲区，并派生汇总压缩、NULL 密度、
基数／频率、长度和样式测量值，之后丢弃采样值及临时指纹。

对于选定的 text/binary 列，Tier 2 也可以单独对该列进行抽样。 这样可以获得每个列的压缩率，而不仅仅是表级别的平均值。

实时数据库表的数据比例使用一个中性的、固定大小的1000行分组，每个分组包含一个描述符，每列具有固定的值长度，并且数据按列连续存储。 这种方式衡量了与压缩相关的结构，而不会捕获任何数据库或传输协议的信息。 每列的比例保留了`blueprint-compression-probe-v2`，其带有标签的、长度前缀的值仍然是更具体的熵输入。

PostgreSQL 表的块使用 `blueprint-columnar-transfer-probe-v2`，它将行组通过一个持久的 zstd 级别 3 上下文，并在每个组之后刷新。MySQL 和 SQL Server 使用 `blueprint-columnar-transfer-probe-v3`：相同的中性字节和持久上下文，并在 256 KiB 的探测块边界处进行额外的刷新。SQL Server 的 `nvarchar`、`nchar` 和 `ntext` 样本被测量为 UTF-16LE 字节分布。SQL Server 的 `varchar`、`char` 和 `text` 保留其采样的窄字节宽度；Blueprint 记录源排序规则的目录代码页为 `utf-8`、`windows-N` 或 `code-page-N`。数据库驱动程序仍然将解码后的字符串暴露给采样器，因此不声称具有旧代码页的字节身份。表 `ratio_stddev` 跨外部行组输出进行测量。每个列的投影块仍然是独立的、一次性的熵测量，并输出 `0.0`。较早版本的 Blueprint 可能包含 `blueprint-columnar-transfer-probe-v1`；具有不同标签的比例不可比较。

采样字节只会通过所选数据库会话进入本地进程。它们不会写入磁盘，不会包含在
`blueprint.toml` 或审计日志中，不会上传，也不会发送到 DBWarp 基础设施。

## 本地 worker 并发

数据库采样始终使用一个串行连接。可选的 `--compression-workers N` 设置只并行
压缩已经读取到内存中的样本。它接受 1–32 个 worker，默认值为 1，以尽量减少对源主机的影响。要使用更多本地 CPU，请显式增大该值：

```bash
--measure-compression --yes \
--compression-workers 4
```

当 zstd 是瓶颈时，更高的值可以缩短耗时，但会增加本地 CPU 和峰值内存。它不会
创建并发数据库采样连接。每个 worker 拥有自己的 zstd 上下文，输入队列上限等于
worker 数。worker 数量不会改变测量值。匿名标签顺序会因默认的新密钥而有意变化；
只有在获批的跨运行比较中才应重复使用受保护的 `--anonymization-key-file`。

只有当引擎维护的目录值能够安全证明表在目录读取时为空，采集器才会跳过行和样式
查询。PostgreSQL 要求最近分析的统计信息且之后没有修改；SQL Server 使用分区行
计数器。MySQL 表行估计可能对非空表报告零，因此采集器不会用它跳过采样。这种
保守差异可保护保真度。

## Blueprint 文件中出现的内容

只会输出汇总摘要。对于类似文本的列，Tier 2 过程可能输出有界样式标签，例如 `json`、`xml`、`natural-text`、`base64`、`hex`、`numeric-text` 或 `mixed`。

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
sample_method = "LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "blueprint-compression-probe-v2"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "blueprint-columnar-transfer-probe-v3"
```

这些值用于估算网络传输的大小。

## 重要性

两个原始表大小相同的数据库在迁移期间可能表现得截然不同：

- JSON、XML、重复的业务代码、稀疏文本和自然语言文本通常可以很好地压缩。
- 加密值、已经压缩的 blob、随机令牌和高熵二进制数据无法很好地压缩。
- SQL Server Unicode 文本与窄文本具有不同的字节分布。采样器将 `nvarchar` 建模为 UTF-16LE，并记录解释 `varchar` 所需的排序规则代码页，而不是将每个文本列都视为 UTF-8。

少量本地测量通常比根据列类型猜测更有用。

## 偏差和透明度

某些引擎不提供完全均匀的表采样。当可使用该访问路径时，MySQL 会将有界样本分散到四个数值主键范围；
否则回退到 `LIMIT N`。两者都会显式标记为有偏，因为它们都不是统计随机样本。除最后一个范围窗口外，每个窗口
都有排他的上界。稀疏或偏斜的主键区域可能使窗口不足，但窗口不会再次读取下一个窗口中的行。其他不太理想的
引擎回退也通过 `sampled_with_bias` 和 `bias_reason` 记录。

Blueprint 记录了有限样本的布局，并将其与相关的文本字段分开。MySQL 数值主键范围抽样会输出 `sample_layout = "primary-key-range-windows"`，并且对每个窗口按照完整的键进行排序。

带有偏差的样本仍然有用，但置信度较低。审计日志记录了已启用行采样以及本地编码的探测字节计数。数据库会话的字节总数报告为 `unknown`，当驱动程序不提供这些信息时。

## 实际采样设置

首次生产安全过程：

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

当存在读取副本或维护窗口时，可以获得更准确的测量结果：

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

大型数据库不需要巨量样本。目标是获得稳定的压缩信号，而不是精确的行级分析。`--max-wall-secs` 是整个实时采集的硬性截止时间，包括连接、目录、RTT 和采样，不会为每个阶段重新计时。

实时数据库采样还对每个表设置不可配置的 16 MiB 投影负载上限。
初始 SQL 投影按类型分配预算，并单独观察原始八位组长度。当投影值被缩窄时，
MySQL 和 SQL Server 可以减少行数，并调整逐列限制后重试，但仍必须符合预算。
宽度超出该预算的值仍只返回有界前缀；压缩和值汇总的来源信息会记录此限制，
而长度统计则按照所选的长度保真度策略，保留服务器报告的原始采样值长度。

该上限不是网络字节数或进程内存的限制。协议编码、原始长度元数据、重试和驱动缓冲区都会增加开销。
审计会记录配置的负载上限、执行的查询以及在本地编码的确切探针字节总数；
它并不报告实测的数据库网络流量。

## 如何解读这些测量结果。

`sample_encoding` 字段是合同的一部分。 比例只能在同一个编码标签内进行比较，因为不同的采样编码可能会对相同的数据产生不同的压缩比例。 特别是，表级别的列式传输比例和每个列的 v2 比例是互补的测量值，不能互相替代。
