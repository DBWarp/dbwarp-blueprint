# 结构化文件 Blueprint 源

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../STRUCTURED_FILES.md)。本译文不应被视为合同级文本。

**语言：** [English](../STRUCTURED_FILES.md) | [Deutsch](../de/STRUCTURED_FILES.md) | [Français](../fr/STRUCTURED_FILES.md) | [Español](../es/STRUCTURED_FILES.md) | [Polski](../pl/STRUCTURED_FILES.md) | [日本語](../ja/STRUCTURED_FILES.md) | **简体中文**

当源是文件而不是实时数据库时，`dbwarp-blueprint` 可以从本地 Parquet 和 Avro 输入构建净化后的 Blueprint TOML。

这是一种离线模式：

- 无数据库连接；
- 无凭据；
- 无遥测；
- 不会将行值写入输出；
- 表和列标识符仅输出为 `table-NNN` 和 `col-N`；
- 审计仅记录本地输入/输出文件路径和输出哈希。

## Parquet

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

Parquet 模式读取页脚和行组元数据。它会推导：

- 根据文件元数据得出的行数；
- 根据 Parquet 物理/逻辑类型得出的列类型标签；
- 根据定义级别得出的可空性；
- 完整列统计可用时观测到的空值比例；
- 根据列块元数据得出的粗略编码平均宽度和逐列源存储比率；
- 源对象字节数、行组数、分区数和编解码器来源。

仅使用元数据的 Parquet 采集不会虚构解码后的 p95 宽度。可选的解码采样会用解码后的 `len_avg`、`len_p95`、`null_fraction` 和逻辑 `table_bytes` 观测值替换编码宽度提示。

仅使用元数据的 Parquet 将未压缩列块字节作为逻辑 `table_bytes` 估计值。
表级 `ratio_storage` 将该值与对象实际大小比较；列级 `ratio_storage` 比较未压缩和压缩列块
字节。这些是文件规划信号，不是 DBWarp 传输压缩，也绝不会输出为
`ratio_zstd_3`。

## Avro

```bash
dbwarp-blueprint \
  --from-avro /data/customer-sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Avro 对象容器不会公开 Parquet 风格的页脚行数。因此，Avro 模式会遍历容器一次，以统计记录数、推导逻辑 `table_bytes`，并观测逐列 `len_avg`、`len_p95` 和 `null_fraction`。写入器模式提供逻辑类型元数据。`storage_bytes` 和 `ratio_storage` 描述 Avro 容器，而不是 DBWarp 传输估算。这适用于估算器和合成测试数据集规划。

## 逻辑类型保真度

结构化文件采集会保留估算器所需的有界逻辑元数据：十进制精度和小数位数、日期和时间类型系列、时间戳精度及 UTC/本地语义、UUID、固定二进制宽度、UTF-8 字符串和原始字节。纯空字段保持为 `type = "null"`，而不会变成合成文本。

嵌套 Parquet 叶节点以及 Avro 数组、映射、记录或多类型联合无法表示为单个精确 SQL 标量。Blueprint 会记录标准化的 `json` 类型和 `source_semantics`，例如 `"repeated-leaf"`、`"nested-json"` 或 `"multi-type-union"`。下游生成器必须将这些值标识为有代表性的 JSON 压力，不得声称嵌套模式能够精确往返。

源文件名主干、Parquet 路径、Avro 字段名和批处理 `logical_table` 标签不会写为 Blueprint 标识符。多文件数据集会输出确定性的 `table-NNN` 标识符，聚合对象字节数、分区数、行组数、编解码器、宽度、空值比例和兼容的压缩来源，并拒绝结构化逻辑列契约不同的文件。

## 解码后的压缩采样

结构化文件模式支持可选的解码后压缩采样：

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

相同标志也适用于 `--from-avro`。

启用后，`dbwarp-blueprint` 会：

- 从文件解码最多 `--sample-rows` 条记录；
- 使用实时数据库 Blueprint 采集所用的同一 `dbwarp-blueprint-rowframe-v1` 行帧编码采样值；
- 输出表级和逐列 zstd-3 压缩摘要；
- 在生成的 TOML 中记录 `sample_encoding = "dbwarp-blueprint-rowframe-v1"`；
- 仅在内存中保存采样字节，绝不会将行值写入磁盘。

`--measure-compression` 需要 `--yes`，因为它会读取解码后的客户值，即使只持久化聚合比率也是如此。

当前采样器使用确定性的前 N 条样本。这样可重现且开销低，但如果文件经过排序或聚集，则可能存在偏差。对于高风险估算，请优先选择有代表性的文件，或从不同分片生成多个 Blueprint 文件。未来版本可能会加入行组/块分层采样。

## 范围

结构化文件 Blueprint 模式适用于：

- 在 DBWarp 运行前估算 Parquet/Avro 导入大小；
- 根据文件元数据生成不含客户数据的合成测试数据集；
- 规划 Parquet/Avro -> DBWarp columnar -> 目标数据库流程。

当真实源为受支持的数据库，即 PostgreSQL、MySQL 或 SQL Server 时，它不能替代实时数据库 Blueprint 采集。数据库目录包含通用文件元数据中不存在的索引、键、FK、统计信息新鲜度和引擎布局详细信息。
