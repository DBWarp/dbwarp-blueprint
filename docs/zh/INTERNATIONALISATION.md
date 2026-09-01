# 国际化

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../INTERNATIONALISATION.md)。本译文不应被视为合同级文本。

**语言：** [English](../INTERNATIONALISATION.md) | [Deutsch](../de/INTERNATIONALISATION.md) | [Français](../fr/INTERNATIONALISATION.md) | [Español](../es/INTERNATIONALISATION.md) | [Polski](../pl/INTERNATIONALISATION.md) | [日本語](../ja/INTERNATIONALISATION.md) | **简体中文**

`dbwarp-blueprint` 将面向用户的显示与运维语法分离。这是安全和自动化边界，而不仅仅是显示偏好。

## 支持的语言

英文源文本具有最高效力。非英文显示目录由机器辅助生成，即使其键和标记覆盖率已
通过验证，也可能包含错误。请对照英文文本核对安全、合同、监管和最小权限方面的
决策。翻译文档另行采用的发布门控参见
[`TRANSLATIONS.md`](../TRANSLATIONS.md)。

| 值 | 语言 | 生成演示文稿中使用的区域设置标签 |
|---|---|---|
| `en` | 英语 | `en-US` |
| `de` | 德语 | `de-DE` |
| `fr` | 法语 | `fr-FR` |
| `es` | 西班牙语 | `es-ES` |
| `pl` | 波兰语 | `pl-PL` |
| `ja` | 日语 | `ja-JP` |
| `zh` | 简体中文 | `zh-CN` |

显式选择语言：

```bash
dbwarp-blueprint --lang de --help
dbwarp-blueprint --lang ja --connect postgresql://db.internal/app --dry-run
```

未提供 `--lang` 时，解析顺序为：

1. `DBWARP_BLUEPRINT_LANG`；
2. `LC_ALL`；
3. `LC_MESSAGES`；
4. `LANG`；
5. 英语。

环境区域设置标签可以包含地区和编码后缀，因此 `de_CH.UTF-8`、`pl_PL.UTF-8` 和 `ja-JP` 会解析为各自的基础语言。显式 `--lang` 值有意限制为表中的七个规范标记。

Windows 通常不设置 `LC_ALL`、`LC_MESSAGES` 和 `LANG`，因此除非传入 `--lang` 或设置 `DBWARP_BLUEPRINT_LANG`，工具会使用英语。例如 PowerShell 使用 `$env:DBWARP_BLUEPRINT_LANG = "de"`，cmd 使用 `set DBWARP_BLUEPRINT_LANG=de`。Windows 环境变量名不区分大小写，Linux 和 macOS 区分；请始终使用规范的大写名称。

## 会翻译的内容

- 顶层和选项帮助说明；
- 用法和可选值标签等帮助框架；
- 预检计划和同意提示；
- DBP 消息摘要、原因和纠正措施；
- 进度和警告文字；
- PowerPoint 演示文稿标题、标签、说明和区域设置元数据。

诊断所需的致命底层详情可能原样保留在本地化 DBP 消息下方。非致命数据库警告会在原始驱动详情可能包含源标识符时将其隐藏；稳定的 DBP 代码和匿名 Blueprint 目标仍可用于排查。

## 永不改变的内容

以下内容在每种显示语言中都保持为规范英文标记：

- `dbwarp-blueprint` 命令和 `--measure-compression` 等选项名称；
- `verify-full`、`balanced` 和 `exact` 等可接受值；
- `postgresql://`、`mysql://` 和 `sqlserver://` 等 URI 方案；
- 环境变量名称和文件路径；
- `source=ID` 和 `table=ID` 等选择器；
- `DBP1001E` 等 DBP 标识符；
- `table-001`、`col-1` 和 `schema-A` 等匿名化标识符；
- 审计键、TOML 键、捆绑包键、数据库类型名称和索引方法。

因此，脚本不需要针对不同语言处理不同的选项或值；当所有其他确定性输入相同时，在 `--lang ja` 下生成的 Blueprint 与在 `--lang en` 下生成的 Blueprint 逐字节完全相同。

## 严格目录行为

所有本地化目录都编译进二进制文件。程序在启动时会验证每个已发布的非英语区域设置是否精确覆盖：

- 当前实时 Clap 帮助树；
- 每个稳定 DBP 代码及其全部三个诊断字段；
- 每个稳定提示、进度、警告和演示文稿键；
- 每个必需的占位符和受保护的运维标记。

缺少或多余条目、占位符更改、运维标记变化、无效 JSON 或不可见/双向格式控制字符都会触发 `DBP1010E`，并按封闭失败原则拒绝启动。程序不会在缺少翻译时静默替换为英语。

## 维护人员工作流

规范源是英文 Rust 帮助以及 `src/i18n.rs` 中的消息/UI 定义。当任何面向客户的短语发生变化时：

1. 在同一次提交中更新 `locales/` 下的每个区域设置目录；
2. 精确保留所有占位符和规范运维标记；
3. 运行针对精确覆盖的专项测试；
4. 当故障或警告发生变化时，在 `tests/cli_errors.rs` 中添加或更新相应的运维边界用例；
5. 运行完整测试套件，并检查有代表性的帮助/演示文稿输出；
6. 在将新文字视为适用于客户合同、监管申报或公开营销材料的最终文字之前，取得母语技术审校。

专项验证：

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

集成测试还会证明：所有语言的选项标记完全相同，本地化 DBP 代码保持稳定，输出 TOML 不受语言影响，并且生成的演示文稿文字带有所选区域设置。
