# Qubit JSON 用户手册

本手册面向使用 `qubit-json` 0.8、Rust 1.94+ 的服务、配置读取器和数据管道。它说明如何在
调用方预算约束下准入 JSON，同时保留 Serde 数据模型；它不替代 `serde_json`，也不负责应用
数据模式。

[English](user_guide.md) · [README](../README.zh_CN.md) · [已发布 API 文档](https://docs.rs/qubit-json)

## 手册目标与读者

当信任边界需要明确限制输入大小、嵌套深度、集合大小、数字原始文本长度、解码后值或输出
字节时使用本库。数据模式、权限、标识符策略和十进制精度仍应在 JSON 准入之后由应用层独立
校验。

## 概念模型

`qubit-budget` 负责资源标识、限制、事务和会话；`qubit-json` 提供四个公开领域：

- `decode`：准入严格 JSON 文本，或先按明确策略做文本规范化。
- `encode`：生成带预算的 JSON 输出。
- `value`：从 Serde 事件构造或校验已经生成的 `serde_json::Value` 值树。
- `value::traverse`：不依赖 Rust 递归地读取或修改已有值树。

严格解码器和编码器都是有状态对象。使用 `with_limits(limits)` 可以创建新的独立会话；需要跨
调用累计用量时，则通过 `new(session)` 传入 `JsonDecodeSession` 或 `JsonEncodeSession`。解码
结果和缓冲输出的资源用量
先在事务中暂存，到文档规定的成功边界才提交；解码失败后，输入用量仍会保留在会话中。

## 实战场景：HTTP 边界上的有界 JSON

假设一个 HTTP 接口接收包含标识符和小型载荷的 JSON 请求体。成功标准是：完整支持无符号
64 位标识符；在提交解码结果前拒绝过大的请求体；只把通过资源准入的数据交给应用层校验。

### 安装与最小配置

默认使用 crates.io 上的当前最新版 `0.8`；本地开发时也可以把版本号替换为 `path`：

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.4", features = ["json"] }
serde_json = "1.0"
```

如果应用需要解码到通过 derive 生成的类型，再添加
`serde = { version = "1.0", features = ["derive"] }`。

### 核心工作流

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;

fn main() -> Result<(), JsonDecodeError<JsonResource>> {
    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .max_depth(32)
        .max_nodes(256)
        .max_sequence_items(64)
        .max_map_entries(64)
        .max_key_bytes(128)
        .max_string_bytes(2048)
        .max_number_bytes(20)
        .max_payload_bytes(4096)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);

    let request_body = br#"{"id":18446744073709551615,"ok":true}"#;
    let value: serde_json::Value = decoder.decode_utf8(request_body)?;
    assert_eq!(value["id"], serde_json::json!(u64::MAX));

    let small_limits = limits.into_builder().max_input_bytes(8).build();
    let mut small_decoder = JsonDecoder::with_limits(small_limits);
    let error = small_decoder
        .decode_utf8::<serde_json::Value>(br#"{"ok":true}"#)
        .expect_err("the request body must exceed eight bytes");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.raw_input_bytes(), 11);
    Ok(())
}
```

成功分支会得到已经通过资源准入的 `serde_json::Value`。失败分支在提交解码结果前返回
`JsonDecodeErrorKind::Budget`，同时保留测得的原始输入长度。HTTP 适配层因此可以根据稳定的
错误类别生成响应，无须依赖解析器的私有实现细节。

输出侧使用 `JsonEncoder::with_limits(JsonEncodeLimits::...)`，再调用 `to_vec`、`write_buffered` 或
`write_incremental`：

```rust
use qubit_budget::json::JsonEncodeLimits;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use serde_json::json;

let value = json!({"status": "ok"});
let mut buffered = JsonEncoder::with_limits(
    JsonEncodeLimits::builder().max_output_bytes(64).build(),
);
assert_eq!(buffered.to_vec(&value)?, br#"{"status":"ok"}"#);

let mut incremental = JsonEncoder::with_limits(
    JsonEncodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_output_bytes(4)
        .build(),
);
let mut output = Vec::new();
let error = incremental
    .write_incremental(&mut output, &[1, 2, 3])
    .expect_err("four output bytes cannot hold the complete array");
assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
assert_eq!(output, b"[1,2");
# Ok::<(), qubit_json::encode::JsonEncodeError<qubit_budget::json::JsonResource>>(())
```

缓冲模式会先完成序列化和预算检查，再接触写入器；只有完整写入成功后才提交资源用量，但 I/O
失败仍可能在外部写入器中留下部分字节。增量模式在后续序列化、预算或写入操作失败时，会像
上例一样保留已经接受的前缀。

下一步通常是把已准入文档反序列化为应用类型，再执行模式、权限和标识符规则。如果边界还
需要文本规范化、重复解码、对象键唯一性或值树处理，请继续阅读后续章节。

## 进阶用法

只有在边界明确允许时才使用 `NormalizingJsonDecoder`，并通过策略选择 BOM、外围空白、一层
JSON Markdown 围栏或控制字符转义。规范化策略不携带 `JsonDecodeLimits`；请使用
`NormalizingJsonDecoder::with_limits(policy, limits)` 显式传入限制，或把
`JsonDecodeSession` 传给 `NormalizingJsonDecoder::new(policy, session)`。

严格解码器和规范化解码器当前都接收完整的 `&str` 或 `&[u8]`。输入字节限制只负责准入调用方
已经提供的切片，不能追溯限制 HTTP body 聚合器或其他传输层已经分配的内存。向解码器交付完整
输入前，应先在外层传输边界设置有界读取或 body 聚合上限。

### 已准备的规范化文档

一次性规范化方法只能返回自有数据，因为临时规范化文本无法跨越调用继续存活。需要让结果
借用原文、通过 Serde seed 解码或重复构造结果时，应先显式准备文档：

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;

fn main() -> Result<(), JsonDecodeError> {
    let limits = JsonDecodeLimits::builder()
        .max_input_bytes(1024)
        .max_normalized_input_bytes(1024)
        .max_depth(16)
        .max_nodes(64)
        .max_sequence_items(32)
        .max_map_entries(32)
        .max_key_bytes(128)
        .max_string_bytes(512)
        .max_number_bytes(32)
        .max_payload_bytes(1024)
        .build();
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::lenient(),
        limits,
    );
    let document = decoder.prepare_str("  \"borrowed\"  ")?;
    let first: &str = decoder.decode_precharged_document(&document)?;
    let second: &str = decoder.decode_precharged_document(&document)?;
    assert_eq!((first, second), ("borrowed", "borrowed"));
    Ok(())
}
```

`prepare_str` 和 `prepare_utf8` 会立即提交原始输入与规范化输入的用量。
`decode_precharged_*` 与 `validate_precharged_document` 的名称明确表达了这个输入计量
前置条件。每次成功调用 `decode_precharged_document`、
`decode_precharged_document_seed`、`decode_precharged_object_document`、
`decode_precharged_array_document` 或 `validate_precharged_document`，都会分别提交一份值资源
用量；构造结果失败时，只回滚本次尝试暂存的值资源。准备后的文档不依赖创建它的解码器，可以交给
预算类型兼容的另一个解码器。借用规则
遵循 Serde 的表示方式：未转义的 JSON 字符串可以借用文档内容，包含转义的字符串则必须构造
自有数据。

当输入由另一个 Serde 反序列化器持有时，可以使用 `AccountingJsonValueSeed`。它会核算解码事件，但看不到
数字的原始文本、`NumberBytes` 或文本级整数范围；需要这些保证时应使用 `JsonDecoder`。
如果边界还要求特定的顶层容器，请使用 `decode_object_str`、`decode_object_utf8` 或对应的数组方法。

### 重复对象键

严格准入不会额外施加全局键唯一规则。重复键如何处理取决于 Serde 目标类型：
`serde_json::Value` 与 `serde_json::Map` 采用“后值覆盖前值”，许多派生结构体则会拒绝重复的
已知字段。因此，应通过目标类型表达文档契约，不能把严格准入理解为自动校验键唯一。

动态 JSON 若要求每层对象的键都唯一，可将严格文本准入与
`DuplicateKeyRejectingJsonValue` 组合：

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::value::DuplicateKeyRejectingJsonValue;

fn main() -> Result<(), JsonDecodeError<JsonResource>> {
    let input = r#"{"role":"user","role":"admin"}"#;
    let limits = JsonDecodeLimits::builder()
        .max_input_bytes(1024)
        .max_depth(16)
        .max_nodes(64)
        .max_sequence_items(16)
        .max_map_entries(16)
        .max_key_bytes(64)
        .max_string_bytes(256)
        .max_number_bytes(32)
        .max_payload_bytes(1024)
        .build();

    let mut ordinary_decoder = JsonDecoder::with_limits(limits);
    let ordinary: serde_json::Value = ordinary_decoder.decode_str(input)?;
    assert_eq!(ordinary["role"], "admin");

    let mut unique_key_decoder = JsonDecoder::with_limits(limits);
    let error = unique_key_decoder
        .decode_str::<DuplicateKeyRejectingJsonValue>(input)
        .expect_err("duplicate object keys must be rejected");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    Ok(())
}
```

