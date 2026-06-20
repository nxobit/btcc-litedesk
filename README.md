# BTCC Litedesk

BTCC Litedesk 是一个基于 Rust 的桌面钱包示例项目，当前以 BTCC 单链钱包管理为主，提供钱包列表、创建、导入、单笔发送、批量转账、接收二维码和主题切换等功能。

## 当前状态

- 桌面应用入口：`examples/wallet`
- 核心钱包与数据库逻辑：`src/wallet`、`src/db`
- 当前版本：`0.1.1`

## 主要文档

- [主要功能](docs/主要功能.md)
- [项目结构](docs/项目结构.md)
- [数据库表结构](docs/db_table/README.md)
- [btcc_wallets 表](docs/db_table/btcc_wallets.md)
- [项目执行记录](docs/项目执行/11-钱包生成.md)

## 运行与构建

### 开发运行

```bash
cargo run -p wallet
```

### 检查编译

```bash
cargo check -p wallet
```

### Release 构建

```bash
cargo build --release -p wallet
```

## 数据与配置路径

### 开发模式

- 数据库：`db/btcc_litedesk.sqlite`
- 主题目录：`examples/wallet/themes`

### 安装版

#### Windows

- 数据库：`%LOCALAPPDATA%\BTCC Litedesk\db\btcc_litedesk.sqlite`
- 配置：`%LOCALAPPDATA%\BTCC Litedesk\config\setings.json`
- 日志：`%LOCALAPPDATA%\BTCC Litedesk\logs\wallet-panic.log`

#### macOS

- 数据目录：`~/Library/Application Support/BTCC Litedesk`
- 主题目录：`.app/Contents/Resources/themes`

## 打包

### Windows

- Inno Setup 脚本：`packaging/windows/BTCC-Litedesk.iss`
- 打包脚本：`scripts/build_windows_installer.ps1`
- 输出目录：`dist/`

### macOS

- 打包脚本：`scripts/build_macos_app.sh`
- 输出产物：`.app`、`.dmg`
- 当前为未签名测试版，首次下载后可能需要手动放行

## 技术栈

- Rust 2024
- GPUI
- gpui-component
- SQLite / rusqlite
- bip39
- reqwest
- serde / serde_json

## 免责说明

本项目仅用于学习、演示和功能验证，不构成任何投资建议或资产托管承诺。

- 请自行保管助记词、私钥和钱包密码
- 请在理解风险的前提下使用
- 作者不对资产损失负责
