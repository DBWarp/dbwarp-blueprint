# TLS 和证书

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../TLS.md)。本译文不应被视为合同级文本。

**语言：** [English](../../TLS.md) | [Deutsch](../de/TLS.md) | [Français](../fr/TLS.md) | [Español](../es/TLS.md) | [Polski](../pl/TLS.md) | [日本語](../ja/TLS.md) | **简体中文**

只要数据库连接跨越网络边界，就应使用 TLS。
默认模式为 `verify-full`：除非运维人员明确选择其他模式，否则会验证证书链和服务器主机名。

## 常用选项

```bash
--tls-mode disable|prefer|require|verify-ca|verify-full
--tls-ca /path/to/ca-bundle.pem
--tls-cert /path/to/client-cert.pem
--tls-key /path/to/client-key.pem
```

建议的生产设置：

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

## 内部 CA

如果数据库证书由内部 CA 签名：

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml
```

## 主机名不匹配

使用 `--tls-mode verify-full` 运行时，请在 `--connect` 中使用与证书匹配的主机名。此版本不支持覆盖 TLS 服务器名称；传入 `--tls-server-name` 会明确失败，而不是静默削弱验证。对于 PostgreSQL 和 MySQL，如果策略允许仅验证 CA 链而不验证主机名，请使用 `--tls-mode verify-ca`。

默认信任来源因引擎而异：

- 省略 `--tls-ca` 时，PostgreSQL 和 MySQL 使用编译进二进制文件的 Mozilla 根证书。提供 PEM 捆绑包会替换这些根证书。
- 省略 `--tls-ca` 时，SQL Server 使用操作系统信任存储区。提供的 `.pem` 或 `.crt` 文件必须只包含一个 CA 证书，并替换操作系统根证书。

SQL Server 驱动在 `verify-ca` 和 `verify-full` 两种模式下都会验证连接主机名；对该引擎而言，`verify-ca` 有意不弱于 `verify-full`。

## 明文和兼容模式

`prefer` 仅允许用于环回目标。PostgreSQL 在该模式下可能回退到本地明文并发出 `DBP1404W`；其他引擎仍会尝试 TLS。远程目标使用 `disable` 或 `require` 时需要 `--i-know-what-im-doing`，因为前者允许明文，后者加密但不验证服务器。该确认不会使这些模式适合生产环境。

## mTLS

PostgreSQL 和 MySQL 支持客户端证书身份验证。如果其中任一数据库要求客户端证书：

```bash
--tls-cert /etc/dbwarp/client.crt \
--tls-key /etc/dbwarp/client.key
```

在类 Unix 系统上，私钥文件不得允许同组用户或其他用户读取。
SQL Server 尚未实现客户端证书身份验证；对该引擎提供 `--tls-cert`/`--tls-key` 会以 `DBP1015E` 失败，而不是静默忽略这些文件。

## 跳过验证

`--tls-skip-verify` 仅用于诊断。除非安全团队已经明确批准，否则不要将其用于生产数据库 Blueprint 采集。

## 审计日志

审计日志会记录请求的 TLS 模式、CA 路径、客户端证书路径以及是否跳过验证。连接成功后会记录是否协商 TLS；当前驱动不公开可靠的协议版本，因此版本会标为不可用。不会记录私钥内容。
