# 数据库表结构

运行时 SQLite 数据库默认位于：

```text
db/btcc_litedesk.sqlite
```

开发模式默认读取项目根目录下这份数据库。安装版会根据运行平台改到用户数据目录。

数据库连接与 migration 入口在：

```text
src/db/mod.rs
```

业务读写主要集中在：

```text
src/db/btcc_wallet.rs
```

## 表清单

| 表名 | 文档 | 说明 |
| --- | --- | --- |
| `btcc_wallets` | [`btcc_wallets.md`](./btcc_wallets.md) | BTCC 钱包元数据、余额缓存、加密助记词和加密 WIF |

## 维护约定

- 新增或修改表结构时，同步更新对应文档
- UI 层不直接拼 SQL，统一通过 `src/db/` 暴露函数读写
- 运行时生成的 SQLite 文件不提交到 Git
