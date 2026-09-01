<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../.github/assets/dbwarp-logo-dark.png">
    <img src="../../.github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

# dbwarp-blueprint

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../README.md)。本译文不应被视为合同级文本。

**语言：** [English](../../README.md) | [Deutsch](../de/README.md) | [Français](../fr/README.md) | [Español](../es/README.md) | [Polski](../pl/README.md) | [日本語](../ja/README.md) | **简体中文**

英文文档具有最高效力。机器翻译文档集可能在经过多次独立审查后单独提供，但仍可能包含错误。

## 产品简介

DBWarp Blueprint 是一款以信任为先的数据库 Blueprint 采集器。您可以在自己的环境中针对 PostgreSQL、MySQL 或 SQL Server 运行它。它会读取目录元数据；如果您要求测量压缩率，它还会读取有界行样本。随后，它会写出数据库的匿名化结构 Blueprint，其中包括表大小、行数、类型系列以及索引和外键结构。

标识符会替换为密钥化匿名标签，任何行值都不会写入 Blueprint。
默认情况下，新的进程本地密钥可防止离线字典检查；`--anonymization-key-file` 允许客户在经批准的多次对比运行之间保留标签。
分享任何输出前，请阅读 [`SECURITY.md`](SECURITY.md)：其中准确说明了各模式披露的内容，以及哪些选项会扩大披露范围。

输出是纯文本文件。您可以逐行阅读，然后再决定是否分享。

DBWarp Blueprint 免费且开源，并且完全在您的环境中运行。借助它，您可以向我们提供数据库的客观信息，而无需向我们提供数据库本身。

## 为什么要运行它

与我们分享 Blueprint 输出后，我们可以告诉您 DBWarp 能将数据迁移速度提高多少，以及这会如何影响迁移、CI/CD 测试数据和分析工作的时间计划。

距离最为关键。数据需要传输得越远，DBWarp 能带来的改善就越大。

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; 瑞士苏黎世

---

`dbwarp-blueprint` 是 DBWarp 的客户侧 Blueprint 采集器。在客户自己的环境中运行它，可生成经过脱敏且可审阅的 `blueprint.toml` 文件。DBWarp 可使用该文件进行迁移容量估算、合成测试数据集生成和预检规划，而无需获取数据库访问权限、转储、模式名称或行数据。

它可连接 PostgreSQL、MySQL 或 SQL Server，读取目录元数据，按需从有界行样本测量本地压缩率，并写出纯文本 TOML。如果输入已经是结构化数据文件而非实时数据库，它还可以在离线模式下从本地 Parquet 或 Avro 文件推导 Blueprint。您可以打开输出、逐行审阅，然后决定是否分享。

可选的 `--deck blueprint.pptx` 还会写出同一匿名化 Blueprint 的 PowerPoint 摘要。既可以在实时数据库运行期间生成演示文稿，也可以稍后使用 `--from-toml blueprint.toml --deck blueprint.pptx` 从已审阅的 TOML 文件生成。演示文稿生成器内置于 Rust 二进制文件中，不会建立任何网络连接。

## 用途

DBWarp 需要足够的结构信息来估算和规划传输：

- 表的数量；
- 近似行数；
- 表和索引大小；
- 列类型系列、精确的结构容量/索引前缀，以及默认按隐私策略舍入的观测宽度；
- 索引和外键结构；
- 隐私安全的非表对象计数和外部部署前提条件；
- 从少量本地样本得到的可选表级和列级压缩摘要；
- 可选的客户侧数据库 RTT 证据。

这些事实足以估算传输大小、选择 DBWarp 批量传输的初始方案，并生成具有代表性的合成基准测试数据集，但不足以重建客户的模式或数据。

## 不会执行的操作

`dbwarp-blueprint` 不会：

- 发送遥测；
- 调用 DBWarp 服务器；
- 上传 Blueprint 文件；
- 读取 `~/.pgpass`、`~/.my.cnf`、云凭据或 SSH 密钥；
- 读取 `PGPASSWORD` 或 `MYSQL_PWD` 等默认密码环境变量；
- 写入当前模式所选输出之外的任何内容；批处理模式会写入包含子 Blueprint、
  子审计和可选故障证据的捆绑包目录；
- 在输出中包含真实表名、列名、索引名或模式名、非表对象名、SQL 定义、外部端点、凭据、密钥、证书、二进制文件或行值。

实时 Blueprint 采集会与您指定的端点建立数据库会话。DNS 可能使用已配置的
解析器，集成 Kerberos/SSPI 身份验证可能会联系身份基础设施。批处理模式会针对
每个数据库源重复这一边界。本地 TOML、Parquet、Avro 和捆绑包操作不会由应用程序
主动发起网络连接。

