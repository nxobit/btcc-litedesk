# Release SHA256 校验

这个项目的公开 release 除了安装包本身，还应同时发布对应的 `sha256` 校验文件。

建议发布时一起上传：

- Windows: `BTCC-Litedesk-Setup-<version>.exe`
- Windows: `BTCC-Litedesk-Setup-<version>.exe.sha256`
- macOS: `BTCC-Litedesk-macos-<arch>-<version>.dmg`
- macOS: `BTCC-Litedesk-macos-<arch>-<version>.dmg.sha256`

校验文件内容使用标准格式：

```text
<sha256>  <filename>
```

例如：

```text
0123456789abcdef...  BTCC-Litedesk-Setup-0.1.1.exe
```

## 已实现

当前打包脚本已经自动生成校验文件：

- [scripts/build_windows_installer.ps1](E:/rust/btcc-litedesk/scripts/build_windows_installer.ps1)
- [scripts/build_macos_app.sh](E:/rust/btcc-litedesk/scripts/build_macos_app.sh)

行为如下：

1. 打包完成后自动计算安装包的 `SHA-256`
2. 在安装包旁边生成同名 `.sha256` 文件
3. macOS 和 Windows 都会把版本号带入产物文件名
4. 打包前会校验根 [Cargo.toml](E:/rust/btcc-litedesk/Cargo.toml) 和 [examples/wallet/Cargo.toml](E:/rust/btcc-litedesk/examples/wallet/Cargo.toml) 的版本是否一致

## 发布流程

### Windows

执行：

```powershell
./scripts/build_windows_installer.ps1
```

产物位于 `dist/`：

- `BTCC-Litedesk-Setup-<version>.exe`
- `BTCC-Litedesk-Setup-<version>.exe.sha256`

### macOS

执行：

```bash
./scripts/build_macos_app.sh
```

产物位于 `dist/macos/`：

- `BTCC-Litedesk-macos-<arch>-<version>.dmg`
- `BTCC-Litedesk-macos-<arch>-<version>.dmg.sha256`

## 用户如何校验

### Windows

在 PowerShell 中：

```powershell
$hash = (Get-FileHash .\BTCC-Litedesk-Setup-0.1.1.exe -Algorithm SHA256).Hash.ToLower()
Get-Content .\BTCC-Litedesk-Setup-0.1.1.exe.sha256
```

比较两者是否一致。

### macOS

在终端中：

```bash
shasum -a 256 BTCC-Litedesk-macos-arm64-0.1.1.dmg
cat BTCC-Litedesk-macos-arm64-0.1.1.dmg.sha256
```

比较两者是否一致。

## 和开源代码一致，具体怎么验证

`SHA-256` 本身只能证明：

- 你下载到的文件，和发布者上传的文件一致
- 发布过程中产物没有被替换或损坏

它不能单独证明：

- 这个二进制一定是从公开源码构建出来的

如果要验证“release 和开源代码一致”，建议流程是：

1. 发布时固定 git tag 或 commit
2. 从该 tag/commit 拉取源码
3. 在尽量一致的构建环境中本地重新打包
4. 比较你本地构建产物的 `SHA-256` 和 release 附带的 `.sha256`

如果两者一致，才能较强地说明：

- 发布产物和该份公开源码构建结果一致

## 后续建议

如果你希望这件事更严格，可以继续做两件事：

1. 在 release 页面明确写出对应的 git tag / commit
2. 把构建环境固定下来，例如 Rust toolchain、目标平台、依赖版本、打包脚本版本

再往前一步，就是把发布流程放进 CI，让 CI 从 tag 自动构建并自动附带 `sha256` 文件。
