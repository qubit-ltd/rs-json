# 从 0.3 迁移到 0.8

[English](migration_0_3_to_0_8.md)

0.8 是尚未发布的重新设计，不是可直接替换的升级。它把严格 JSON 准入、受控文本规范化、
编码、物化值和遍历拆分为不同领域，并要求在信任边界显式配置预算。

## 开发期依赖

```toml
[dependencies]
qubit-json = { version = "0.8", git = "https://github.com/qubit-ltd/rs-json.git", branch = "main" }
qubit-budget = { version = "0.4", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
```

可复现构建应使用 `rev` 固定提交，而不是跟随 `branch`。

## 重命名与模块映射

| 0.3 | 0.8 |
| --- | --- |
| `qubit_json::LenientJsonDecoder` | `qubit_json::decode::NormalizingJsonDecoder` |
| `qubit_json::JsonDecodeOptions` | `qubit_json::decode::NormalizingJsonDecodePolicy` 与 `JsonDecodeLimits` |
| `qubit_json::JsonDecodeError*` | `qubit_json::decode::JsonDecodeError*` |
| `qubit_json::JsonTopLevelKind` | `qubit_json::decode::JsonRootKind` |
| strict option preset | `qubit_json::decode::JsonDecoder` |

decoder 方法现在接收 `&mut self`，因为 decoder 持有累计记账状态。每个独立边界应新建 decoder；
只有确实需要累计限制时才复用。

## 规范化输入

以前：

```rust,ignore
use qubit_json::LenientJsonDecoder;

let decoder = LenientJsonDecoder::default();
let value = decoder.decode_value("```json\n{\"ok\":true}\n```")?;
```

现在：

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::{NormalizingJsonDecodePolicy, NormalizingJsonDecoder};

let policy = NormalizingJsonDecodePolicy::default();
let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_input_bytes(4096)
    .max_depth(32)
    .max_nodes(256)
    .build();
let mut decoder = NormalizingJsonDecoder::with_limits(policy, limits);
let value: serde_json::Value =
    decoder.decode_str("```json\n{\"ok\":true}\n```")?;
assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::decode::JsonDecodeError<JsonResource>>(())
```

当结果需要借用规范化文档、需要使用 Serde seed，或需要多次解码同一份规范化文本时，先调用
`prepare_str` 或 `prepare_utf8`。

## 严格 JSON

输入契约不允许清理展示痕迹时使用 `JsonDecoder`。该类型没有隐式无限预算默认值：不可信
边界调用 `with_limits`；只有可信或已经完成准入的输入才显式调用 `unlimited()`。严格 decoder
收到原始空输入时返回 `InvalidJson`；`EmptyInput` 表示配置的规范化流程没有产生文档。

## 错误

解码错误对资源与数量类型使用泛型，并公开稳定的 `kind`、`stage`、预算、语法、位置和诊断
策略信息。诊断默认脱敏；只有显式选择 `DiagnosticPolicy::Detailed` 才保留详细 Serde 来源。

编码使用 `JsonEncodeError`。需要把错误移入其他错误模型的调用方，应穷尽匹配
`JsonEncodeError::into_source()`，不要再把 `kind()` 与可选 `into_*` 提取器组合使用。

## 新增领域

- `encode::JsonEncoder` 使用 value 与 output 预算执行严格编码。
- `value::JsonValueEncoder` 从 Serde 事件构造严格 `serde_json::Value`。
- `value::DuplicateKeyRejectingJsonValue` 强制对象键唯一。
- `value::traverse` 提供带事务记账的迭代 reader 与原地 mutator。

迁移安全敏感边界前，请阅读[用户手册](user_guide.zh_CN.md)、
[数字契约](number_contract.zh_CN.md)和[设计文档](json_design.zh_CN.md)。

