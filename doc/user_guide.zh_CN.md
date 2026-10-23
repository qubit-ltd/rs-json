# Qubit JSON 用户手册

本手册面向使用 `qubit-json` 0.12、Rust 1.94+ 的服务、配置读取器和数据管道。它说明如何在
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

以下依赖版本与本 checkout 一致；使用尚未发布的本地改动时，改为对应的 `path` 依赖：

```toml
[dependencies]
qubit-json = "0.12"
qubit-budget = { version = "0.9.1", features = ["json"] }
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

成功分支会得到已经通过资源准入的 `serde_json::Value`。失败分支返回 `JsonDecodeErrorKind::Budget`，并报告本次观察到的 11 个输入字节。这个 accessor
描述本次调用观察到的字节数，不表示它们已提交到累计预算：被拒绝的输入计费会随失败的 value
attempt 回滚，先前已接受的输入计费则继续保留。HTTP 适配层可以依据稳定错误类别生成响应。

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

## 为签名和哈希生成规范化字节

应用为 JSON 签名或计算哈希时，即使对象成员的到达顺序不同，签名方和验证方也必须使用相同
的字节表示。开启可选 `jcs` feature，再把已经构造好的 `serde_json::Value` 交给 `JcsEncoder`：

```toml
[dependencies]
qubit-json = { version = "0.12", features = ["jcs"] }
serde_json = "1.0"
```

应用可以先规范化，再把结果字节传给现有的签名或哈希逻辑：

```rust
use qubit_budget::json::{JsonEncodeLimits, JsonResource};
use qubit_json::encode::JcsEncoder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(4096)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    let value = serde_json::json!({"b": 2, "a": 1.0});
    let canonical = encoder.to_vec(&value)?;
    assert_eq!(canonical, br#"{"a":1,"b":2}"#);
    Ok(())
}
```

该 feature 默认关闭。编码器接收 `Value`，不接收任意 `Serialize` 实现或原始 JSON 文本。读取原始
JSON 时，可使用 `JcsDecoder` 在构造 `Value` 时拒绝重复键；重复成员一旦合并到 map 中，编码器就无法再发现它们。
现有 `JsonDecoder` 用于严格 JSON 解码，但其整数文本范围不足以覆盖 JCS 接受的所有数字词法。
需要应用 JCS 数字和重复键规则时，原始文本应使用 `JcsDecoder`。

JCS 数值遵循 IEEE 754 双精度模型。包含 `9007199254740993` 的值可能规范化为
`9007199254740992`。协议必须精确保留的整数应表示成 JSON 字符串。

`to_vec` 和 `to_string` 返回完整结果。`write_buffered` 会先生成并检查规范化文档，再写入目标；
`write_incremental` 则直接写出已经生成的字节，并在成功前执行 flush。目标写入或 flush 失败时，
两种模式都可能留下已接受的前缀，且这些字节仍计入输出预算；值资源事务只在完整操作成功后提交。
`Budget` 表示值或输出预算拒绝操作，`WorkingMemory` 表示单独配置的 `max_working_bytes` 不足。
工作内存覆盖 renderer 遍历栈和活动对象成员表的辅助容量，不包含输入 `Value`、分配器元数据或输出缓冲容量。
`unlimited()` 会同时取消值、输出和工作内存限制，只应处理可信数据或已由其他层准入的数据。

### 按 JCS 规则解码原始文本

若输入来自原始 JSON 文本，使用 feature-gated 的 `JcsDecoder`，而不是先用普通 `JsonDecoder` 构造
`Value`。下面的例子对输入、结构和值数量都设定有限预算，再将结果交给 `JcsEncoder`：

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonEncodeLimits, JsonResource};
use qubit_json::decode::JcsDecoder;
use qubit_json::encode::JcsEncoder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let decode_limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .max_depth(16)
        .max_nodes(32)
        .max_map_entries(8)
        .max_number_bytes(32)
        .max_string_bytes(256)
        .max_key_bytes(64)
        .build();
    let mut decoder = JcsDecoder::with_limits(decode_limits);
    let value = decoder.decode_str(r#"{"b":100000000000000000000,"a":1.0}"#)?;

    let encode_limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(4096)
        .build();
    let mut encoder = JcsEncoder::with_limits(encode_limits, 16 * 1024);
    let canonical = encoder.to_vec(&value)?;
    assert_eq!(canonical, br#"{"a":1,"b":100000000000000000000}"#);
    Ok(())
}
```