该包装类型会递归检查嵌套对象；如果输入契约本来就包含文本规范化，也可将它作为
`NormalizingJsonDecoder` 的目标类型。调用方自行持有 Serde 反序列化器时，可使用对应的
`DuplicateKeyRejectingJsonValueSeed`。

对于已有值，`JsonTreeReader::account` 会在调用方事务中暂存整棵树的资源用量，但不会调用
访问器。`JsonTreeMutator` 先准入原始值树，再执行原地访问器回调，最后准入修改后的值树：

```rust
use std::convert::Infallible;

use qubit_budget::json::JsonValueLimits;
use qubit_json::value::traverse::{
    JsonTreeContext, JsonTreeControl, JsonTreeMutVisitor, JsonTreeMutator,
};
use serde_json::{Value, json};

struct RemoveSecrets;

impl JsonTreeMutVisitor for RemoveSecrets {
    type Error = Infallible;

    fn visit(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
        if let Value::Object(entries) = value {
            entries.remove("secret");
        }
        Ok(JsonTreeControl::Descend)
    }
}

let limits = JsonValueLimits::<qubit_budget::json::JsonResource, usize>::builder()
    .max_nodes(32)
    .max_payload_bytes(256)
    .build();
let mut input_budget = limits.budget();
let mut output_budget = limits.budget();
let mut input = input_budget.transaction();
let mut output = output_budget.transaction();
let mut value = json!({"name": "qubit", "secret": "remove me"});

JsonTreeMutator::new(&mut input, &mut output)
    .process(&mut value, &mut RemoveSecrets)
    .expect("both trees fit the configured budgets");
input.commit().expect("input accounting commits");
output.commit().expect("output accounting commits");
assert_eq!(value, json!({"name": "qubit"}));
```

