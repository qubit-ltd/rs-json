# Qubit JSON 用户手册

本手册面向使用 `qubit-json` 0.8、Rust 1.94+ 的服务、配置读取器和数据管道。它说明如何在
调用方预算约束下准入 JSON，同时保留 Serde 数据模型；它不替代 `serde_json`，也不负责应用
schema。

[English](user_guide.md) · [README](../README.zh_CN.md) · [API 文档](https://docs.rs/qubit-json/0.8.0/qubit_json/)

## 手册目标与读者

当信任边界需要明确限制输入大小、嵌套深度、集合大小、number token 大小、解码后 value 或
输出字节时使用本 crate。schema、权限、标识符策略和十进制精度仍应在 JSON 准入之后由应用层
独立校验。

## 概念模型

`qubit-budget` 负责资源标识、限制、transaction 和 session；`qubit-json` 提供四个公开领域：

- `decode`：准入严格 JSON 文本，或先按明确 policy 做文本规范化。
- `encode`：生成带预算的 JSON 输出。
- `value`：从 Serde 事件构造或校验物化的 `serde_json::Value` tree。
- `value::traverse`：不依赖 Rust 递归地读取或修改已有 tree。

严格 decoder/encoder 是有状态对象。隔离运行可使用 `owned(limits)`；需要跨调用累计记账时，
传入 `JsonDecodeSession`/`JsonEncodeSession`。解码后 value 与 buffered 输出的消耗在文档规定的
成功边界暂存并提交；decode 失败后的输入消耗会保留在 session 中。

## 贯穿场景：HTTP 边界上的有界 JSON

假设一个 endpoint 接收包含标识符和小型 payload 的 JSON。服务希望在构造业务状态前拒绝超大
请求，同时保留完整的无符号 64 位标识符范围。

### 安装与最小配置

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.3", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 核心工作流

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_input_bytes(4096)
    .max_number_bytes(20)
    .build();
let mut decoder = JsonDecoder::owned(limits);
let value: serde_json::Value =
    decoder.decode_str(r#"{"id":18446744073709551615,"ok":true}"#)?;
assert_eq!(value["id"], serde_json::json!(u64::MAX));
# Ok::<(), qubit_json::decode::JsonDecodeError<JsonResource>>(())
```

可观察结果是已准入的 `serde_json::Value`；超限会在 typed value 提交前返回
`JsonDecodeError::Budget`。语法或数字范围问题返回 `JsonDecodeError::Syntax`，目标类型不匹配
返回 `JsonDecodeError::Deserialize`。

输出侧使用 `JsonEncoder::owned(JsonEncodeLimits::...)`，再调用 `to_vec`、`write_buffered` 或
`write_incremental`。buffered 只有完整字节序列准备好后才提交；incremental 的 writer 失败时，
可能已经留下被接受的输出前缀。

## 进阶用法

只有在边界明确允许时才使用 `NormalizingJsonDecoder`，并通过 policy 选择 BOM、外围空白、一层
JSON Markdown 围栏或控制字符转义。policy 不携带 `JsonDecodeLimits`；`owned` 显式接收 limits，
`from_session` 显式接收 session。

当另一个 Serde deserializer 持有输入时使用 `JsonValueSeed`。它核算解码事件，但看不到原始
number lexeme、`NumberBytes` 或文本级整数范围；需要这些保证时使用 `JsonDecoder`。

对已有 value，`JsonTreeReader::account` 在调用方 transaction 中暂存整棵树的消耗，不调用
visitor。`JsonTreeMutator` 先准入原始 tree，再执行原地 visitor 回调，最后准入修改后的 tree。
错误分别是 `JsonTreeMutateError::InputBudget`、`::Visitor` 和 `::OutputBudget`；visitor 或输出
预算失败不会回滚已经完成的修改。visitor 可返回 `JsonTreeControl::SkipSubtree` 跳过后代回调，
但最终输出记账仍覆盖修改后 tree 的全部后代。

## 错误与诊断

公开错误领域包括 `NormalizingJsonDecodeError`、`JsonDecodeError`、`JsonEncodeError`、
`JsonSyntaxError`、`JsonTreeProcessError` 和 `JsonTreeMutateError`。先区分错误领域，再决定重试、
返回客户端错误或记录诊断。规范化诊断可能保留输入派生信息；`DiagnosticPolicy::Detailed` 只应
在可信边界启用，不可信日志默认脱敏。

数字契约独立于资源限制：负整数装入 `i64`，非负整数装入 `u64`，小数/指数必须是有限 `f64`。
超出范围或必须避免二进制浮点舍入的精确十进制，应使用字符串或显式领域表示。详见
[JSON 数字契约](number_contract.zh_CN.md)。

## 排障

- `Budget`：检查对应的输入字节、number 字节、深度、节点、集合、key/string 或输出限制；只有
  需要累计记账时才继续复用该 session。
- `Syntax`：检查原始字节以及错误中的 reason、offset、line、column；规范化不会凭空补齐 JSON
  语法。
- `Deserialize`：说明 JSON 已准入，但与目标类型不匹配；分别修正 payload 或目标 schema。这也包括
  `serde_json` 的具象化递归保护：词法校验使用显式栈，可能准入一个类型化反序列化不会具象化的文档。
- tree 修改后出现部分结果：`JsonTreeMutator` 是增量式的，visitor 或输出预算失败不会回滚已有
  修改。

## 限制与最佳实践

所有不可信边界都配置有限限制，至少覆盖输入/输出字节、深度、节点、集合大小、key/string
字节和 number 字节。不要仅为省去配置而使用无限 session。将应用校验与资源准入分离；超过
JavaScript 安全整数范围的 wire number 是否转为 `BigInt`，由浏览器协议处理。
同时保留 Serde 的深度保护。关闭它只会把深度不可信输入转移到 Rust 调用栈，并不能保证任意目标
反序列化器安全。

## 延伸阅读

- [中文 README](../README.zh_CN.md) · [English README](../README.md)
- [English user guide](user_guide.md)
- [JSON 数字契约](number_contract.zh_CN.md)
- [API 文档](https://docs.rs/qubit-json/0.8.0/qubit_json/)