## 下载或构建

| 方式 | 最适合 | 链接 |
|---|---|---|
| 下载二进制文件 | 快速试用、售前工程通话、隔离测试主机 | [`binaries/README.md`](BINARIES.md) |
| 从精简源代码克隆构建 | 安全审查、生产策略、可重现性检查 | [`BUILD.md`](BUILD.md) |
| 从带依赖源代码的发布包构建 | 严格的离线依赖审计 | GitHub Releases |

最重视信任的方式是从源代码构建。常规仓库保持精简，并使用 `Cargo.lock` 固定依赖版本。对于更严格的离线审计，每个版本还会发布一个包含所有依赖源文件的依赖完整源代码包。为方便使用，也提供带 SHA256 校验和的发布二进制文件。

## 快速开始

需要时可选择显示语言。默认语言为英语；二进制文件内嵌了完整的德语、法语、西班牙语、波兰语、日语和简体中文目录：

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --dry-run
```

只有面向用户的帮助、提示、诊断、进度和 PowerPoint 演示文稿标签会被翻译。命令和选项名称、可接受值、URI 方案、环境变量名称、选择器、DBP 代码、审计键以及生成的 TOML 始终使用规范英文标记。这样可确保所有语言下的自动化和支持流程完全一致。请参阅 [`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md)。

首先进行试运行。它只打印计划，不连接数据库：

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --dry-run
```

建议采用启用 TLS、审计日志和压缩测量的生产式运行：

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

使用 `--measure-compression --yes` 时，输出会包含表级 zstd 比率和逐列压缩预测。逐列块与表级比率使用相同的有界样本计算；它们用于 DBWarp 测试数据集估算，不会把采样值写入磁盘。模式 v3 及更高版本还会输出隐私安全的逐列基数与分布聚合，以及推断得到的索引前缀和关系摘要。临时指纹在内存中有明确上限并会被丢弃；值和指纹绝不会出现在 Blueprint TOML 中。

从模式 v4 开始，Blueprint 还会清点非表对象。默认的 `--artifact-detail summary` 不读取定义，
只按对象类别和外部前提类别保存有界计数。`graph` 添加匿名依赖拓扑，
`analyzed` 添加有界的语言特征和复杂度区间；两者都需要 `--yes`，因为即使
匿名图也可能识别应用程序：

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```


