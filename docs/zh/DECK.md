# 可视化摘要演示文稿

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../DECK.md)。本译文不应被视为合同级文本。

**语言：** [English](../../DECK.md) | [Deutsch](../de/DECK.md) | [Français](../fr/DECK.md) | [Español](../es/DECK.md) | [Polski](../pl/DECK.md) | [日本語](../ja/DECK.md) | **简体中文**

`dbwarp-blueprint --deck blueprint.pptx` 会在 `--out` TOML 文件旁写入可选的 PowerPoint (`.pptx`) Blueprint 摘要。`dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx` 可稍后从现有且已经审阅的 Blueprint 文件构建同一演示文稿，而无需连接数据库。它只是同一份匿名化数据的演示，不会读取、发送或计算任何其他数据库信息。

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

## 页脚和保密级别

每张内容幻灯片都使用 DBWarp 标准页脚：左侧为小型标志，可选的分隔符和保密
级别，中央为不带其他文字的幻灯片编号，右侧为 `DBWarp.com`。标题幻灯片不编号。

使用 `--deck-confidentiality public|internal|confidential|restricted` 可添加一个本地化的
内置分类标签。其他任何安全的非空值都会作为自定义标签原样显示；含空格的值需要加
引号，例如 `--deck-confidentiality "CLIENT // SENSITIVE"`。标签不得带有前导或尾随
空格，不得包含控制字符或双向文本格式控制字符，显示宽度也不得超过 48 个单位。
不需要分类标签时请省略此选项。该设置只改变演示文稿的显示方式，不会修改 Blueprint
文件或演示文稿所概述的数据；固定 `--generated-at` 时，输出仍具有确定性。

## 信任属性

- **在本地从内存构建。** 演示文稿由生成 `blueprint.toml` 的同一内存中 Blueprint 渲染。不会执行额外数据库查询，也不会再次遍历目录。在 `--from-toml` 模式下，内存中 Blueprint 改为从已经审阅的 TOML 文件加载。
- **无网络。** 生成演示文稿不会建立任何类型的出站连接。
- **无第三方库。** OOXML 直接在 [`src/deck.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/deck.rs) 中生成；`.pptx` 是由 XML 部件组成的普通 ZIP，您可以使用 `unzip` 解压并阅读。无 PowerPoint 自动化、无渲染服务，依赖关系图中也不增加 crate。已批准的 DBWarp 标志图像和静态 DM Sans 字体嵌入在 Rust 二进制文件中，并作为 OOXML 媒体和字体部件写入；生成时不会读取运行时资产路径。
- **无真实标识符，无行数据。** 表、列和索引使用与 Blueprint 文件相同的匿名占位符（`table-001`、`col-1`、`idx-1`、`schema-A`），每个数字都使用相同的有文档记录的精度。除 Blueprint 文件所含内容外，演示文稿不包含任何客户特定事实。
- **确定性。** 固定 `--generated-at` 后，对于同一所选语言，相同 Blueprint 会生成字节完全相同的 `.pptx`（固定部件顺序、固定时间戳）。

## 所含内容

演示文稿会适应模式大小：

- **标题**：DBWarp 标志和标语、引擎、版本、源类型、表数量、生成时间戳。
- **执行摘要**：面向管理层的迁移规模、数据集中度、关系复杂性和可共享证据指标。
- **概览**：表/行/数据大小/索引大小总计，以及列、索引、外键和模式数量。
- **小型模式**（少量表）：每个表一个按大小展示的面板（行、字节、列类型、索引）和一个外键图。
- **大型模式**：进行特征分析，而不是逐项列举：
  - *最大表*：按大小排列的最大表，以及 `+ N more` 形式的剩余数量。
  - *模式组成*：列类型分布以及索引/总体统计信息。
  - *关系*：外键数量、相连表与独立表，以及被引用最多的（枢纽）表。
- **实测压缩**（仅 Tier 2）：已采样表数量、加权 zstd-3 比率、预计压缩后占用空间，以及可压缩性最高的已采样表。
- **信任模型**：总结上述保证的结束幻灯片。

## 审阅输出

`.pptx` 是标准 OOXML 软件包。要审计其确切内容：

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

可使用 PowerPoint、LibreOffice Impress 或 Google Slides 打开。生成器位于 [`src/deck.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/deck.rs)，并内置于 Rust 二进制文件中。无需安装、审计或保持同步的独立演示文稿生成器。
