# 更新日志

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../CHANGELOG.md)。本译文不应被视为合同级文本。

**语言：** [English](../../CHANGELOG.md) | [Deutsch](../de/CHANGELOG.md) | [Français](../fr/CHANGELOG.md) | [Español](../es/CHANGELOG.md) | [Polski](../pl/CHANGELOG.md) | [日本語](../ja/CHANGELOG.md) | **简体中文**

发布版本标识采集器。Blueprint 模式版本和压缩样本编码是各自独立的兼容性契约；
请参阅 [FORMAT.md](FORMAT.md) 和[压缩测量](COMPRESSION_MEASUREMENT.md)。

## 1.6.0

### Blueprint schema v7

- 输出 schema v7，其中包含明确的表分类、存储组织、分区、段状态、列序号，
  以及更丰富的类型、可空性、生成值和标识语义。
- 为每个表和整个采集添加有关结构、行数、已分配字节、统计信息和源环境观测的
  证据。缺失、被拒绝、格式错误或受选择范围限制的证据会明确保留，而不会表示成
  已测量的零或完整清单。
- 在对象清单中添加基于目录的逐对象要求状态和有界复杂性证据。当无法证明所需
  目录或所选评估对象完整时，汇总完整性仍为 false。
- 保留对 schema-v1 到 schema-v6 格式的输入兼容性。较早的版本可能无法读取 schema-v7 格式的文件。
### 采集保真度与安全性

- 在 PostgreSQL、MySQL 和 SQL Server 中区分完整的有界读取、部分样本和目录
  估算，包括行级安全和继承边界、自适应 MySQL 范围窗口以及 SQL Server
  内存优化表。
- 加强基数、NULL 计数、分区、关系和汇总的一致性，防止舍入或不完整的证据变成
  精确断言。
- 未安装 PolyBase 时，将 SQL Server 外部表的覆盖范围明确报告为一项限制。
- 采集受行数、字节数、值和截止时间限制。非致命降级仍会通过稳定消息代码显示在
  Blueprint 和审计记录中。

### Oracle 范围边界

- 添加 Oracle Basic 目录采集功能，用于 Oracle 12c、19c、21c 和
  23ai/26ai，作为预览版本：该功能仅采集目录信息，不读取任何表数据，且需要确认后
  才能启用。详情请参见 `sql/grants/ORACLE_PREVIEW.md`，了解其限制。

### 发布产物与身份验证

- Linux 发布归档包含 SQL Server Kerberos/GSSAPI 身份验证。只有在选择集成身份验证时，
  才会加载平台 Kerberos 运行时，因此采集器无需 Kerberos 库即可启动；只有在使用
  集成身份验证时，缺少运行时才会报告 `DBP1604E`。Windows 发布二进制文件继续包含
  SQL Server SSPI 身份验证。
- 两种集成模式都使用操作系统凭据，并且在服务器主体与预期主体不匹配时拒绝连接。

### 运维与兼容性

- SQL Server 的 Enhanced 授权脚本会在单独的批次中添加一个服务器级权限：SQL Server
  2019 使用 `VIEW SERVER STATE`，2022 和 2025 使用
  `VIEW SERVER PERFORMANCE STATE`。这样，使用 `--artifact-detail graph` 或
  `analyzed` 的 Enhanced 采集便可报告自管理服务器的大致 CPU 和内存区间。Basic 和
  Standard 不授予此权限，并将这些区间报告为未知；DBA 可以移除该批次，以便在没有
  此权限的情况下保留 Enhanced。
- 将 SQL Server、PostgreSQL、Parquet 和 Windows 身份验证依赖项更新到修复已发布
  安全公告的版本。SQL Server 驱动程序升级到 0.13；`--tls-ca` 保持其限制性含义，
  仅信任提供的 CA。`--max-wall-secs` 仍是整个采集的唯一截止时间。
- SQL Server 仅在请求分析非表对象时读取操作系统容量（`--artifact-detail graph` 或
  `analyzed`）。其他采集将容量区间记录为未请求，而不是读取失败。
- 英文文档仍为权威版本。翻译后的 Markdown 仅作补充，并在英文措辞和运行时消息
补充信息，并附有自己的翻译声明。

### 演示文稿

- 在演示文稿中添加非表对象幻灯片和单独的构件复杂度幻灯片。复杂度幻灯片仅在已采集
  复杂度时显示，并在每个等级旁显示覆盖率，因此不完整的证据绝不会显示为低复杂度。

## 1.5.1

### 采集与保真度

- 保持 Blueprint 模式 v6，区分目录估算、采样观测以及无法获取的统计信息新鲜度证据。
- 改进二进制负载测量、已压缩负载的特征分析和有界压缩探针。
有限制的压缩探测。来自不同 `sample_encoding` 值的比例不可互换；在比较比例之前，请检查编码。
- 限制 MySQL 和 SQL Server 自适应采样重试的规模，包括过大值及字符集转换引起的扩张。
  记录仍然存在的前缀偏差，同时保留原始采样值的长度元数据。
- 修正连接字符集改变返回字节长度时的 MySQL 截断检测。

### 运维与审查

比较构建验证以及支持的数据库版本。
- 更新机器翻译文档和运行时措辞，继续以英文为准，译文作为补充材料。
- 在源码和二进制分发包中加入发布历史、支持和贡献指南。
### 兼容性。

现有的蓝图仍然可以被新的采集器读取。反之则不保证：1.5.0版本读取器会拒绝新的可选`sample_layout`字段，而旧版本可能无法识别新的压缩样本编码。使用相同的版本来生成文件，并从中构建演示文稿。

固定确切的发布版本和校验和。来自不同版本的成果不保证一致。

## 1.5.0

上一版本提供 PostgreSQL、MySQL 和 SQL Server 的模式 v6 采集、结构化文件检查、
本地 Blueprint 和演示文稿输出，以及针对不同版本的权限授予脚本。
其确切源码和成果物请参阅
[发布标签](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0)。

问题报告请参阅 [SUPPORT.md](SUPPORT.md)。贡献指南请参阅 [CONTRIBUTING.md](CONTRIBUTING.md)。
