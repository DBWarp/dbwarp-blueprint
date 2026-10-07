# 贡献指南

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../CONTRIBUTING.md)。本译文不应被视为合同级文本。

**语言：** [English](../../CONTRIBUTING.md) | [Deutsch](../de/CONTRIBUTING.md) | [Français](../fr/CONTRIBUTING.md) | [Español](../es/CONTRIBUTING.md) | [Polski](../pl/CONTRIBUTING.md) | [日本語](../ja/CONTRIBUTING.md) | **简体中文**

请先提交不含敏感信息的 issue，描述问题并提供一个小型合成数据复现。
对于较大的改动，请在准备补丁前讨论实现方式。
维护者会判断建议是否符合产品定位及其安全边界；提交 issue 或拉取请求不代表一定会被接受。

安全的问题报告请遵循 [SUPPORT.md](SUPPORT.md)，私密漏洞报告请遵循
[SECURITY.md](SECURITY.md)。请相互尊重，并围绕可复现的行为展开讨论。

## 准备改动

- 使用 [BUILD.md](BUILD.md) 中规定的固定版本工具链和锁定依赖。
- 保持改动范围集中，为变更的行为增加回归测试覆盖。
- 切勿在补丁或测试数据中包含客户数据、凭据、私有基础设施详情或可识别身份的审计证据。
- 保留明确同意、有界的源端影响、最小权限访问，以及对缺失或退化观测的如实报告。
- 记录任何 Blueprint 契约或压缩编码变更。现有可选字段及兼容性规则是接口的一部分。
- CLI 选项、诊断代码和序列化字段保持规范英文。
  面向用户的运行时内容如有变化，必须更新所有随产品提供的运行时目录。
  英文 Markdown 为准，译文 Markdown 仅作补充。

## 本地检查

安装固定版本工具链后，在源码仓库中运行：

```bash
cargo fmt --all --check
cargo test --locked --all-targets
./tools/check_blueprint_core_sync.sh
python3 tools/check_public_tree.py
```

核心模块有自己的单元测试：

```bash
cargo test --locked --manifest-path crates/dbwarp-blueprint-core/Cargo.toml --lib
```

通过本地测试并不能证明更改适用于所有数据库版本或平台。请描述实际进行了哪些测试，并明确指出哪些配置未经过测试。未经维护者批准，请勿在补丁中发布构建产物、更新发布标签或更改代码仓库的安全设置。

## 维护人员工作流

规范源是英文 Rust 帮助以及 `src/i18n.rs` 中的消息/UI 定义。当任何用户可见的短语发生变化时：

1. 在同一次提交中更新 `locales/` 下的每个区域设置目录；
2. 精确保留所有占位符和规范运维标记；
3. 运行针对精确覆盖的专项测试；
4. 在相关的测试用例中，添加或更新。
4. 当故障或警告发生变化时，在 `tests/cli_errors.rs` 中添加或更新相应的运维边界用例；
5. 运行完整的测试套件，并检查具有代表性的 help/deck 输出。

专项验证：

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

集成测试还会证明：所有语言的选项标记完全相同，本地化 DBP 代码保持稳定，输出 TOML 不受语言影响，并且生成的演示文稿文字带有所选区域设置。
