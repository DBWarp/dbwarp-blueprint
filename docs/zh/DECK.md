# 可视化摘要演示文稿

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../DECK.md)。本译文不应被视为合同级文本。

**语言：** [English](../../DECK.md) | [Deutsch](../de/DECK.md) | [Français](../fr/DECK.md) | [Español](../es/DECK.md) | [Polski](../pl/DECK.md) | [日本語](../ja/DECK.md) | **简体中文**

`dbwarp-blueprint --deck blueprint.pptx` 会在 `--out` TOML 文件旁写入可选的 PowerPoint (`.pptx`) Blueprint 摘要。`dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx` 可稍后从现有且已经审阅的 Blueprint 文件构建同一演示文稿，而无需连接数据库。它只是同一份匿名化数据的演示：不会从数据库读取或向数据库发送任何其他内容。演示文稿只会根据 Blueprint 中已有的字段计算已记录的本地摘要和预测。

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

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx \
  --lang ja
```

`--lang en|de|fr|es|pl|ja|zh` 会本地化演示文稿中面向用户的文字和 PowerPoint 语言元数据。匿名标识符、数据库类型名称、索引方法、测量值和源 TOML 保持规范且与语言无关。如果演示文稿短语缺失，语言目录验证会按封闭失败原则拒绝运行，而不是替换为英语。请参阅 [`INTERNATIONALISATION.md`](INTERNATIONALISATION.md)。

每一张幻灯片也包含本地化的演讲者备注。这些备注将可见的证据转化为简短、自然的语言摘要，而不是重复幻灯片上的每一个标签和数值。它们只使用来自 Blueprint 的数据，并且省略了重复的页脚，因此演讲者视图和导出的备注手稿可以提供解释，而不会引入新的事实。

## 页脚和保密级别

每一页内容幻灯片都有相同的页脚：左侧有一个小标识，可选的分割线和保密级别，居中的幻灯片编号，以及右侧的`DBWarp.com`。 标题页没有编号。

使用 `--deck-confidentiality public|internal|confidential|restricted` 可添加一个本地化的
内置分类标签。其他任何安全的非空值都会作为自定义标签原样显示；含空格的值需要加
引号，例如 `--deck-confidentiality "CLIENT // SENSITIVE"`。标签不得带有前导或尾随
空格，不得包含控制字符或双向文本格式控制字符，显示宽度也不得超过 48 个单位。
不需要分类标签时请省略此选项。该设置只改变演示文稿的显示方式，不会修改 Blueprint
文件或演示文稿所概述的数据。对于完全相同的已审阅 Blueprint、语言、标签和时间戳，
演示文稿的字节可重现。

## 信任属性

- **在本地从内存构建。** 演示文稿由生成 `blueprint.toml` 的同一内存中 Blueprint 渲染。不会执行额外数据库查询，也不会再次遍历目录。在 `--from-toml` 模式下，内存中 Blueprint 改为从已经审阅的 TOML 文件加载。
- **应用程序不使用网络。** 生成演示文稿不会打开网络连接；从网络挂载路径读取 Blueprint 仍受主机存储栈影响。
- **不使用第三方库。** OOXML 写入器在 `src/deck.rs` 及其 `deck_*` 模块中
  实现。`.pptx` 文件是由 XML 部分组成的 ZIP 文件，您可以使用 `unzip` 检查。
  它不使用 PowerPoint 自动化、渲染服务或额外依赖项。DBWarp 的 logo 图像和
  DM Sans 静态字体文件嵌入在 Rust 二进制文件中，并以 OOXML media/font 格式
  存储；生成过程不会读取任何运行时资源路径。
- **无真实标识符，无行数据。** 表、列和索引使用与 Blueprint 文件相同的匿名占位符（`table-001`、`col-1`、`idx-1`、`schema-A`）。源测量值保留其记录的精度；任何预测都只根据 Blueprint 中已有的字段计算。除该输入外，演示文稿不包含任何特定于您数据库的信息。
- **从固定输入重现。** 对于相同的已审阅 Blueprint、所选语言、保密标签和固定时间戳，会生成字节完全相同的 `.pptx`（固定部件顺序和时间戳）。这并不意味着两次实时采集相同：还需要使用相同的受保护 `--anonymization-key-file`、源状态和采集选项。

## 所含内容

演示文稿会适应模式大小：

- **标题**：DBWarp 标志和标语、引擎、版本、源类型、表数量、生成时间戳。
- **执行摘要**：面向管理层的迁移规模、数据集中度、关系复杂性和审查证据指标。
- **概览**：表/行/数据大小/索引大小总计，以及列、索引、外键和模式数量。
- **小型模式**（少量表）：每个表一个按大小展示的面板（行、字节、列类型、索引）和一个外键图。
- **大型模式**：进行特征分析，而不是逐项列举：
  - *最大表*：按大小排列的最大表，以及 `+ N more` 形式的剩余数量。
  - *模式组成*：列类型分布以及索引/总体统计信息。
  - *关系*：外键数量、相连表与独立表，以及被引用最多的（枢纽）表。
- **实测压缩**（仅 Tier 2）：已采样表数量、加权 zstd-3 比率、预计压缩后占用空间，以及可压缩性最高的已采样表。
- **数据库逻辑，超越表结构。** 非表对象（`graph` 和 `analyzed` 仅提供详细信息）：六个通俗易懂的组别解释了数据库的逻辑和依赖关系，这些内容超出了普通的表定义范围，包括：查询层、可执行逻辑、自动行为、类型和产生值的对象、外部依赖以及平台配置。显示的计数来自 Blueprint 构件清单。
- **构件复杂度**（仅限 `graph` 和 `analyzed` 详细级别）：非表对象的总体复杂度等级、符合条件的对象集合覆盖率，以及全部七个维度的等级。幻灯片将这些维度定义为定义大小、分支、功能使用、依赖关系、环境要求、隐藏源代码和特定于方言的行为。每个维度的等级旁都会显示覆盖率，因此部分或未知证据不会被误认为低复杂度。总体结果为 `unknown` 表示证据不完整；`not applicable` 要求符合条件的对象集合为空且已证实完整。
- **信任模型**：总结上述信任属性的结束幻灯片。

## 审阅输出

`.pptx` 是标准 OOXML 软件包。要审计其确切内容：

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

在 PowerPoint、LibreOffice Impress 或 Google Slides 中打开它。演示文稿创建器位于 [`src/deck.rs`](../../src/deck.rs)，并已集成到 Rust 二进制文件中。无需安装或审计单独的演示文稿创建工具。
