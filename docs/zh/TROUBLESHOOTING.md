# 故障排除

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../TROUBLESHOOTING.md)。本译文不应被视为合同级文本。

**语言：** [English](../TROUBLESHOOTING.md) | [Deutsch](../de/TROUBLESHOOTING.md) | [Français](../fr/TROUBLESHOOTING.md) | [Español](../es/TROUBLESHOOTING.md) | [Polski](../pl/TROUBLESHOOTING.md) | [日本語](../ja/TROUBLESHOOTING.md) | **简体中文**

常见的 `dbwarp-blueprint` 故障及后续处理方法。

由运维人员处理的故障现在会以稳定的 `DBPnnnnS` 消息代码开头，例如 `DBP1001E`。搜索文档或创建支持工单时请使用该代码。请参阅[运维消息代码](MESSAGES.md)。

## 未使用请求的语言

诊断区域设置选择时，请使用显式的受支持值：

```bash
dbwarp-blueprint --lang pl --help
```

支持的值为 `en`、`de`、`fr`、`es`、`pl`、`ja` 和 `zh`。没有提供 `--lang` 时，工具会依次检查 `DBWARP_BLUEPRINT_LANG`、`LC_ALL`、`LC_MESSAGES` 和 `LANG`。不受支持的显式值会以 `DBP1011E` 拒绝；不完整的内嵌语言目录会以 `DBP1010E` 使启动失败，而不是回退到英语。

Windows 通常没有这些区域设置变量；请传入 `--lang` 或设置 `DBWARP_BLUEPRINT_LANG`。

## 横幅宽度或颜色不正确

设置了 `COLUMNS` 时横幅宽度取自该变量，否则在 Linux 和 macOS 上取自控制台，再否则为 80 列。颜色能力取自 `NO_COLOR`、`TERM` 和 `COLORTERM`；Windows 通常没有 `TERM`，此时使用 16 色。可用 `--color always`、`--color never` 或显式设置 `COLUMNS` 覆盖。

## URI 中的密码被拒绝

症状：

```text
DBP1001E refusing to use URI-embedded password
```

修复方法：从 URI 中删除密码，并使用以下方式之一：

```bash
--password-file /path/to/pass
--password-env DBWARP_BLUEPRINT_PASSWORD
```

在 Unix 上，文件模式不得允许组/其他用户读取。

## 密码文件权限错误

症状：工具因权限过宽而拒绝 `--password-file` 或 `--tls-key`。

修复方法：

```bash
chmod 600 /etc/dbwarp/db.pass
chmod 600 /etc/dbwarp/client.key
```

这样可以防止同一主机上的其他本地用户意外获取信息。

## TLS 验证失败

请将 `--tls-mode verify-full` 与正确的 CA 捆绑包和主机名一起使用：

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

如果证书主机名不匹配，请修复 DNS 名称或证书。在非环回主机上，除非同时提供 `--i-know-what-im-doing`，否则 `--tls-skip-verify` 会被拒绝；不要在生产环境中使用它。

## SQL Server TLS 信任根

对于 SQL Server 的证书验证模式，省略 `--tls-ca` 时会使用操作系统信任存储区。提供的 `.pem` 或 `.crt` 文件必须只包含一个 CA 证书，并替换这些根证书。驱动在 `verify-ca` 和 `verify-full` 两种模式下都会检查连接主机名。

## Tier 2 要求明确同意

症状：

```text
--measure-compression requires --yes
```

修复方法：

```bash
--measure-compression --yes
```

这是有意设计的显式要求，因为 Tier 2 会先将有界行样本读入内存，然后再丢弃。

## 采样耗时过长

减小以下一项或两项：

```bash
--sample-rows 500
--max-wall-secs 120
```

对于首次生产审查，较小的 Tier 2 样本优于完全不测量压缩。如果结果有偏或不完整，请在副本上使用更大的预算重新运行。

## DBA 禁止非目录 SELECT 1 探测

禁用 RTT 探测：

```bash
--no-rtt-probe
```

默认 RTT 探测是五次 `SELECT 1` 查询，不会读取行数据，但某些策略会将任何非目录查询都归类为超出范围。

## 输出不包含压缩部分

只有同时提供以下两个选项时，才会出现压缩部分：

```bash
--measure-compression --yes
```

仅目录 Blueprint 是有效的，但下游压缩估算将通过推断获得。

## 某些压缩样本被标记为有偏

某些引擎并非在所有情况下都提供均匀表采样，小表可能需要回退到 `LIMIT`。Blueprint 文件会记录 `sampled_with_bias` 和 `bias_reason`，以便估算器和审查人员将其纳入考量。

有偏样本仍然有用，只是证据力度不如均匀样本。

## 从 TOML 生成演示文稿失败

`--from-toml` 必须与 `--deck` 配对：

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

不要将实时数据库选项与 `--from-toml` 一起使用。工具会拒绝混合实时/离线模式，以保持审计边界简单明确。

## Blueprint 文件看起来太小

正常的 Blueprint 文件很紧凑。它包含结构元数据、舍入后的计数、索引、FK 图结构和可选压缩摘要，不应包含行值或标识符。

如果需要具有代表性的基准测试数据库，请将已批准的 `blueprint.toml` 交给为该项目授权且经过独立审查的下游工具。

## 需要证明未发生上传

使用审计日志和网络工具：

```bash
./dbwarp-blueprint ... --audit-log audit.txt
strace -f -e trace=connect ./dbwarp-blueprint ...
tcpdump host db.internal
```

预期的运行时网络行为取决于当前模式。实时 `--connect` 运行会打开请求的数据库会话；DNS 可能会联系已配置的解析器，集成式 Kerberos/SSPI 身份验证可能会联系 KDC 或域控制器。批处理模式会为每个数据库源打开一个数据库会话。本地 TOML、Parquet、Avro 和捆绑包操作不会发起应用程序网络连接，但网络挂载路径仍受主机存储栈影响。