对象的存在是规划证据，并不表示 DBWarp 能自动重建或转换它。请参阅
[`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)。

### MySQL 长度保真度

默认的 `balanced` 策略会精确保留声明的字符/字节容量和索引前缀长度。采样得到的平均值/p95 值长度使用相对误差桶（最大误差约为 3.2%，不超过 32 字节的值精确保留）。这样可使通常为 9 个字符的 `VARCHAR(3000)` 键在生成数据中保持在约 9 个字符，同时保留有效的源 DDL/索引限制：

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

只有当策略允许提供额外精度时，才使用精确采样统计信息：

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

使用 `--length-fidelity strict` 可对声明长度、观测长度和前缀长度沿用旧版粗粒度、适合分享的分桶方式。Strict 模式会刻意牺牲测试数据集/索引保真度，不适合客户基准测试。旧版 `--preserve-exact-lengths --yes` 写法仍作为 `--length-fidelity exact --yes` 的兼容别名保留。

新 Blueprint 文件会分别记录 `declared_length_fidelity`、`index_length_fidelity` 和 `observed_length_fidelity` 字段。旧版 `length_metadata` 字段仍保留，以便与旧消费者进行保守兼容。PostgreSQL 字符容量使用精确的目录值；依赖编码的字节上限和索引前缀长度仍不可用。

对于具有客户代表性的生成式基准测试，`--measure-compression` 不是可选项：它提供观测到的平均值/p95 值长度，从而避免将真实值仅有几个字符的已声明数千字节键按其容量生成。默认采样墙钟预算为 300 秒。对于非常大的模式，请增加 `--max-wall-secs`。如果任何非空可变宽度索引列仍未采样，下游规划工具应拒绝该 Blueprint。此后若要进行冒烟或兼容性生成，必须在下游显式使用覆盖项，并必须标记为不具代表性。

然后审阅这些文件：

```bash
less blueprint.toml
less audit.txt
```

如果符合您的策略，可将 `blueprint.toml` 分享给 DBWarp。演示文稿经审阅后也可以
分享。审计日志包含端点、身份、路径和计时详细信息；除非特定支持案例要求通过已批准的
安全渠道提供，否则应将其作为受访问控制的运维证据保留。

## 结构化文件模式

如果源已经是本地结构化文件，无需数据库凭据即可生成 Blueprint TOML：

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

```bash
./dbwarp-blueprint \
  --from-avro /data/sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Parquet 模式读取页脚和行组元数据。Avro 对象容器没有等效的页脚行数，因此 Avro 模式会遍历容器统计记录数，并使用写入器模式获取列结构。两种模式都不会连接数据库，也不会读取凭据选项。

如果策略允许解码采样，文件模式还可以从有界本地样本估算 DBWarp 传输风格的压缩率：

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

相同选项也适用于 `--from-avro`。采样值会在内存中编码为 `dbwarp-blueprint-rowframe-v1`；只有聚合 zstd 压缩比会写入 Blueprint TOML。

## 批处理和捆绑包模式

对于多个数据库、多个表/数据集或客户数据资产审查，请使用批处理清单并写入捆绑包目录：

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

工作目录包含 `bundle.toml`、每个源的子 Blueprint 文件和受访问控制的每个源审计日志。
默认不要传输整个工作目录。您可以列出或提取其内容，也可以创建一个单独审阅的打包
Blueprint 捆绑包：

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
./dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 --out table-042.blueprint.toml
./dbwarp-blueprint --bundle-pack customer-blueprint-bundle --out customer-blueprint-bundle.packed.toml
```

有关清单语法、结构化文件数据集模式和选择器规则，请参阅 [`docs/BATCH_AND_BUNDLES.md`](BATCH_AND_BUNDLES.md)。

## 常用数据库命令

PostgreSQL：

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

MySQL：

```bash
./dbwarp-blueprint \
  --connect mysql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

SQL Server：

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@db.internal,1433/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

有关 Kerberos、SSPI 和 Entra ID 的示例，请参阅 [`AUTH.md`](AUTH.md)。有关内部 CA、mTLS 和主机名验证的信息，请参阅 [`TLS.md`](TLS.md)。

## 仅目录模式

如果策略禁止采样行，请省略 `--measure-compression`：

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

仅目录模式只读取元数据。DBWarp 仍可根据表大小、行数、类型系列和索引/FK 结构进行估算，但由于必须推断文本/二进制熵，压缩和合成测试数据集的真实性会较弱。

## 输出预览

```toml
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 6
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 28
row_count = 12500000
table_bytes = 4200000000
index_bytes = 1100000000

[tables.table-001]
rows = 12500000
table_bytes = 4200000000
index_bytes = 1100000000
schema = "schema-A"
has_clustered_index = false

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false

[tables.table-001.idxs.idx-1]
type = "btree"
primary = true
unique = true
cols = [1]
```

完整文件契约记录于 [`FORMAT.md`](FORMAT.md)。审计日志记录于 [`AUDIT.md`](AUDIT.md)。

## 可视化摘要演示文稿

在实时运行期间生成演示文稿：

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

也可以稍后从已审阅的 Blueprint 文件构建，不连接数据库：

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx
```

演示文稿会根据模式大小进行调整：小型模式显示逐表详情，大型模式显示特征分析幻灯片，存在 Tier 2 数据时显示压缩摘要，并包含信任模型幻灯片。请参阅 [`DECK.md`](DECK.md)。

## 文档

从这里开始：

- [`docs/QUICKSTART.md`](QUICKSTART.md)：首次安全运行和第一个交接包。
- [`docs/COOKBOOK.md`](COOKBOOK.md)：PostgreSQL、MySQL、SQL Server、TLS、演示文稿和无采样工作流的实用方案。
- [`docs/DBA_REVIEW_GUIDE.md`](DBA_REVIEW_GUIDE.md)：DBA/安全审查人员在运行工具前需要了解的内容。
- [`sql/grants/README.md`](../../sql/grants/README.md)：感知版本的最小权限授予脚本，以及采集后的账户删除。
- [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md)：常见故障及修复方法。
- [`docs/MESSAGES.md`](MESSAGES.md)：稳定的 `DBPnnnnS` 运维消息代码。
- [`docs/COMPRESSION_MEASUREMENT.md`](COMPRESSION_MEASUREMENT.md)：Tier 2 压缩采样的工作方式。
- [`docs/INDEX.md`](INDEX.md)：完整文档导航。

安全审查起点：

- [`SECURITY.md`](SECURITY.md)：安全模型和凭据处理。
- [`AUDIT.md`](AUDIT.md)：读取、写入、查询和记录的内容。
- [`FORMAT.md`](FORMAT.md)：输出字段和舍入规则。
- [`TLS.md`](TLS.md)：TLS 和 mTLS 行为。
- [`AUTH.md`](AUTH.md)：支持的身份验证模式。
- [`BUILD.md`](BUILD.md)：从源代码构建和发布验证。
- [`DECK.md`](DECK.md)：可选的 PowerPoint 摘要演示文稿。

## 许可证

Apache-2.0 OR MIT.
