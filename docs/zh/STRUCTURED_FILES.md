# 结构化文件 Blueprint 源

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../STRUCTURED_FILES.md)。本译文不应被视为合同级文本。

**语言：** [English](../STRUCTURED_FILES.md) | [Deutsch](../de/STRUCTURED_FILES.md) | [Français](../fr/STRUCTURED_FILES.md) | [Español](../es/STRUCTURED_FILES.md) | [Polski](../pl/STRUCTURED_FILES.md) | [日本語](../ja/STRUCTURED_FILES.md) | **简体中文**

当源是文件而不是实时数据库时，`dbwarp-blueprint` 可以从本地 Parquet 和 Avro 输入构建有界、匿名化的 Blueprint TOML。

这是一种离线模式：

- 无数据库连接；
- 无凭据；
- 无遥测；
- 不会将行值写入输出；
- 表和列标识符仅输出为 `table-NNN` 和 `col-N`；
- 审计会记录本地输入/输出路径、输出哈希，以及模式、时间、采样工作量和警告等正常
  运维证据；此模式下不记录数据库端点。

## Parquet

```bash
dbwarp-blueprint \
  --from-parquet /data/orders.parquet \
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
  --from-avro /data/events.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Avro 对象容器不会公开 Parquet 风格的页脚行数。因此，Avro 模式会遍历容器一次，以统计记录数、推导逻辑 `table_bytes`，并观测逐列 `len_avg`、`len_p95` 和 `null_fraction`。写入器模式提供逻辑类型元数据。`storage_bytes` 和 `ratio_storage` 描述 Avro 容器，而不是 DBWarp 传输估算。

## 逻辑类型保真度

结构化文件捕获保留了用于估算所需的数据，包括：十进制数 precision/scale、日期和时间类型、时间戳精度和 UTC/local 语义、UUID、固定大小的二进制数据、UTF-8 字符串以及原始字节。 仅包含 NULL 值的字段仍然保持 `type = "null"` 的状态，而不是被转换为合成文本。

嵌套的 Parquet 结构、Avro 数组、映射、记录或多类型联合，无法表示为单个精确的 SQL 标量值。Blueprint 会记录一个规范化的 `json` 类型，以及 `source_semantics` 这样的元素，例如 `"repeated-leaf"`、`"nested-json"` 或 `"multi-type-union"`。这些列以 JSON 格式存储；嵌套的结构不会被完全复制。

源文件名主干、Parquet 路径、Avro 字段名和批处理 `logical_table` 标签不会写为 Blueprint 标识符。多文件数据集会输出由秘密密钥保护的 `table-NNN` 标识符，聚合对象字节数、分区数、行组数、编解码器、宽度、空值比例和兼容的压缩来源，并拒绝结构化逻辑列契约不同的文件。

## 解码后的压缩采样

结构化文件模式支持可选的解码后压缩采样：

```bash
dbwarp-blueprint \
  --from-parquet /data/orders.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

相同标志也适用于 `--from-avro`。

启用后，`dbwarp-blueprint` 会：

- 从文件解码最多 `--sample-rows` 条记录；
- 使用实时数据库 Blueprint 采集所用的同一瞬态
  `blueprint-compression-probe-v2` 表示来编码采样值；
- 输出表级和逐列 zstd-3 压缩摘要；
- 在生成的 TOML 中记录 `sample_encoding = "blueprint-compression-probe-v2"`；
- 仅在内存中保存采样字节，绝不会将行值写入磁盘。

`--measure-compression` 需要 `--yes`，因为它读取解码后的数据值。它会持久化聚合压缩、空值密度、cardinality/frequency、长度和样式等测量值，而不是采样值。

当前的采样器使用一种确定性的前N个样本。 这种方法可重复且成本低，但如果文件已排序或聚类，则可能会产生偏差。 对于重要的估算，建议使用具有代表性的文件，或者从不同的分片生成多个 Blueprint 文件。

## 范围

结构化文件 Blueprint 模式适用于：

- 在 DBWarp 运行前估算 Parquet/Avro 导入大小；
- 计划将数据从 Parquet/Avro 迁移到数据库。

当真实源为受支持的数据库，即 PostgreSQL、MySQL 或 SQL Server 时，它不能替代实时数据库 Blueprint 采集。数据库目录包含通用文件元数据中不存在的索引、键、FK、统计信息新鲜度和引擎布局详细信息。
