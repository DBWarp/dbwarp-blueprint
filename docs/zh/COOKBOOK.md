# 操作手册

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../COOKBOOK.md)。本译文不应被视为合同级文本。

**语言：** [English](../COOKBOOK.md) | [Deutsch](../de/COOKBOOK.md) | [Français](../fr/COOKBOOK.md) | [Español](../es/COOKBOOK.md) | [Polski](../pl/COOKBOOK.md) | [日本語](../ja/COOKBOOK.md) | **简体中文**

面向常见 `dbwarp-blueprint` 工作流的任务型操作方案。

## 方案：本地化运维会话

选择一个完整的内嵌语言目录，同时保持命令、值、标识符和输出模式使用规范形式：

```bash
./dbwarp-blueprint --lang de --help
./dbwarp-blueprint --lang ja \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

对于无人值守运行，请设置 `DBWARP_BLUEPRINT_LANG=fr` 或标准进程区域设置。显式的 `--lang` 始终优先。DBP 代码和底层提供商详细信息保持规范形式，因此可以搜索本地化故障并将其分享给支持人员。

## 方案：使用内部 CA 的 PostgreSQL

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out pg-appdb.blueprint.toml \
  --audit-log pg-appdb.audit.txt
```

用于常规生产 PostgreSQL 审查。如果主机名验证失败，请修复服务器证书或使用正确的 DNS 名称；除环回测试外，不要使用 `--tls-skip-verify`。

## 方案：使用用户名文件的 MySQL

当用户名包含不便进行 URI 编码的字符时很有用。

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

若要重建具有性能代表性的合成数据，请使用默认的 balanced 策略：精确的 MySQL 声明/索引元数据和紧密舍入的采样宽度：

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

确认 `declared_length_fidelity = "exact"`、`index_length_fidelity = "exact"` 和 `observed_length_fidelity = "relative-rounded-v2"`。只有在客户批准分享精确的采样长度统计信息后，才使用 `--length-fidelity exact --yes`。名称和值仍会被排除。

对于拥有数千个表的数据资产，如有需要，请将 `--max-wall-secs` 提高到默认的 300 秒以上。保真度标记用于证明策略，而下游估算器会单独要求每个非空可变宽度索引列具备观测平均值/p95 长度，之后才会将测试数据集标记为基准测试就绪。

## 方案：SQL Server SQL 身份验证

```bash
./dbwarp-blueprint \
  --connect sqlserver://sql-blueprint@sql-primary.internal,1433/appdb \
  --password-file /etc/dbwarp/sql-blueprint.pass \
  --auth-mode sql-auth \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-appdb.blueprint.toml \
  --audit-log mssql-appdb.audit.txt
```

SQL Server 的证书验证 TLS 模式在省略 `--tls-ca` 时使用操作系统信任存储区。提供的 `.pem` 或 `.crt` 文件必须只包含一个 CA 证书，并替换这些根证书。`verify-ca` 和 `verify-full` 都会验证连接主机名。

## 方案：SQL Server Entra ID 令牌

在工具外部生成令牌，然后通过文件传入：

```bash
install -d -m 700 "$HOME/.cache/dbwarp-blueprint"
TOKEN_FILE="$HOME/.cache/dbwarp-blueprint/sql-token"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"
chmod 600 "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

## 方案：仅目录安全审查

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

这是阻力最低的审查模式。它避免行采样，但会降低下游压缩和出口流量估算的准确性。

## 评估非表对象迁移复杂度

首先使用默认摘要，在不读取定义的情况下收集计数和外部前提条件：

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail summary \
  --out appdb-summary.blueprint.toml \
  --audit-log appdb-summary.audit.txt \
  --yes
```


获得安全批准后，收集匿名依赖关系和有界的语言复杂度证据：

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail analyzed \
  --out appdb-analyzed.blueprint.toml \
  --audit-log appdb-analyzed.audit.txt \
  --yes
