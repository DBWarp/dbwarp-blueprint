# 身份验证

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../AUTH.md)。本译文不应被视为合同级文本。

**语言：** [English](../../AUTH.md) | [Deutsch](../de/AUTH.md) | [Français](../fr/AUTH.md) | [Español](../es/AUTH.md) | [Polski](../pl/AUTH.md) | [日本語](../ja/AUTH.md) | **简体中文**

`dbwarp-blueprint` 支持 PostgreSQL、MySQL 和 SQL Server Blueprint 采集最常用的身份验证模式。

## 用户名

您可以在 URI 中提供用户名，也可以单独提供：

```bash
--connect postgresql://app@db.internal/payments
```

或者：

```bash
--connect postgresql://db.internal/payments --user app
```

对于不便进行 URI 编码的用户名，请使用：

```bash
--user-file /path/to/user.txt
--user-env DB_USER
```

## 密码

建议方式：

```bash
--password-file /path/to/password.txt
```

替代方式：

```bash
--password-env DB_PASSWORD
```

如果未提供密码来源，工具会在可行时进行交互式提示。

工具拒绝内嵌于连接 URI 的密码。

## SQL Server Entra ID 令牌

对于使用 Microsoft Entra ID 的 Azure SQL Database 或 Managed Instance，请使用常规工具生成令牌，然后将其作为机密传给 `dbwarp-blueprint`。

令牌文件：

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-file /secure/path/token.txt \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out blueprint.toml
```

指定的环境变量：

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-env AZURE_SQL_TOKEN \
  --tls-mode verify-full \
  --out blueprint.toml
```

工具不会调用 Azure CLI，不会刷新令牌，也不会将令牌写入磁盘。

## SQL Server 集成身份验证

集成身份验证使用主机上已存在的操作系统凭据。

Linux Kerberos / GSSAPI：

```bash
kinit user@EXAMPLE.COM
./dbwarp-blueprint \
  --connect sqlserver://db.internal,1433/payments \
  --auth-mode integrated \
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' \
  --tls-mode verify-full \
  --out blueprint.toml
```

Linux 发布归档包含 Kerberos/GSSAPI 支持，但启动时不需要其运行时库。二进制
文件会在选择 `--auth-mode integrated` 时加载平台 GSSAPI 运行时；如果缺少该
运行时，则会报告 `DBP1604E`。从源代码构建时，必须使用
`DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` 启用该功能。

Windows SSPI：

```powershell
.\dbwarp-blueprint.exe `
  --connect sqlserver://db.internal,1433/payments `
  --auth-mode integrated `
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' `
  --tls-mode verify-full `
  --out blueprint.toml
```

在 integrated 模式下，`dbwarp-blueprint` 不读取密码。操作系统会向 SQL Server 驱动程序提供身份验证令牌。

集成身份验证仅适用于 SQL Server。PostgreSQL 和 MySQL 会以 `DBP1005E` 拒绝 `--auth-mode integrated`。

