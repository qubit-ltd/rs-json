# `qubit-json` 产品需求

## 问题

Rust 服务需要处理两类 JSON：来自受控协议的严格字节流，以及来自文本通道、可能带围栏或
少量明确噪声的输入。两类输入都必须受资源限制约束，且调用方需要区分预算、语法、类型和
输出错误，而不是面对重名或跨领域的历史错误类型。

## 产品边界

产品由四个领域组成：

| 领域 | 交付能力 | 不承担的责任 |
| --- | --- | --- |
| `lenient` | 可配置规范化和直接 Serde 解码 | 推测或修复任意损坏 JSON |
| `text` | 严格对象式 decoder/encoder | 文本修复或隐式 session |
| `value` | 从 Serde 事件构造并记账 Value | 代替原始文本的语法验证 |
| `tree` | 非递归访问、修改和完整 tree 记账 | 回滚已执行的可变 visitor 变更 |

资源限制、预算和 session 属于 `qubit-budget`；本 crate 不重复定义这些通用概念。

## 用户契约

### 宽松输入

用户通过 `NormalizingJsonDecodeOptions` 配置允许的规范化，再创建 `NormalizingJsonDecoder`。默认错误
脱敏；只有明确请求 `Detailed` 才保留可能包含输入的信息。带 session 的调用必须保留原始与
规范化输入消耗，并仅在完整类型解码后提交 value 消耗。

### 严格文本

用户必须以对象方式传入调用方持有的 session：`JsonDecoder` 负责 `decode`、`decode_seed`
和 `validate`，`JsonEncoder` 负责 `to_vec`、`write_buffered` 和 `write_incremental`。严格
输入不经过任何修复；每个 document 的记账边界由相应 session transaction 定义。

### Value 与 tree

用户可用 `JsonValueSeed` 在 `JsonValueTransaction` 中构造 `serde_json::Value`。用户可用
`JsonTreeReader` 或 `JsonTreeMutator` 对物化 value 进行无 Rust 递归的遍历；用
`JsonTreeBudgetTracker` 为完整 tree 反复记账。mutator 可以跳过被预算拒绝的子树，但不承诺
回滚已提交的业务变更。

## 错误契约

所有公开错误按领域归属，不提供根级聚合或同名兼容层：

1. `NormalizingJsonDecodeError`：宽松规范化和类型解码。
2. `JsonDecodeError`：严格预算、语法、类型解码。
3. `JsonEncodeError`：严格预算、原始 JSON、序列化、写入。
4. `JsonSyntaxError`：稳定的语法原因和位置。
5. `JsonTreeProcessError`：tree 的预算和 visitor 错误。

## 验收标准

- 对外模块只有 `lenient`、`text`、`value`、`tree`；共享实现保持 crate-private。
- 严格 text API 的所有操作经由 decoder/encoder 对象，而非公开自由函数。
- session-aware decode 在失败后保留输入消耗，value 暂存消耗只在完整成功后提交。
- value 和 tree 可独立用于已物化 JSON；tree 遍历不依赖 Rust 调用栈深度。
- 文档与示例只描述当前四领域、五个 error 和 `0.8` 安装方式。
