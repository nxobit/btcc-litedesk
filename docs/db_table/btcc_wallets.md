# btcc_wallets

`btcc_wallets` 是 BTCC 钱包列表表，用于保存钱包列表页面所需的本地钱包元数据、余额缓存，以及加密后的助记词和私钥。

## 字段

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | 本地钱包记录 ID |
| `name` | `TEXT NOT NULL` | 钱包名称 |
| `address` | `TEXT NOT NULL UNIQUE` | BTCC 地址 |
| `network` | `TEXT NOT NULL DEFAULT 'Bitcoin-Classic (BTCC)'` | 网络名称 |
| `derivation_path` | `TEXT NOT NULL DEFAULT ''` | 派生路径，例如 `m/84'/0'/0'/0/0` |
| `source_type` | `TEXT NOT NULL DEFAULT 'watch'` | 来源：`generated`、`mnemonic`、`wif`、`watch` |
| `public_key` | `TEXT NOT NULL DEFAULT ''` | 公钥 |
| `encrypted_mnemonic` | `BLOB` | 加密后的助记词 |
| `encrypted_wif` | `BLOB` | 加密后的 WIF 私钥 |
| `balance_sats` | `INTEGER NOT NULL DEFAULT 0` | 确认余额，单位 sats |
| `unconfirmed_sats` | `INTEGER NOT NULL DEFAULT 0` | 未确认余额，单位 sats |
| `utxo_count` | `INTEGER NOT NULL DEFAULT 0` | UTXO 数量 |
| `last_synced_at` | `TEXT` | 最近余额同步时间 |
| `note` | `TEXT NOT NULL DEFAULT ''` | 备注 |
| `is_active` | `INTEGER NOT NULL DEFAULT 1` | 是否有效，软删除使用 |
| `created_at` | `TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP` | 创建时间 |
| `updated_at` | `TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP` | 更新时间 |

## 索引

| 索引 | 字段 | 说明 |
| --- | --- | --- |
| `idx_btcc_wallets_active_updated` | `is_active, updated_at DESC` | 钱包列表读取 |
| `idx_btcc_wallets_address` | `address` | 地址查询与去重 |

## 列表查询规则

正常列表读取时会过滤：

- `is_active = 1`
- `address <> '__btcc_wallet_vault__'`

默认排序：

```sql
ORDER BY created_at DESC, id DESC
```

搜索规则：

- 名称：`LIKE %关键词%`
- 地址：完整匹配

## 内部记录

表中有一条内部记录用于钱包密码校验，地址固定为：

```text
__btcc_wallet_vault__
```

约定如下：

- `name = __vault__`
- `public_key = Argon2 哈希值`
- `is_active = 0`

这条记录不会出现在钱包列表中。

## 安全约定

- 数据库不保存明文助记词
- 数据库不保存明文 WIF 私钥
- `encrypted_mnemonic` 和 `encrypted_wif` 使用 `src/wallet/pbe.rs` 中的本地加密逻辑写入
- 钱包密码仅用于校验与解密，不以明文形式落库

### PBE 升级补充

- 历史密文使用 `PBKDF2-HMAC-SHA512 + AES-256-CBC + 1000` 次迭代
- 当前新密文使用 `PBKDF2-HMAC-SHA512 + AES-256-CBC + 20000` 次迭代
- 新密文格式为 `v2$20000$<payload>`，显式记录版本和迭代次数
- 解密时兼容旧密文；旧钱包在解锁成功或单条密钥解密成功后，会静默重写为新格式