数字不论输入写成整数、小数还是指数，都会按 binary64 舍入后存入 `Value`。例如
`9007199254740993` 会变成 `9007199254740992`；`1e400` 因结果非有限而拒绝，较小的数字可以下溢为零。
若金额或标识符必须逐位精确，应在 JSON 中用字符串表示，之后由应用按领域规则解析。

重复键比较的是 JSON 反转义后的键字符串。下面的断言说明，Unicode 转义可以拼出相同键；双反斜杠则会让反斜杠成为键名的一部分：

```rust
use qubit_json::decode::{JcsDecodeError, JcsDecoder};

let error = JcsDecoder::unlimited()
    .decode_str(r#"{"x":1,"\u0078":2}"#)
    .unwrap_err();
assert!(matches!(error, JcsDecodeError::DuplicateKey { .. }));
let value = JcsDecoder::unlimited()
    .decode_str(r#"{"x":1,"\\u0078":2}"#)
    .unwrap();
assert_eq!(value.as_object().unwrap().len(), 2);
```



即使使用无限 decoder，包含 128 层容器的文档也会触发固定构造上限：

```rust
use qubit_json::decode::{JcsDecodeError, JcsDecoder};

let nested = (0..128).fold(String::from("null"), |value, _| format!("[{value}]"));
let error = JcsDecoder::unlimited().decode_str(&nested).unwrap_err();
assert!(matches!(
    error,
    JcsDecodeError::MaterializationDepthLimit {
        limit: 127,
        line: 1,
        column: 128,
    }
));
```

按变体处理 JCS 错误：`Decode(source)` 包含普通输入、语法和预算错误；`DuplicateKey { line, column }`
指向第二个键的开引号；`MaterializationDepthLimit { limit, line, column }` 表示触发固定构造保护。
该上限为 127 层容器，与包含根值的 `max_depth` 不同。公开错误族还包括 `JcsEncodeError`、
`JsonSerializationError` 以及普通的 `JsonDecodeError` / `JsonEncodeError` 领域错误。

同一对象中的键按 JSON 反转义后的字符串查重，因此 `"x"` 与 `"\u0078"` 重复；嵌套对象也执行同样检查。
Unicode 不做 NFC/NFD 归一化，因而不同码点序列仍是不同键。合法 JSON 的空白和成员顺序不要求规范；
注释、BOM、尾逗号、非法 UTF-8、孤立代理项和尾随第二个文档会被拒绝。该入口不是“输入已规范”验证器。

`decode_input` 会读到 EOF 并缓冲完整输入以返回自有 `Value`。设置有限 `max_input_bytes` 时，读取逻辑可能
额外消耗一个探测字节，用于区分刚好到达限额的 EOF 与超限输入；该字节不计费、不进入缓冲，也无法放回输入源。
已经接受的输入字节即使后续解码失败仍计入 session，值资源则只在完整成功后提交。

资源预算限制被计量的 JSON 资源，不是精确的 malloc 上限：输入缓冲区、`Vec`/`Map` 容量和分配器元数据
不会被精确计入。词法深度按包含根值的 `max_depth` 计量。JCS Value 构造另有固定的 127 层容器上限：
根容器算第 1 层，标量不增加容器层数。即使使用 `JcsDecoder::unlimited()`，第 128 个容器仍会返回
`JcsDecodeError::MaterializationDepthLimit { limit, line, column }`。若配置的 `max_depth` 更低，则会先以
`JcsDecodeError::Decode(JsonDecodeErrorKind::Budget)` 拒绝。

