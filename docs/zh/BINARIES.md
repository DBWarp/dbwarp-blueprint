# 下载 dbwarp-blueprint

> 本文档由机器辅助翻译，尚待中文技术专家审校。请参阅[规范英文原文](../../binaries/README.md)。本译文不应被视为合同级文本。

**语言：** [English](../../binaries/README.md) | [Deutsch](../de/BINARIES.md) | [Français](../fr/BINARIES.md) | [Español](../es/BINARIES.md) | [Polski](../pl/BINARIES.md) | [日本語](../ja/BINARIES.md) | **简体中文**

预构建的 `dbwarp-blueprint` 二进制文件发布在 GitHub Releases 页面：

<https://github.com/DBWarp/dbwarp-blueprint/releases>

您可以下载二进制文件、验证其校验和、在本地运行它，并在与 DBWarp 分享任何内容之前检查生成的 `blueprint.toml`。

请选择确切的发布标签，例如 `https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0`，然后从同一标签下载归档文件和 `SHA256SUMS.txt`。对于可重现或受审计的运行，请勿使用可变的 `releases/latest` URL。

## 文件

| 平台 | 文件 |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |
| 离线源代码审计包 | `dbwarp-blueprint-source-vendored.tar.gz` |
| 校验和 | `SHA256SUMS.txt` |

每个发布版本还包括 `SHA256SUMS.txt`。

## 验证下载内容

Linux/macOS：

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell：

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

将打印的哈希值与 `SHA256SUMS.txt` 中对应的行进行比较。

## 下载二进制文件还是本地构建？

提供可下载的二进制文件是为了方便。最强的信任路径仍是从源代码构建：

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
git checkout <release-tag>
./build.sh
```

该常规源代码克隆被刻意保持精简，并使用 `Cargo.lock` 固定依赖项版本。

如果您的策略要求在构建前审查每个依赖项源文件，请从同一发布版本下载 `dbwarp-blueprint-source-vendored.tar.gz`，并在解压后的树中构建：

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

请参阅 [`BUILD.md`](BUILD.md)。

## 工具的作用

`dbwarp-blueprint` 读取数据库元数据，并可选择在少量本地样本上测量压缩率。它会为 DBWarp 迁移估算写入匿名化文本文件。它不会上传该文件，也不会发送遥测。
