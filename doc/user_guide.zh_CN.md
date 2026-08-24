# Qubit JSON 用户手册

`qubit-json` 是位于 Serde/serde_json 外层的资源感知边界。它不替代 serde_json parser，也不定义
新的 value tree；当输入、解码后 value 或输出必须受调用方预算约束时使用它。

## 选择入口

- 严格 JSON 文本使用 `JsonDecoder`，获得完整语法、数字范围和 value budget 准入。
- 仅在明确允许 BOM、空白、Markdown 围栏或控制字符规范化时使用
  `NormalizingJsonDecoder`；规范化后仍进入同一严格准入路径。
- 带预算紧凑输出使用 `JsonEncoder`。
- 当输入由另一个 Serde deserializer 持有时使用 `JsonValueSeed`；它核算已解码事件，但不能
  校验原始 JSON lexeme。
- 已有 `serde_json::Value` 使用 `JsonTreeReader`、`JsonTreeMutator` 或
  `JsonTreeBudgetTracker`。

## 严格解码与编码

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_input_bytes(4096)
    .max_number_bytes(20)
    .build();
let value: serde_json::Value =
    JsonDecoder::owned(limits).decode_str(r#"{"id":18446744073709551615}"#)?;
assert_eq!(value["id"], serde_json::json!(u64::MAX));
# Ok::<(), qubit_json::decode::JsonDecodeError<JsonResource>>(())
```

需要跨调用累计记账时显式构造 session。decode 失败后保留输入消耗，暂存的 value 消耗仅在
完整强类型成功后提交。buffered encode 提交完整输出；incremental encode 可能在 writer 中留下
已接受前缀。

## 数字限制

负整数必须装入 `i64`，非负整数必须装入 `u64`，小数与指数必须是有限 `f64`。边界、浏览器
`BigInt` 要求、精确十进制建议，以及 numeric range 与 `NumberBytes` 的区别见
[JSON 数字契约](number_contract.zh_CN.md)。

## 错误处理

严格 decode 区分 `Budget`、`Syntax` 和强类型 `Deserialize`；数字越界是语法领域的
`IntegerOutOfRange`/`FloatOutOfRange`，不是预算失败。严格 encode 区分预算、无效 RawValue、
Serde 序列化和 writer 失败。详细的规范化诊断可能含输入派生信息，只能在可信边界启用。

## 运行建议

所有不可信边界都应配置有限限制，尤其是输入/输出字节、深度、节点、集合大小、key/string
字节和 number 字节。标识符政策、十进制 scale、schema、权限等应用约束应在 JSON 准入之后
独立校验。