错误分别是 `JsonTreeMutateError::InputBudget`、`::Visitor` 和 `::OutputBudget`；访问器或输出
预算失败不会回滚已经完成的修改。访问器可以返回 `JsonTreeControl::SkipSubtree` 跳过后代回调，
但最终输出记账仍会覆盖修改后值树的全部后代。

## 错误与诊断

严格解码器与规范化解码器返回同一个泛型 `JsonDecodeError`。先根据 `kind()` 返回的稳定类别
分支处理：

| `JsonDecodeErrorKind` | 含义 | 可读取的结构化信息 |
| --- | --- | --- |
| `Budget` | 配置的资源限制拒绝了本次测量 | `budget_error()`、`raw_input_bytes()`、`normalized_input_bytes()` |
| `EmptyInput` | 配置的规范化流程没有产生 JSON 文档 | `stage()`、输入字节数 |
| `InvalidUtf8` | 字节输入不是合法 UTF-8 | `utf8_valid_up_to()`、`utf8_error_len()` |
| `InvalidJson` | JSON 语法或数字契约不合法 | `syntax_error()`、`line()`、`column()` |
| `UnexpectedTopLevel` | 仅接受对象或数组的方法收到错误的根类型 | `expected_top_level()`、`actual_top_level()` |
| `Deserialize` | 已准入文档无法构造为请求的 Rust 类型 | `line()`、`column()`，启用详细诊断后还可读取来源错误 |

适配层取得错误所有权后，也可以调用 `into_source()`，对 `JsonDecodeErrorSource` 做穷举匹配，
不需要克隆预算或语法错误详情。

`JsonValueEncoder` 与 `JsonEncoder` 共用隐私安全的 `JsonSerializationError` 错误模型。恢复策略应读取
`JsonSerializationError::category()`，需要精确原因时读取 `kind()`。常用错误组有便捷谓词，
仅在适用时返回细节的访问器则避免调用方解析文本：