以上示例假定 Windows 主体已作为 SQL Server 登录名存在。`sql/grants/` 中的各层级脚本会创建带密码的 SQL 登录名，这种形式不适用于此模式。因此，应先使用 `FROM WINDOWS` 创建登录名，再原样应用相应层级的权限；只有登录 DDL 不同。有关具体语句以及组、托管服务账户和计算机账户的情况，请参阅[集成身份验证的 Windows 和域主体](../../sql/grants/DATABASE_PERMISSIONS.md#windows-and-domain-principals-for-integrated-authentication)。

与 `sql-auth` 相比，此模式下有两个尤其重要的运维注意事项。SQL Server 看到的身份就是运行采集器进程的账户。如果管理员在 `BUILTIN\Administrators` 属于 `sysadmin` 的主机上启动采集器，则该会话将以 `sysadmin` 身份运行，并绕过权限脚本中的所有 `DENY`，但采集仍会成功。`--expect-server-principal` 会在读取任何目录之前使此情况以 `DBP1606E` 失败。此外，专用服务账户不会继承启动者的文件访问权限。使用凭据文件时，该账户需要读取自身凭据文件的权限，并需要对 `--out` 和 `--audit-log` 路径的写入权限。

每个 SQL Server 连接都会在本地审计中记录 `ORIGINAL_LOGIN()`、`SUSER_SNAME()` 和 `USER_NAME()`。`--expect-server-principal` 是可选的，并且也适用于 SQL 身份验证。它要求 SQL Server 将 `ORIGINAL_LOGIN()` 与已建立会话中的预期主体进行比较。如果出现不匹配或身份不可用，则会发生 `DBP1606E` 错误，这发生在任何目录捕获之前，因此操作员无法意外地在不同的或具有更高权限的登录名下进行收集。确切的身份信息仍然是本地审计的证据，并且不会包含在 Blueprint、deck 或 bundle 文件中。

## 云托管数据库身份验证

托管端点本身不会改变 `dbwarp-blueprint` 所需的数据库权限。原生数据库用户名和密码使用 `sql-auth`；完成网络和数据库账户预配后，不需要云控制平面角色。

`dbwarp-blueprint` 不调用云 CLI、元数据服务、密钥管理器或令牌刷新 API。包装程序必须生成或检索每个短期令牌，并通过一个受保护的密钥来源提供该令牌。

### PostgreSQL 和 MySQL 云令牌

对于由 AWS、Azure 或 Google Cloud 生成的 PostgreSQL/MySQL 托管服务直接令牌，请使用 `cloud-token`。必须且只能提供 `--password-file` 或 `--password-env` 之一。此模式要求 `verify-full`；如果提供商或实例 CA 不在二进制文件编译的信任集中，请添加相应 CA 证书包。

PostgreSQL 示例：

```bash
./dbwarp-blueprint \
  --connect postgresql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

MySQL 示例：

```bash
./dbwarp-blueprint \
  --connect mysql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

对于 MySQL，`cloud-token` 仅在该经过验证的 TLS 连接内启用 `mysql_clear_password` 交换。普通 `sql-auth` 连接仍禁用该插件。PostgreSQL 在同样的已验证 TLS 要求下使用其常规密码协议。

### 云端运行时权限

这些权限用于授权登录或连接隧道，绝不会替代数据库主体及其权限：

| 托管连接路径 | 二进制模式 | 数据库外部的运行时权限 |
|---|---|---|
| RDS/Aurora PostgreSQL 或 MySQL IAM 登录 | `cloud-token` | 针对确切数据库用户 ARN 的 `rds-db:connect` |
| Azure Database for PostgreSQL/MySQL Entra 登录 | `cloud-token` | 数据访问不需要 Azure 资源 RBAC 角色；必须在数据库内映射该身份 |
| Cloud SQL PostgreSQL/MySQL 直接 IAM 登录 | `cloud-token` | 确切权限 `cloudsql.instances.login`；`roles/cloudsql.instanceUser` 是权限更宽的预定义替代角色 |
| Cloud SQL Auth Proxy 或连接器 | 通常使用 `sql-auth`；代理可能执行自动 IAM 身份验证 | 代理身份需要 `roles/cloudsql.client`；自动 IAM 身份验证还需要登录权限 |
| Azure SQL Database 或 Managed Instance Entra 登录 | `entra-token` | 数据访问不需要 Azure 资源 RBAC 角色；使用上文记录的 SQL Server 令牌选项 |
| 使用原生数据库凭据的任何受支持托管数据库 | `sql-auth` | 无 |

在您审查权限时，请记录版本相关的数据库权限、精确的云策略、内置的角色替代方案以及范围限制。服务配置、主体创建、网络访问、令牌生成以及可选的密钥检索是配置或包装层面的职责，而不是应该附加到采集器的权限，仅仅因为该端点是受管理的。
