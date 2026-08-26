# `qubit-json` 产品需求

## 问题

Rust 服务需要处理两类 JSON：来自受控协议的严格字节流，以及来自文本通道、可能带围栏或
少量明确噪声的输入。两类输入都必须受资源限制约束，且调用方需要区分预算、语法、类型和
输出错误，而不是面对重名或跨领域的历史错误类型。

## 产品边界

产品由四个领域组成：

| 领域 | 交付能力 | 不承担的责任 |
| --- | --- | --- |
| `decode` | 可配置规范化、严格解码和 lexical admission | 推测或修复任意损坏 JSON |
| `encode` | 严格对象式 encoder | 文本修复或隐式 session |
| `value` | 从 Serde 事件构造并记账 Value | 代替原始文本的语法验证 |
| `value::traverse` | 非递归访问、修改和完整 tree 记账 | 回滚已执行的可变 visitor 变更 |

资源限制、预算和 session 属于 `qubit-budget`；本 crate 不重复定义这些通用概念。

## 用户契约

### 宽松输入

用户通过 `NormalizingJsonDecodePolicy` 配置允许的规范化，并把预算作为独立的
`JsonDecodeLimits` 或 `JsonDecodeSession` 显式交给 `NormalizingJsonDecoder`。policy 不得携带预算，
decoder 不提供隐式默认预算。默认错误脱敏；只有明确请求 `Detailed` 才保留可能包含输入的信息。
带 session 的调用必须保留原始与规范化输入消耗，并仅在完整类型解码后提交 value 消耗。
一次性规范化解码面向 owned 目标；借用、seed 和重复物化通过
`NormalizedJsonDocument` 两阶段接口完成。prepare 只记一次输入，每次 document decode 独立记账并提交 value。

### 严格文本

用户必须以对象方式传入调用方持有的 session：`JsonDecoder` 负责 `decode`、`decode_seed`
和 `validate`，`JsonEncoder` 负责 `to_vec`、`write_buffered` 和 `write_incremental`。严格
输入不经过任何修复；每个 document 的记账边界由相应 session transaction 定义。

JSON 数字的产品边界为：负整数 `i64`、非负整数 `u64`、小数/指数有限 `f64`。超过
JavaScript 安全整数但仍在 64 位范围内的标识符允许作为 number 传输，前端必须使用保持整数
精度并映射 BigInt 的 parser；`n` 后缀不属于 JSON。更宽整数和精确十进制使用字符串或显式
领域 wire。`NumberBytes` 只限制 token 资源，不改变数值范围。

### Value 与 tree

用户可用 `JsonValueSeed` 在 `JsonValueTransaction` 中构造 `serde_json::Value`。用户可用
`JsonTreeReader` 或 `JsonTreeMutator` 对物化 value 进行无 Rust 递归的遍历；reader 可在调用方已有
transaction 中仅记账而不调用 visitor、不提交 transaction，`JsonTreeBudgetTracker` 复用该路径为
完整 tree 反复记账。mutator 在调用 visitor 前先对完整输入 tree 执行预算准入；输入 tree
任一部分超限时，本次操作直接失败且 visitor 不执行。visitor 成功返回后，实现再对完整输出
tree 独立计量和准入，但不承诺回滚已提交的业务变更。

## 错误契约

两个 decoder facade 返回统一的 `JsonDecodeError<R, Q>`，不提供旧错误兼容层。公开错误和诊断
类型按领域归属：

1. `JsonDecodeError`：严格和规范化解码，通过 kind/stage/accessor 暴露稳定结构。
2. `JsonEncodeError`：严格预算、原始 JSON、序列化、写入。
3. `JsonSyntaxError`：稳定的语法原因和位置。
4. `JsonTreeProcessError`：reader tree 的预算和 visitor 错误。
5. `JsonTreeMutateError`：mutator 的输入预算、visitor 和输出预算错误。

## 验收标准

- 对外模块为 `decode`、`encode`、`value`；tree 能力位于
  `value::traverse`，共享实现保持 crate-private。
- 严格 text API 的所有操作经由 decoder/encoder 对象，而非公开自由函数。
- session-aware decode 在失败后保留输入消耗，value 暂存消耗只在完整成功后提交。
- value 和 tree 可独立用于已物化 JSON；tree 遍历不依赖 Rust 调用栈深度。
- 文档与示例只描述当前四领域、统一解码错误模型和 `0.8` 安装方式。
- 依赖图不启用 serde_json arbitrary precision，旧私有 Number marker 始终是普通 object key。
