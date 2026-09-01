# 压缩测量

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../COMPRESSION_MEASUREMENT.md)。本译文不应被视为合同级文本。

**语言：** [English](../COMPRESSION_MEASUREMENT.md) | [Deutsch](../de/COMPRESSION_MEASUREMENT.md) | [Français](../fr/COMPRESSION_MEASUREMENT.md) | [Español](../es/COMPRESSION_MEASUREMENT.md) | [Polski](../pl/COMPRESSION_MEASUREMENT.md) | [日本語](../ja/COMPRESSION_MEASUREMENT.md) | **简体中文**

`dbwarp-blueprint` 可以选择测量有代表性的表数据的压缩效果。这能提高 DBWarp 估算的准确性，因为 WAN 传输时间和出口流量成本取决于压缩后字节数，而不是原始表大小。

压缩测量为选择加入功能，并且需要明确同意。交互式实时运行可接受预检提示；无人值守运行和结构化文件使用：

```bash
--measure-compression --yes
```

如果没有这些标志，工具只读取目录元数据。

## 采样内容

对于每个用户表，工具会将有界数量的行读入内存，将其编码为确定性的行帧缓冲区，在本地使用 zstd 级别 3 压缩该缓冲区，记录舍入后的比率，然后丢弃缓冲区。

对于选定的文本/二进制列，Tier 2 还可能单独采样该列。这使下游规划工具能够匹配每列熵，而不是只依赖表级平均值。

每次测量都是一个独立的单次 zstd 帧，并预先声明输入大小。比率方差（`ratio_stddev`）在同一缓冲区按行对齐的 64 KiB 块上测量，因此方差描述的是估算器所预测的传输，而不是对单个完整缓冲区的平均。由于预先声明了输入大小，zstd 会选择与估算器传输建模方式一致的大小自适应参数。对于小样本（大约低于 1 MiB），比率可能与早期版本（通过未声明大小的流式上下文测量）的捕获明显不同；跨越该边界的小表比率不可直接比较。与传输一致的，是声明了大小的测量。

采样字节不会写入磁盘，不会包含在 `blueprint.toml` 中，不会包含在审计日志中，也不会发送到任何位置，唯一例外是从数据库服务器传输到您运行的本地进程。

## 本地 worker 并发

数据库采样始终使用一个串行连接。可选的 `--compression-workers N` 设置只并行
压缩已经读取到内存中的样本。它接受 1–32 个 worker，默认值为 1，以尽量减少对源主机的影响。要使用更多本地 CPU，请显式增大该值：

```bash
--measure-compression --yes \
--compression-workers 4
```

当 zstd 是瓶颈时，更高的值可以缩短耗时，但会增加本地 CPU 和峰值内存。它不会
创建并发数据库采样连接。每个 worker 拥有自己的 zstd 上下文，输入队列上限等于
worker 数。输出顺序和 v6 Blueprint 值保持确定性。

只有当引擎维护的目录值能够安全证明表在目录读取时为空，采集器才会跳过行和样式
查询。PostgreSQL 要求最近分析的统计信息且之后没有修改；SQL Server 使用分区行
计数器。MySQL 表行估计可能对非空表报告零，因此采集器不会用它跳过采样。这种
保守差异可保护保真度。

## Blueprint 文件中出现的内容

只会输出摘要数值。对于类似文本的列，Tier 2 过程可能输出有界样式标签，例如 `json`、`xml`、`natural-text`、`base64`、`hex`、`numeric-text` 或 `mixed`。

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
sample_method = "column LIMIT N (engine-specific bounded sample)"
sampled_with_bias = true
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "dbwarp-blueprint-rowframe-v1"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

这些值可帮助经批准的下游工具估算网络传输大小，并生成具有相似可压缩性的合成文本/二进制数据。

## 重要性

两个原始表大小相同的数据库在迁移期间可能表现得截然不同：

- JSON、XML、重复的业务代码、稀疏文本和自然语言文本通常可以很好地压缩。
- 加密值、已经压缩的 blob、随机令牌和高熵二进制数据无法很好地压缩。
- SQL Server `nvarchar` 数据具有与 UTF-8 文本不同的字节分布，因此采样时会相应编码。

少量本地测量通常比根据列类型猜测更有用。

## 偏差和透明度

某些引擎不提供完全均匀的表采样。当工具回退到不太理想的方法时，Blueprint 文件会通过 `sampled_with_bias` 和 `bias_reason` 标记它。

有偏样本仍然有用，但下游工具应以较低置信度处理它们。审计日志会记录已启用行采样和本地编码的 row-frame 字节数。驱动不公开的网络字节数标为 `unknown`。

## 实际采样设置

首次生产安全过程：

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

有只读副本或维护窗口时，可使用更佳的估算器输入：

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

大型数据库不需要巨量样本。目标是获得稳定的压缩信号，而不是精确的行级分析。`--max-wall-secs` 是整个实时采集的硬性截止时间，包括连接、目录、RTT 和采样，不会为每个阶段重新计时。

实时数据库采样还对每个表设置不可配置的 16 MiB 投影负载上限。
SQL 投影会在驱动程序接收数据前于服务器上截断可变宽度单元格，并对异常宽的表减少行数上限。
因此，非常大的 LOB 值只贡献有界前缀，而非其完整内容。
审计会记录活动的表负载上限和在本地编码的确切行帧字节总数。

## 下游消费者如何使用它

下游消费者应按以下顺序使用压缩证据：

1. 可识别的逐列压缩块；
2. 可识别的表级压缩块；
3. 没有实测比率时的类型/样式默认值。

`sample_encoding` 字段是契约的一部分。消费者应只使用带有可识别编码标签的比率，因为不同的样本编码可能会对相同逻辑数据产生不同的压缩比率。
