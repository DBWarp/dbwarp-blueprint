# 快速开始

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../QUICKSTART.md)。本译文不应被视为合同级文本。

**语言：** [English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | [Français](../fr/QUICKSTART.md) | [Español](../es/QUICKSTART.md) | [Polski](../pl/QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | **简体中文**

这个快速入门指南是为数据库管理员或安全审查人员准备的，他们需要生成一个可共享的 DBWarp Blueprint 文件，而无需暴露数据。

## 1. 选择工具运行方式

请选择以下方式之一：

- 下载发布二进制文件并验证其校验和。
- 使用 `./build.sh` 从源代码构建。
- 从带依赖源代码的发布包构建，以进行严格的离线依赖审查。

请参阅 [`../BUILD.md`](BUILD.md) 和 [`../binaries/README.md`](BINARIES.md)。

需要时显式选择显示语言：

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --schema app --dry-run
```

支持的值为 `en`、`de`、`fr`、`es`、`pl`、`ja` 和 `zh`。显示语言会改变帮助、提示、诊断、进度文本和演示文稿文字，但绝不会改变选项名称、可接受值、URI 方案、选择器、DBP 代码、审计键或 Blueprint TOML。请参阅 [`INTERNATIONALISATION.md`](INTERNATIONALISATION.md)。

## 2. 配置专用最小权限账户

请在任何实时连接前完成此步骤，包括稍后会用于采集的 `--dry-run` 示例。
不要从应用所有者、管理员、超级用户、`root`、`sa` 或 `db_owner` 账户开始。

1. 明确确切的引擎和版本、数据库以及获批的一个或多个架构。
2. 选择采集级别：`basic` 仅用于表目录，`standard` 用于添加有限数量的行样本，或 `enhanced` 用于同时分析非表对象。
3. 由 DBA 复制 `sql/grants/<engine>/` 下对应的脚本，编辑所有标记的数据库、
   架构、主体、密码和角色开关值，并通过正常变更控制流程执行脚本。
4. 使用该脚本创建的专用账户，并在每条实时命令中为每个获批架构传入一个
   `--schema NAME` 选项，以保持相同的获批范围。
5. 审查采集结果后，请数据库管理员检查并运行 `sql/revoke/` 下与所选数据库引擎对应的撤销脚本，以删除账户和权限。

这些脚本会明确区分精确限定范围的授权和更方便的内置角色，并说明角色范围过宽
的情况。可执行脚本请参阅
[`../../sql/grants/README.md`](../../sql/grants/README.md)，按版本说明的 DBA／
安全依据请参阅
[`../../sql/grants/DATABASE_PERMISSIONS.md`](../../sql/grants/DATABASE_PERMISSIONS.md)。
采集器本身不会创建、扩大权限或删除数据库主体。

## 3. 安全准备凭据

不要在连接 URI 中放置密码。工具会拒绝 URI 内嵌密码，以避免密码泄露到进程列表和 shell 历史记录中。

建议的密码文件方式（输入密钥时不会回显，也不会出现在 shell 历史记录中）：

```bash
sudo install -d -m 700 -o "$USER" -g "$(id -gn)" /etc/dbwarp
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

如果用户名不便进行 URI 编码，也可将其放入文件：

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\migration_user' > /etc/dbwarp/db.user
```

然后使用 `--user-file /etc/dbwarp/db.user`。

## 4. 先进行试运行

试运行会验证参数并打印计划操作，而不连接数据库：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

对于 `--from-toml` 演示文稿模式，试运行是本地预检，不会读取数据库。

对于多个源，请先对批处理清单进行试运行：

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 5. 运行仅目录模式

仅目录模式读取元数据和统计信息，但不读取行样本：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

当策略禁止行采样，或希望先进行第一轮安全审查时，请使用此模式。

## 6. 选择非表对象详细级别

默认的 `--artifact-detail summary` 会读取非表对象目录，但不会读取对象定义。它输出有界计数和外部前提类别。如果策略禁止读取这些目录，请使用 `--artifact-detail none`。仅计数的拓扑探测仍会运行；请参见[权限参考](../../sql/grants/README.md#topology-evidence)。

匿名依赖拓扑使用 `graph`；有界语言特征和复杂度区间使用 `analyzed`。两者都需要明确同意：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


输出绝不包含对象名称、定义文本、端点、秘密、密钥、证书或二进制文件。在批准 graph 或 analyzed 模式前，请阅读 [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md)。

## 7. 运行 Tier 2 压缩测量

Tier 2 将有界行样本读入内存，计算压缩、NULL 密度、基数/频率、长度和样式的
聚合测量值，然后丢弃采样值：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
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

在可能的情况下，请使用第二层（Tier 2）。它能提供更准确的传输大小和出口费用估算。

## 8. 生成演示文稿

在实时运行期间：

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
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

## 9. 分享前审阅

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
- 可选的压缩、NULL 密度、基数/频率、长度和样式聚合测量值，绝不包含采样值。

## 10. 与 DBWarp 共享。

最少需要分享的内容：

```text
blueprint.toml
```

对于多个源，请创建并检查一个打包好的集合，而不是共享工作目录：

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

捆绑包元数据会保留批处理清单中选择的源 ID、标签和数据集组 ID。请使用匿名值，
并在传输前进行审阅。

如果您的数据库数量较多，或者有多个 Parquet 或 Avro 数据集，或者您只想共享部分源数据库或表，请参阅[批量收集和蓝图包](BATCH_AND_BUNDLES.md)。

### 审阅与分享

默认只分享已审阅的 `blueprint.toml` 或打包后的捆绑包。演示文稿只有在其内容和保密级别经过审阅，并依据组织政策单独获批后，才可一并提供。

请将审计记录、命令记录以及未经批准的报告保存在本地，并进行访问控制。它们可能包含终端地址、已验证的身份、本地路径、时间数据以及清单标识符。仅在特定支持需求下，通过已批准的安全渠道发送这些信息。切勿将密码文件、令牌文件、匿名化密钥、CA 私钥、数据库备份或数据库日志与共享的 Blueprint 一起发送。