## 进阶用法

只有在边界明确允许时才使用 `NormalizingJsonDecoder`，并通过策略选择 BOM、外围空白、一层
JSON Markdown 围栏或控制字符转义。规范化策略不携带 `JsonDecodeLimits`；请使用
`NormalizingJsonDecoder::with_limits(policy, limits)` 显式传入限制，或把
`JsonDecodeSession` 传给 `NormalizingJsonDecoder::new(policy, session)`。

严格解码和规范化解码的切片方法都会处理传入的完整 `&str` 或 `&[u8]`。输入字节限制只准入该切片，
无法追溯限制 HTTP body 聚合器或其他传输层已经分配的内存。`JsonDecoder` 的切片方法可以返回借用输入的
结果；一次性 `NormalizingJsonDecoder` 方法要求目标实现 `DeserializeOwned`，因为规范化文本可能是临时数据。
若需在规范化后借用文本，应先准备文档再解码，借用结果不能超过所引用的 `NormalizedJsonDocument` 生命周期。
`JsonDecoder::decode_input` 可从字节输入源解码拥有所有权的结果：

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecoder;
fn main() -> Result<(), JsonDecodeError<JsonResource>> {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut body = std::io::Cursor::new(br#"{"id":42}  "#);
    let request: serde_json::Value = decoder.decode_input(&mut body)?;
    assert_eq!(request["id"], 42);
    Ok(())
}
```

`decode_input` 接受 `Input<Item = u8>`，标准 `Read` 类型（例如 `Cursor`）可通过 `qubit-io`
集成直接使用。它会读取到 EOF，再对整份文档执行严格检查，包括末尾空白以及第二个 JSON 值。
配置 `max_input_bytes` 后，每次读取最多包含剩余额度加一个探针字节。探针用于区分恰好到达
限额时的 EOF 与后续仍有数据的情况。额度内字节会计费并暂存；若探针读到了数据，解码会以预算
错误结束。`Input` 无法把探针字节放回输入源，因此该字节既不计费，也不进入 JSON 缓冲区或错误中
报告的输入长度。返回类型必须实现 `DeserializeOwned`，因为方法内部会暂存整份文档。未设置输入字节
上限时，临时缓冲区没有配置最大值。
这个入口只解析普通严格 JSON；要将任意原始 JSON 规范化为 JCS，还需要适用的数字文本规则和重复键拒绝。

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
预算类型兼容的另一个解码器。规范化无需改写时，文档文本可以借用原始输入；需要改写时，文档会持有新分配的文本。
借用结果受准备文档的生命周期约束。借用规则遵循 Serde 的表示方式：未转义的 JSON 字符串可以借用文档内容，包含转义的字符串则必须构造
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

原生 `i64`、`u64` 和有限 `f64` 事件会保留各自的数字表示。启用
`arbitrary_precision` 的上游反序列化器可能把数字作为 map 事件提供；包装器无法判断这些事件的来源，
因此会把它们当作普通对象处理。先用 `JsonDecoder` 扫描文本也不会恢复来源信息。详见
[JSON 数字契约](number_contract.zh_CN.md)。JCS 使用 binary64，超出其精确范围的整数会舍入，
不能作为精确整数表示的通用替代方案。

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

## 数字入口、私有协议与 Display

三种数字边界要分开判断：

- **普通文本与 Serde 事件：** strict 文本要求负整数在 `i64` 范围、非负整数在 `u64` 范围，小数或指数可转换为有限 `f64`。序列化器和 seed 收到 `i128`、`u128` 事件时会显式检查范围。文本解码先准入 `NumberBytes`，再检查数值范围。通用 seed 看不到原始 token，不能执行词法长度限制；遇到 map 形状事件时，也无法判断它是真对象还是上游模拟的 serde_json 私有 Number map。
- **已经物化的 `Value` 树：** `JsonTreeReader` 按现有 Number 的紧凑显示内容长度计量，不先转 `f64`，也不改写数字。启用 `arbitrary_precision` 后，树读取器能按实际表示计量 `1e400` 或 39 位整数。这条计量规则不代表普通或 JCS encoder 会接受该数字。
- **JCS 文本：** `JcsDecoder` 对所有 token（包括整数词法）使用有限 IEEE-754 binary64。例如 `9007199254740993` 会舍入为 `9007199254740992`；`1e400` 这类溢出会被拒绝。解码还会拒绝反转义后重复的键，并保留 127 层容器物化保护。

encoder 识别 serde_json 的两种精确私有 struct 协议：`RawValue` 与 `Number`。真实 Number 协议只有一个字符串 payload，通过 `serialize_str` 或一次 `collect_str` 提交；文本在输出或 Value 构造前会完成验证。普通 map 即使含有 `$serde_json::private::Number` 键，仍按对象处理。手写协议结构不合法时返回 `JsonSerializationErrorKind::InvalidSerializerState`，其状态原因是 `JsonSerializerStateError::InvalidRawValueProtocol` 或 `InvalidNumberProtocol`。错误类型是穷举类型，下游 `match` 必须增加新变体处理。诊断不会保留私有 payload 文本。

把 `Display` 编码成 JSON 字符串或对象键时，collector 会先检查本次写入后完整字符串/键的预计测量值，再追加该分块。预检成功不扣预算；格式化完成后才对完整测量准入一次。这能阻止库的 collector 在字符串或键超过限制后继续增长，但库无法控制用户 `Display::fmt` 内部的任意分配、分配器元数据或进程 RSS。`RawValue`/Number 的原始输出 payload 仍要等完整文本收集后才能校验 JSON 结构；收集期间仍受输出字节上限约束。

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
| `Read` | 输入源在文档解码完成前发生读取错误 | `read_error_kind()`、`raw_input_bytes()`；仅详细诊断保留 `read_error()` |

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
| `SerializerContract` | `InvalidSerializerState`（含 `InvalidRawValueProtocol` 与 `InvalidNumberProtocol`）、`DisplayFormattingFailed` | `is_serializer_contract_error()`、`serializer_state_error()` |
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

脱敏诊断只保留稳定分类和源坐标，不保留意外字节、token、键/值文本或 parser/Serde 来源。
只有 `DiagnosticPolicy::Detailed` 会保留由输入产生的来源错误。严格解码通过
`JsonDecoder::with_diagnostic_policy(DiagnosticPolicy::Detailed)` 配置，规范化解码通过
`NormalizingJsonDecodePolicyBuilder` 配置。详细诊断只应在可信边界启用，不可信日志应保持
默认脱敏。可信调用方如需精确字节，应通过稳定 offset 检查自己保有的原始输入，错误对象本身
不会保留该字节。编码错误不会保留第三方 `Serialize::custom` 提供的任意文本。其他领域还
提供 `JsonEncodeError`、`JsonSyntaxError`、`JsonTreeProcessError` 和
`JsonTreeMutateError`。JCS 解码另返回 `JcsDecodeError`：普通输入和预算问题在 `Decode` 中，重复键及 127 层构造上限分别由
`DuplicateKey` 和 `MaterializationDepthLimit` 标识，并带有源坐标。

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
- `Read`：这是输入源故障，不是 JSON 语法错误。已经接受的字节继续计费，输入源不会自动倒带。
  脱敏诊断可读取 `read_error_kind()` 和 `raw_input_bytes()`；只有启用 `Detailed` 诊断才能读取原始来源。
  仅当输入源支持安全续读或回退时才重试。
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

- [0.12 迁移指南](migration_0_12.zh_CN.md) · [English migration guide](migration_0_12.md)

- [中文 README](../README.zh_CN.md) · [English README](../README.md)
- [English user guide](user_guide.md)
- [JSON 数字契约](number_contract.zh_CN.md)
- [已发布 API 文档](https://docs.rs/qubit-json)；当前分支 API 可运行
  `cargo doc --all-features --open` 生成