| 类别 | 代表性精确原因 | 便捷方法/细节 |
| --- | --- | --- |
| `Number` | `IntegerOutOfRange`、`NonFiniteFloat`、`InvalidNumberRepresentation` | `is_number_error()`、`integer_signedness()` |
| `ObjectKey` | `UnsupportedMapKey`、`DuplicateObjectKey` | `is_map_key_error()`、`map_key_kind()` |
| `RawValue` | `InvalidRawValue` | `is_raw_value_error()` |
| `Capacity` | `CollectionLengthOverflow` | `collection_kind()` |
| `SerializerContract` | `InvalidSerializerState`、`DisplayFormattingFailed` | `is_serializer_contract_error()`、`serializer_state_error()` |
| `Custom` | `CustomSerialization` | 有意保持不透明：不保留序列化器提供的任意文本 |

`stage()` 会准确指出公开处理边界：

| `JsonDecodeStage` | 处理边界 |
| --- | --- |
| `Input` | 核算原始输入字节 |
| `DecodeText` | 校验字节输入是否为 UTF-8 |
| `Normalize` | 转换文本或核算规范化后的字节 |
| `Admission` | 准入解码后值所需的资源 |
| `Parse` | 校验 JSON 语法和数字范围 |
| `TopLevelCheck` | 检查根节点是否为要求的对象或数组 |
| `Deserialize` | 构造请求的 Rust 类型 |

只有 `DiagnosticPolicy::Detailed` 会保留由输入产生的来源错误。严格解码通过
`JsonDecoder::with_diagnostic_policy(DiagnosticPolicy::Detailed)` 配置，规范化解码通过
`NormalizingJsonDecodePolicyBuilder` 配置。详细诊断只应在可信边界启用，不可信日志应保持
默认脱敏。编码错误不会保留第三方 `Serialize::custom` 提供的任意文本。其他领域还提供
`JsonEncodeError`、`JsonSyntaxError`、
`JsonTreeProcessError` 和 `JsonTreeMutateError`。

数字契约独立于资源限制：负整数装入 `i64`，非负整数装入 `u64`，小数/指数必须是有限 `f64`。
超出范围或必须避免二进制浮点舍入的精确十进制，应使用字符串或显式领域表示。详见
[JSON 数字契约](number_contract.zh_CN.md)。

## 排障

- `Budget`：检查对应的输入字节、数字字节、深度、节点、集合、键、字符串或输出限制；只有
  需要累计记账时才继续复用该会话。
- `NormalizingJsonDecoder` 返回 `EmptyInput`：确认配置的空白、BOM 或 Markdown 围栏
  处理是否移除了全部输入。原始空输入直接交给严格 `JsonDecoder` 时归类为 `InvalidJson`。
- `InvalidUtf8`：查看 `utf8_valid_up_to()` 和 `utf8_error_len()`，先在传输边界拒绝或修复字节流，
  再进入 JSON 解析。
- `InvalidJson`：检查原始字节以及错误中的原因、偏移量、行号和列号；规范化不会凭空补齐 JSON
  语法。
- `UnexpectedTopLevel`：比较 `expected_top_level()` 与 `actual_top_level()`，修正请求体，或改用
  根类型约束与输入一致的解码方法。
- `Deserialize`：说明 JSON 已准入，但与目标类型不匹配；分别修正请求内容或目标数据模式。这也包括
  `serde_json` 的构造递归保护：词法校验使用显式栈，可能准入一个目标类型无法继续构造的文档；
  目标类型为 `DuplicateKeyRejectingJsonValue` 时，重复对象键也在这一边界报告。
- 值树修改后出现部分结果：`JsonTreeMutator` 是增量式的，访问器或输出预算失败不会回滚已有
  修改。

## 限制与最佳实践

所有不可信边界都应配置有限限制，至少覆盖输入/输出字节、深度、节点、集合大小、键、字符串
和数字字节。不要仅为省去配置而使用无限会话。将应用校验与资源准入分离；线上协议中的数字
超过 JavaScript 安全整数范围时，是否转为 `BigInt` 应由浏览器端协议决定。
同时保留 Serde 的深度保护。关闭它只会把深度不可信输入转移到 Rust 调用栈，并不能保证任意目标
反序列化器安全。

## 延伸阅读

- [中文 README](../README.zh_CN.md) · [English README](../README.md)
- [English user guide](user_guide.md)
- [JSON 数字契约](number_contract.zh_CN.md)
- [已发布 API 文档](https://docs.rs/qubit-json)；当前分支 API 可运行
  `cargo doc --all-features --open` 生成