```


请检查 `visibility`、全部三个完整性标志、`catalogs_unreadable`、`families_not_inventoried` 和 `counts_by_external_class`。将每个外部类别视为明确的迁移任务。已清点的对象并不证明 DBWarp 能重建或转换它；请与迁移能力矩阵比较。参见 [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)。

## 方案：禁用 RTT 探测

默认情况下，工具会在建立连接后运行五次 `SELECT 1` 探测并输出一个 `[network]` 块。如果 DBA 禁止非目录查询，请禁用它：

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --no-rtt-probe \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

RTT 探测绝不会读取行数据；每个查询只返回常量整数 `1`。

## 方案：压缩采样限时运行

对于大型生产系统，首次运行应保持保守：

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal/appdb \
  --password-file /etc/dbwarp/mysql.pass \
  --measure-compression --yes \
  --sample-rows 500 \
  --max-wall-secs 120 \
  --out blueprint.toml \
  --audit-log audit.txt
```

如果输出将许多样本标记为有偏或缺失，请在只读副本上使用更大的时间预算重新运行。

## 方案：一个客户、多个数据库

当客户希望针对多个数据库获得一个可审阅的软件包时，请使用批处理清单。

`customer.batch.toml`：

```toml
[defaults]
measure_compression = true
sample_rows = 1000
max_wall_secs = 300
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["erp", "critical"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
tags = ["billing"]

[[source]]
id = "warehouse_sql"
kind = "sqlserver"
connect_env = "WAREHOUSE_SQL_URI"
password_file = "/etc/dbwarp/warehouse.pass"
auth_mode = "sql-auth"
tags = ["warehouse"]
```

试运行：

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

运行：

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

这会写出 `bundle.toml`、每个源对应的一个子 Blueprint，以及每个源对应的一份审计。每个子 Blueprint 仍可单独审阅。

## 方案：一个客户、混合数据库和数据湖文件

当客户在实时数据库旁还有 Parquet 或 Avro 提取文件时，请在同一批处理中使用结构化文件源。

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["database"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/mnt/customer/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/mnt/customer/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` 当前像 `merge_same_schema` 一样合并文件，但会在捆绑包中保留客户的意图。请将不相关的模式放在不同源中。

## 方案：从捆绑包中仅提取一个源或表

批处理运行后，列出源：

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

提取一个源：

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

从一个源中提取一个表：

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

当客户仅批准数据资产的一部分用于基准测试，或者您希望从大型捆绑包生成小型、聚焦的测试数据集时，请使用此方式。

## 方案：打包经过单独审阅的捆绑包以供交接

工作捆绑包目录包含子 Blueprint 和访问受控的审计。不要将其整体传输。审阅清单值和子 Blueprint 后，创建单文件交接包：

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

打包文件会保留运维人员提供的源 ID、标签、数据集组 ID 和审计路径元数据。请使用匿名值，检查打包后的 TOML，并且只通过批准的渠道传输。

## 方案：批处理交接包

创建如下目录：

```text
customer-blueprint-handoff/
  customer-blueprint-bundle.packed.toml
  customer.batch.toml.redacted
  reviewer-notes.md       # optional
```

请从经过审阅的副本构建此独立目录。将工作用的 `bundle.toml`、`blueprints/`、`audits/` 和任何 `errors.txt` 保留在本地并实施访问控制。`customer.batch.toml.redacted` 应仅显示已批准的源 ID、种类、标签和数据集模式。不要包含机密、私有主机名、密码文件、令牌文件、私钥、数据库日志或解码后的行样本。

## 方案：从已审阅 TOML 离线生成演示文稿

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

此模式只读取 TOML 文件并写出演示文稿。它会拒绝实时数据库选项，而不是静默忽略它们。

## 方案：字节级完全一致的可重现性

固定时间戳：

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

用于取证审查、快照比较或确定性演示文稿生成。

## 方案：DBWarp 交接包

创建如下目录：

```text
customer-blueprint-handoff/
  blueprint.toml
  blueprint.pptx              # optional
  command-used.redacted.txt
  reviewer-notes.md           # optional
```

`command-used.redacted.txt` 可记录已批准的选项和采样预算，但须删除凭据、令牌、私有主机名和本地路径。将 `audit.txt` 作为访问受控的运维证据保留在本地。仅在有明确支持需求时通过批准的安全渠道提供该文件。不要包含密码文件、令牌文件、私钥或数据库日志。
