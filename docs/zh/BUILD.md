# 从源代码构建 dbwarp-blueprint

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../BUILD.md)。本译文不应被视为合同级文本。

**语言：** [English](../../BUILD.md) | [Deutsch](../de/BUILD.md) | [Français](../fr/BUILD.md) | [Español](../es/BUILD.md) | [Polski](../pl/BUILD.md) | [日本語](../ja/BUILD.md) | **简体中文**

本指南面向希望在工具连接数据库之前自行构建该工具的客户。

## 快速构建

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
./build.sh
```

二进制文件将写入：

```text
target/release/dbwarp-blueprint
```

其他示例均使用 `./dbwarp-blueprint`。从源代码构建后，请直接运行
`target/release/dbwarp-blueprint`，或先将该文件复制为 `./dbwarp-blueprint`。

如果尚未安装固定的 Rust 工具链，且允许经过审查的网络访问，请明确选择启用：

```bash
ALLOW_NETWORK=1 ./build.sh
```

## 构建脚本的作用

`build.sh` 被刻意设计得较为保守：

- 从 `rust-toolchain.toml` 读取固定的 Rust 版本
- 如果现有 `rustc` 与固定版本匹配，则使用该版本
- 除非设置 `ALLOW_NETWORK=1`，否则拒绝下载 Rust
- 固定 rustup 引导程序版本，并在使用前验证其官方 SHA-256
- 将工具链状态保存在 `./build/` 下
- 使用 Cargo.lock 实现可重现的依赖版本
- 默认使用 `cargo build --release --locked` 构建
- 从带依赖源代码的源代码包运行时，自动切换到 `--frozen --offline --locked`
- 如果不存在 `vendor-crates/`，则拒绝 `DBWARP_BLUEPRINT_OFFLINE=1`
- 打印生成的二进制文件的 SHA256
- 在审计中嵌入准确的源修订版本和工作树改动状态

它不使用 `sudo`，也不会修改系统的 Rust 安装。

## 可下载的二进制文件

为确保运行可重现，请固定确切的发布标签并验证其 SHA-256；不要使用可变的下载 URL。

Releases 页面提供预构建二进制文件：

<https://github.com/DBWarp/dbwarp-blueprint/releases>

提供这些文件是为了方便。如果您的策略要求审查源代码，请在本地从同一标签构建。

平台二进制归档是供操作人员使用的包，而不是源代码树，无法在其中直接重新构建。
归档中附带的本指南和 `verify.sh` 描述了使用匹配源代码进行验证的路径。如果需要
`build.sh`、Cargo 源代码或本地对比构建，请使用确切发布标签的检出，或该发布版本的
带依赖源代码归档。

发布文件：

| 平台 | 文件 |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## 验证下载的归档文件

Linux：

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS：

```bash
shasum -a 256 dbwarp-blueprint-macos-arm64.tar.gz
```

将输出值与 `SHA256SUMS.txt` 中对应的行比较。

Windows PowerShell：

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

## 身份验证专用构建

默认构建支持密码、令牌文件、令牌环境变量和 TLS 流程；PostgreSQL 和 MySQL 可使用客户端证书 mTLS。

SQL Server 集成身份验证具有平台专用构建：

| 平台 | 构建命令 | 用途 |
|---|---|---|
| Linux | `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Kerberos / GSSAPI |
| Windows | GitHub 发布的 Windows 二进制文件，或 `cargo build --release --features winauth` | Windows Integrated Auth / SSPI |

Linux Kerberos 需要常规 MIT Kerberos 运行时库。如果 `kinit` 可在主机上运行，所需的运行时组件通常已经存在。

## 不使用脚本构建

如果您的策略倾向于直接使用 Cargo 命令：

```bash
cargo build --release --locked
```

Windows SSPI 构建：

```powershell
cargo build --release --locked --features winauth
```

Linux Kerberos 构建：

```bash
cargo build --release --locked --features integrated-auth-gssapi
```

## 复现发布二进制文件

`./build.sh` 证明已审查的源代码能够构建；字节完全一致还需要发布的全部原生
构建输入。请使用 `PROVENANCE.json` 中记录的准确源修订、目标和功能列表、固定
Rust 工具链、记录的原生编译器/链接器、以提交时间戳设置的
`SOURCE_DATE_EPOCH`，以及发布工作流的路径重映射和链接器标志。Windows 发布
还使用 `clang-cl` 和 `/Brepro`。

复现这些输入后，将解压的发布二进制文件与本地结果比较：

```bash
SOURCE_BIN=target/release/dbwarp-blueprint \
  ./verify.sh /path/to/extracted/dbwarp-blueprint
```

如果哈希不同，不要将二进制文件视为等同。发布 CI 会在同一 runner 上使用不同
Cargo target 目录构建两次并拒绝字节差异；`PROVENANCE.json` 记录评估本地复现
所需的信息。

## 带依赖源代码的依赖项

常规仓库在 `vendor/mysql_async` 下包含一个小型补丁依赖项，使 MySQL `--tls-ca` 具有与工具其余部分相同的限制性信任语义。所有其他依赖项版本均由 `Cargo.lock` 固定。

每个 GitHub Release 都会单独发布 `dbwarp-blueprint-source-vendored.tar.gz` 包，供希望离线检查每个依赖项源文件并进行构建的安全团队使用。

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

该包包含带补丁的 `vendor/mysql_async`、包含所有其他依赖项的已生成 `vendor-crates/` 树，以及将 crates.io 重定向到本地依赖树的已生成 `.cargo/config.toml`。在该模式下，`build.sh` 使用 `cargo build --release --frozen --offline --locked`。
