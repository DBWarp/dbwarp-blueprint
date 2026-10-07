# 支持与问题报告

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../SUPPORT.md)。本译文不应被视为合同级文本。

**语言：** [English](../../SUPPORT.md) | [Deutsch](../de/SUPPORT.md) | [Français](../fr/SUPPORT.md) | [Español](../es/SUPPORT.md) | [Polski](../pl/SUPPORT.md) | [日本語](../ja/SUPPORT.md) | **简体中文**

有关不涉及敏感信息的安装问题、可复现的缺陷和功能请求，请使用
[问题跟踪器](https://github.com/DBWarp/dbwarp-blueprint/issues)。
该渠道不承诺响应时间，也不提供服务级别保证。

请通过 [SECURITY.md](SECURITY.md) 中的途径私下报告可疑漏洞，而不要提交公开 issue。

## 有帮助的信息

- 确切的发布标签、二进制校验和、操作系统和架构。
- 数据库引擎和版本或结构化文件格式，以及源端属于自管理还是托管环境。
- 命令选项：其中的凭据、端点、路径及可识别身份的选择器应已移除，或替换为明确标注的示例。
- `DBP` 诊断代码、预期行为和实际行为。
- 如有可能，提供一个小型合成数据复现。

不要上传生产数据、凭据，或未经审阅的审计日志、捆绑包、Blueprint 或演示文稿。
审计可能包含身份和端点；匿名 Blueprint 仍可能暴露有辨识度的工作负载结构。
请从最少且安全的说明开始，并在分享前审阅每个附件。

## 受支持的配置

[STATUS.md](../../STATUS.md) 描述了功能以及支持的数据库版本。 [BUILD.md](BUILD.md) 描述了平台和身份验证相关的构建要求。 Rust 被固定到 `rust-toolchain.toml` 中的确切版本； 其他工具链可能尚未经过测试。

托管服务权限指南并非意味着所有服务或配置都经过了测试。请使用相应的[权限要求](../../sql/grants/DATABASE_PERMISSIONS.md)，并在生产环境中使用之前，测试具体的配置。

有关采集器版本之间的变更，请参阅 [CHANGELOG.md](CHANGELOG.md)。
