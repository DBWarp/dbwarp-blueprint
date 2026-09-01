# 快速开始

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../QUICKSTART.md)。本译文不应被视为合同级文本。

**语言：** [English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | [Français](../fr/QUICKSTART.md) | [Español](../es/QUICKSTART.md) | [Polski](../pl/QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | **简体中文**

本快速入门面向需要生成可分享的 DBWarp Blueprint 文件、同时不暴露客户数据的售前工程师、DBA 或安全审查人员。

## 1. 选择工具运行方式

请选择以下方式之一：

- 下载发布二进制文件并验证其校验和。
- 使用 `./build.sh` 从源代码构建。
- 从带依赖源代码的发布包构建，以进行严格的离线依赖审查。

请参阅 [`../BUILD.md`](BUILD.md) 和 [`../binaries/README.md`](BINARIES.md)。

需要时显式选择显示语言：

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --dry-run
```

支持的值为 `en`、`de`、`fr`、`es`、`pl`、`ja` 和 `zh`。显示语言会改变帮助、提示、诊断、进度文本和演示文稿文字，但绝不会改变选项名称、可接受值、URI 方案、选择器、DBP 代码、审计键或 Blueprint TOML。请参阅 [`INTERNATIONALISATION.md`](INTERNATIONALISATION.md)。

## 2. 安全准备凭据

不要在连接 URI 中放置密码。工具会拒绝 URI 内嵌密码，以避免密码泄露到进程列表和 shell 历史记录中。

建议的密码文件方式（输入密钥时不会回显，也不会出现在 shell 历史记录中）：

```bash
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

如果用户名不便进行 URI 编码，也可将其放入文件：

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\\migration_user' > /etc/dbwarp/db.user
```

然后使用 `--user-file /etc/dbwarp/db.user`。

## 3. 先进行试运行

试运行会验证参数并打印计划操作，而不连接数据库：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

对于 `--from-toml` 演示文稿模式，试运行是本地预检，不会读取数据库。

对于多个客户源，请改为对批处理清单进行试运行：

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 4. 运行仅目录模式

仅目录模式读取元数据和统计信息，但不读取行样本：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

当策略禁止行采样，或希望先进行第一轮安全审查时，请使用此模式。

## 5. 选择非表对象详细级别

默认的 `--artifact-detail summary` 会读取非表对象目录，但不会读取对象定义。它输出有界计数和外部前提类别。如果策略禁止读取这些目录，请使用 `--artifact-detail none`。

匿名依赖拓扑使用 `graph`；有界语言特征和复杂度区间使用 `analyzed`。两者都需要明确同意：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


输出绝不包含对象名称、定义文本、端点、秘密、密钥、证书或二进制文件。在批准 graph 或 analyzed 模式前，请阅读 [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)。

## 6. 运行 Tier 2 压缩测量

Tier 2 将有界行样本读入内存，在本地压缩，只写出摘要比率，然后丢弃样本字节：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log blueprint.audit.txt
```

条件允许时请使用 Tier 2。它能让 DBWarp 更准确地估算网络传输字节数、出口成本以及合成文本/二进制数据生成。

## 7. 生成演示文稿

在实时运行期间：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --audit-log blueprint.audit.txt \
  --yes
```

也可在审阅后生成，不连接数据库：

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

## 8. 分享前审阅

审阅以下内容：

```bash
less blueprint.toml
less blueprint.audit.txt
unzip -l blueprint.pptx  # optional deck package inspection
```

预期特性：

- 不包含真实表名；
- 不包含真实列名；
- 不包含行值；
- 除固定文件头外不包含注释；
- 行数和字节大小已经舍入；
- 使用 `table-001`、`col-1` 和 `schema-A` 等匿名化 ID；
- 有界对象计数，以及在批准后提供的匿名对象 ID；
- 明确披露对象不完整或不可读的证据，而不是静默省略；
- 可选输出仅包含压缩比，不包含采样字节。

## 9. 交接给 DBWarp

最小交接内容：

```text
blueprint.toml
```

对于多源客户审查，请创建并检查打包后的捆绑包，而不要交接工作目录：

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

捆绑包元数据会保留批处理清单中选择的源 ID、标签和数据集组 ID。请使用匿名值，
并在传输前进行审阅。

当客户拥有多个数据库、多个 Parquet 或 Avro 数据集，或只希望批准选定的源/表用于基准测试生成时，请使用 `docs/BATCH_AND_BUNDLES.md`。

默认将以下内容作为受访问控制的本地证据保留：

```text
blueprint.audit.txt
blueprint.pptx
command-used.txt
```

审计和保存的命令可能包含数据库端点、已验证主体、本地路径、计时数据和清单源 ID。
仅在有特定支持需要时，才通过已批准的安全渠道发送。不要发送密码文件、CA 私钥、
客户转储或数据库日志。
