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
  --artifact-detail none \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

对于无人值守运行，请设置 `DBWARP_BLUEPRINT_LANG=fr` 或标准进程区域设置。显式的 `--lang` 始终优先。DBP 代码和底层驱动程序详细信息保持规范形式，因此可以搜索本地化故障并将其分享给支持人员。

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

上述配置已经使用了默认的平衡策略：精确的 MySQL declaration/index 元数据，以及经过严格舍入的采样宽度。

确认 `declared_length_fidelity = "exact"`、`index_length_fidelity = "exact"` 和 `observed_length_fidelity = "relative-rounded-v2"`。 仅在您的组织批准共享精确的抽样长度统计信息后，才使用 `--length-fidelity exact --yes`。 姓名和值仍然被排除。

在拥有数千张表的数据库中，如果需要，请将 `--max-wall-secs` 的值设置为高于其默认的 300 秒。 质量标志描述了策略；它们并不表明采样覆盖了所有表。

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
install -m 600 /dev/null "$TOKEN_FILE"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

Azure SQL 提供由公共 CA 签发的证书，因此本方案不设置 `--tls-ca`，而是使用操作
系统信任存储区。提供的 `--tls-ca` 文件会用单个证书替换该存储区；请参见
[TLS](TLS.md)。

## 方案：仅目录安全审查

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

这是最便捷的审查模式。它避免了行采样，但会产生不太准确的压缩和数据传输量估算。

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


审查 `visibility`，所有三个完整性选项，`catalogs_unreadable`、`families_not_inventoried` 和 `counts_by_external_class`。将每个外部类视为一个明确的迁移任务。不要将已记录的对象视为 DBWarp 能够重新创建或翻译的证明；请询问 DBWarp 哪些对象类型支持您的迁移。请参阅 [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)。

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

## 食谱：在一个软件包中包含多个数据库。

当您希望对多个数据库进行审查时，可以使用批处理清单。

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

## 食谱：混合数据库和数据湖文件。

当您需要同时处理 Parquet 或 Avro 抽取文件以及实时数据库时，请将结构化文件作为源，并将其放在同一个批处理中。

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
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` 将文件合并，例如 `merge_same_schema`，并在输出包中记录声明的模式。 将不相关的模式保留在单独的源中。

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

仅当只有一部分数据包被批准用于共享时，才使用此方法。

## 说明：打包并审查好后的集合包，以便分享。

工作包目录包含子蓝图和受访问控制的审计文件。请勿整体复制该目录。在查看清单中的值和子蓝图后，创建一个单独的文件进行共享：

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

打包文件会保留运维人员提供的源 ID、标签、数据集组 ID 和审计路径元数据。请使用匿名值，检查打包后的 TOML，并且只通过批准的渠道传输。

## 配方：批量打包以供共享。

请按照[审查和分享指南](QUICKSTART.md#review-and-share)进行操作。将工作清单、审计记录和命令记录保存在本地；从已审查的打包好的Blueprint中，创建一个独立的目录。

```text
blueprint-share/
  customer-blueprint-bundle.packed.toml
```

## 方案：从已审阅 TOML 离线生成演示文稿

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

此模式只读取 TOML 文件并写出演示文稿。它会拒绝实时数据库选项，而不是静默忽略它们。

## 方案：字节级完全一致的可重现性

固定时间戳，并重复使用您持有的同一个受保护匿名化密钥：

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --anonymization-key-file /etc/dbwarp/anonymization.key \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

该密钥文件必须包含正好 32 个原始字节或 64 个十六进制字符，在 Unix 系统上不能为 group/world-readable，并且绝不能被共享。 如果不使用此选项，一个全新的、操作系统生成的密钥会故意在每次运行中改变匿名标签的顺序。 仅固定 `--generated-at` 是不够的。 请使用完整的配方来创建经过批准的取证快照；从完全相同的经过审查的 Blueprint 生成两次的输出，如果其时间戳和语言没有改变，则字节内容将完全相同。

## 配方：用于与 DBWarp 共享的软件包。

请按照[审查和分享指南](QUICKSTART.md#review-and-share)进行操作。默认包仅包含经过批准的Blueprint：

```text
blueprint-share/
  blueprint.toml
```

仅在单独审查和批准后，才添加 `blueprint.pptx`。 将审计记录、命令记录以及 credential/key 材料与共享目录隔离；仅在特定支持需求下，通过批准的安全渠道发送审计记录。
