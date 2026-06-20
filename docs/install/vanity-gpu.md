# 靓号生成器 GPU 说明

本文说明 BTCC 靓号生成器当前的 GPU 实现方式、使用边界，以及为什么 GPU 结果没有助记词。

## 当前实现

当前 GPU 靓号生成已经改成进程内直接调用 Rust 方法，不再通过外部命令：

- 不再使用 `Command::new("vgen")`
- 不再依赖外部 `vgen.exe`
- 不再通过 stdout / json 回传结果

当前调用链是：

1. UI 选择 `GPU`
2. 钱包程序在后台任务里直接调用 `vgen` crate
3. `vgen` 在当前客户端进程内初始化 GPU runner
4. GPU 按 BTCC 规则完成靓号匹配
5. 命中后返回私钥结果
6. 本程序再把结果恢复成 BTCC 钱包对象

相关代码：

- [src/wallet/vanity_gpu.rs](../../src/wallet/vanity_gpu.rs)
- [src/wallet/keys.rs](../../src/wallet/keys.rs)
- [examples/wallet/src/ui/btcc/vanity_generator.rs](../../examples/wallet/src/ui/btcc/vanity_generator.rs)

## 不阻塞主线程

GPU 搜索不会阻塞 UI 主线程。

当前实现里：

- 页面点击“开始生成”后，搜索逻辑运行在 `background_spawn` 后台任务里
- 进程内 `tokio` runtime 也是在后台任务里创建和执行
- 主线程只负责界面刷新、进度显示和停止指令

因此当前实现满足两点：

- 直接方法调用
- 不阻塞主线程

## 不同系统的 GPU 后端

不同系统可用的 GPU 后端不一样：

- Windows：常见是 `Vulkan`、`DX12`
- macOS：常见是 `Metal`
- Linux：常见是 `Vulkan`
- 兼容兜底：`OpenGL`

代码里统一抽象为：

- `Auto`
- `Vulkan`
- `Metal`
- `Dx12`
- `Gl`

## 当前匹配语义

当前 GPU 路径已经按 BTCC 地址规则匹配，不再先匹配 Bitcoin 的 `bc1...` 再回转。

也就是说，当前 GPU 和 CPU 的匹配语义是一致的：

- 前缀
- 后缀
- 前后缀

都会按 BTCC 地址 `cc1...` 的实际结果判断。

## 为什么 GPU 结果没有助记词

这是当前实现的设计结果，不是 bug。

原因是：

1. GPU 路径当前搜索的是私钥空间
2. 命中后返回的是私钥结果
3. 程序再把这个私钥恢复成 WIF 钱包

所以 GPU 结果当前天然包含的是：

- BTCC 地址
- 公钥
- `WIF` 私钥

不会包含助记词。

### 根本原因

助记词钱包和 WIF 钱包不是同一条生成路径：

- 助记词钱包：`熵 -> 助记词 -> seed -> 派生私钥 -> 地址`
- GPU 当前路径：`直接搜索私钥 -> 地址`

私钥命中后，不能反推出原始助记词，所以当前 GPU 结果无法打印助记词。

### 和 CPU 的差别

CPU 当前的助记词模式是：

1. 先生成助记词
2. 再按派生路径得到地址
3. 地址命中后，输出助记词

GPU 当前不是这条路径，所以只适合输出：

- 地址
- WIF

如果以后要做“GPU 命中后也能输出助记词”，就不能继续只搜索私钥，而要改成基于熵或助记词空间的搜索。这会明显增加实现复杂度，也会改变性能特征。

## 钱包保存方式

GPU 模式当前保存的是 WIF 钱包，保存内容主要是：

- BTCC 地址
- 公钥
- `WIF` 私钥

因此在数据语义上，GPU 结果应视为：

- `WIF` 靓号钱包

而不是：

- 助记词靓号钱包

## 当前限制

当前为了稳定接入库调用，工作区里对 vendored `vgen` / `wgpu` 做了本地收敛处理，避免 Windows 上 `DX12` 依赖冲突把整个项目编译打坏。

当前结论：

- `Auto`、`Vulkan`、`Metal`、`OpenGL` 是优先支持路径
- `DX12` 当前不建议作为首选验证路径

如果后续要把 `DX12` 也作为稳定路径，需要继续处理 `wgpu-hal` 和 `windows` 依赖版本冲突。

## 验证方式

当前工程已通过：

```bash
cargo check -p wallet
```

如果要验证运行效果，直接在靓号生成器页面切到 `GPU` 后启动搜索即可。
