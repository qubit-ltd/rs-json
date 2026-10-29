# Qubit JSON

[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

面向 Rust 服务、配置读取器和数据管道的资源感知 JSON 基础设施。它在保留 Serde 数据模型的
同时，为 `serde_json` 补充调用方可控的资源准入，防止不可信输入在解析、值构造或输出阶段
无限制地消耗资源。输入必须是严格 JSON 时使用 `JsonDecoder`；如果输入边界明确允许对外部
文本中的 JSON 做受控清理，则使用 `NormalizingJsonDecoder`。

## 安装

本 checkout 对应的 `0.12` 版本：

```toml
[dependencies]
qubit-json = "0.12"
qubit-budget = { version = "0.10", features = ["json"] }
serde_json = "1.0.135"
```

本地 checkout：

```toml
[dependencies]
qubit-json = { version = "0.12", path = "../rs-json" }
qubit-budget = { version = "0.10", features = ["json"] }
serde_json = "1.0.135"
```

如果需要解码到通过 derive 生成的应用类型，再添加
`serde = { version = "1.0", features = ["derive"] }`。

要生成符合 RFC 8785 的规范化字节，请显式开启 `jcs` feature：

```toml
qubit-json = { version = "0.12", features = ["jcs"] }
```

## 快速开始：解码已聚合且受大小限制的 HTTP body

下面的示例处理的是传输层已设置大小上限并聚合完成的 HTTP body 字节。它接收一个包含完整
`u64` 范围标识符的 JSON，再验证超出 JSON 输入上限的内容会在解码结果提交前被拒绝：

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
    let value: serde_json::Value =
        decoder.decode_utf8(br#"{"id":18446744073709551615,"ok":true}"#)?;
    assert_eq!(value["id"], serde_json::json!(u64::MAX));

    let small_limits = limits.into_builder().max_input_bytes(8).build();
    let mut small_decoder = JsonDecoder::with_limits(small_limits);
    let error = small_decoder
        .decode_utf8::<serde_json::Value>(br#"{"ok":true}"#)
        .expect_err("the request body must exceed eight bytes");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    Ok(())
}
```

与只调用 `serde_json::from_slice` 相比，这个边界会显式执行资源准入，并通过 `kind()` 返回稳定的
错误类别。上面的切片产生之前，传输层必须先限制 body 大小再进行聚合。应用调用顺序是：传输层
有界聚合 → `JsonDecoder` 准入 JSON → 模式、权限和领域校验。

`qubit-json` 不提供 HTTP body 聚合 API。需要查看下游 HTTP 接入方式时，可参考
[`rs-web` controller 示例](https://github.com/qubit-ltd/rs-web/blob/main/examples/controller_crud.rs)；该链接是可能随下游仓库变化的参考示例。更多输入方式见[用户手册的 `decode_input` 章节](doc/user_guide.zh_CN.md#进阶用法)。

准入预算和对象/数组检查针对源 JSON，Serde 目标可能构造出不同形态的值。启用 `raw_value` 时，普通
`serde_json::Value` 会特殊处理首个 `$serde_json::private::RawValue` 键；需要保留字面对象时，请先阅读
[用户手册的重复键章节](doc/user_guide.zh_CN.md#重复对象键)并选择合适的目标类型。
直接反序列化 `SafeJsonValue` 也无法识别私有数字 map；启用 `arbitrary_precision` 后，数字形态可能随
feature 配置变化。解码 JSON 文本时请使用 `JsonDecoder::decode_value_*`，详见[受保护值指南](doc/user_guide.zh_CN.md#受保护值验证解码与排版)。

## 选择解码边界

选择入口时应以输入契约为准，而不是看某份数据能否碰巧解析成功：

| 输入契约 | API |
| --- | --- |
| 输入必须已经是完整、严格的 JSON | `JsonDecoder` |
| 拥有结果所有权、拒绝重复键，并在 feature 合并后仍提供原生数字事件 | `JsonDecoder::decode_unique_str` / `decode_unique_utf8` / `decode_unique_seed_utf8` |
| 从字节输入源读取到 EOF，并生成拥有所有权的结果 | `JsonDecoder::decode_input` |
| 严格解码前允许清理指定的展示痕迹 | 使用显式策略构造的 `NormalizingJsonDecoder` |
| 需要检查规范化结果、重复解码，或让结果借用规范化文本 | 先调用 `NormalizingJsonDecoder::prepare_str` / `prepare_utf8`，再由 `NormalizingJsonDecoder::decode_precharged_document`、`decode_precharged_object_document` 或 `decode_precharged_array_document` 解码 |

默认规范化策略可以清理首尾空白、一个 BOM、一层 JSON Markdown 围栏，以及字符串内未经转义
的 ASCII 控制字符。这只是受控规范化，不是另一种 JSON 方言；注释、尾随逗号、未加引号的键
和缺失的 JSON 语法仍会被拒绝。

## 为签名生成规范化字节

如果应用需要为哈希或签名生成稳定字节，应开启 `jcs`，并把已经构造好的
`serde_json::Value` 交给 `JcsEncoder`：

```rust
# #[cfg(feature = "jcs")]
use qubit_budget::json::{JsonEncodeLimits, JsonResource};
# #[cfg(feature = "jcs")]
use qubit_json::encode::JcsEncoder;

# #[cfg(feature = "jcs")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(4096)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    let value = serde_json::json!({"b": 2, "a": 1.0});
    assert_eq!(encoder.to_vec(&value)?, br#"{"a":1,"b":2}"#);
    Ok(())
}
# #[cfg(not(feature = "jcs"))]
# fn main() {}
```

`jcs` 默认关闭。规范化对象是 `Value` 数据模型，不接受任意 `Serialize` 值或原始 JSON 文本。
用普通解码器处理原始文本时，若契约要求拒绝重复对象键，必须在构造 `Value` 前完成检查；一旦键已合并，
就无法再找回重复项。`JcsDecoder` 会在物化期间执行这项检查。
JCS 数值遵循 IEEE 754 双精度语义，因此 `9007199254740993` 可能输出为
`9007199254740992`。若协议必须保留整数的每一位十进制数字，应将整数编码为 JSON 字符串。
普通 `JsonDecoder` 也不接受所有 JCS 数字词法形式作为原始文本。

需要从原始文本开始时，可在 `jcs` feature 下使用 `JcsDecoder`。深树建议使用
`decode_value_str`、`decode_value_utf8` 或 `decode_value_input`，它们直接返回
`SafeJsonValue`。旧的 `decode_str`、`decode_utf8` 和 `decode_input` 仍兼容保留，返回裸
`serde_json::Value`。

```rust
# #[cfg(feature = "jcs")]
use qubit_json::decode::JcsDecoder;
# #[cfg(feature = "jcs")]
use qubit_json::encode::JcsEncoder;
# #[cfg(feature = "jcs")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let owner = JcsDecoder::unlimited()
        .decode_value_str(r#"{"b":2,"a":1.0}"#)?;
    let mut encoder = JcsEncoder::unlimited();
    let canonical = encoder.to_vec(owner.as_value())?;
    assert_eq!(canonical, br#"{"a":1,"b":2}"#);
    Ok(())
}
# #[cfg(not(feature = "jcs"))]
# fn main() {}
```

JCS 解码会把所有数字舍入为有限 binary64，并拒绝反转义后重复的对象键。有限预算示例
以及舍入、Unicode、深度和输入缓冲边界见[用户手册](doc/user_guide.zh_CN.md#按-jcs-规则解码原始文本)。
`JcsDecoder` 解析输入但不验证输入字节是否已经规范化；规范输出仍由 `JcsEncoder` 生成。

## 为什么需要这个项目

语法正确的 JSON 仍可能过大、嵌套过深，或在构造内存对象时消耗过多资源。`qubit-json` 保留
JSON 语法和 Serde 兼容性，并允许调用方限制原始及规范化输入、嵌套深度、节点数、集合大小、
键、字符串、数字、有效载荷和编码输出。跨调用累计的用量及其提交边界由 `qubit-budget` 的会话
和事务明确表达。

## 核心能力

| 领域 | 适用场景 | 明确边界 |
| --- | --- | --- |
| `decode` | 严格准入 JSON、按策略规范化文本，或用 `JcsDecoder` 解码 JCS 数字 | 普通解码默认采用 `Integer64`；JCS 解码采用 binary64 并拒绝重复键 |
| `encode` | 生成受预算约束的严格 JSON 或 JCS 规范化字节 | 整个操作成功后才提交值资源用量，但 I/O 失败仍可能在外部写入器中留下部分字节 |
| `format` | 验证 JSON 并按原 token 排版 | 只调整空白，保留数字、字符串的原始拼写和重复键，不构造值树 |
| `value` | 使用 `SafeJsonValue` 保护值树，并从 Serde 事件执行计量投影 | Serde seed 看不到数字的原始文本，也不能执行文本级数值范围检查 |
| `value::traverse` | 迭代读取或原地修改已构造的值树 | 修改按步骤生效；新增后代的访问器要自行限制工作量，访问器或输出预算失败会保留此前的改动 |

`qubit-budget` 负责限制、资源标识、预算和会话；`qubit-json` 负责 JSON 规范化、词法校验、
文本编解码、值构造和遍历。

### 核心类型与接口形状

公开 API 围绕编解码器、文档对象和值树访问器组织。下表中的 `Value` 指
`serde_json::Value`，`T` 指应用类型；箭头表示成功时的输入输出，可能失败的操作通过
`Result` 返回结果。

| 公开 API | 输入 → 输出 | 状态与选用方式 |
| --- | --- | --- |
| `decode::JsonDecoder` | `&str` / `&[u8]` → `T` 或 `SafeJsonValue`；字节输入源 → 自有结果 | 持有累计计量的解码会话和诊断策略。`decode_str` / `decode_utf8` 用于 Serde 目标类型，`decode_value_*` 生成拒绝重复键的受保护值树，`validate_*` 只做验证。 |
| `decode::NormalizingJsonDecoder` | 外部文本 → `T`，或通过 `prepare_*` → `NormalizedJsonDocument` | 按显式的 `NormalizingJsonDecodePolicy` 清理文本，再严格解码，资源用量计入累计会话。一次性解码要求目标实现 `DeserializeOwned`。 |
| `decode::NormalizedJsonDocument<'input>` | 准备好的文档 → 规范化 `&str` 或后续解码得到的 `T` | 通过 `Cow<'input, str>` 借用或持有文本，并记录原始、规范化字节数。结果需要借用规范化文本时，先准备文档，再从文档解码。 |
| `encode::JsonEncoder` | `&T: Serialize` → `Vec<u8>` 或字节输出源 | 持有累计计量的编码会话，通过 `to_vec`、`write_buffered`、`write_incremental` 输出严格 JSON。 |
| `value::JsonValueEncoder` | `&T: Serialize` → `Value`；事务投影 → `SafeJsonValue` | 无状态地将 Serde 数据投影为严格 JSON 树。可选的 `encode_in_transaction_with_private_text_limit` 分别限制私有 payload；详见[投影指南](doc/user_guide.zh_CN.md#投影值树并排版输出)。 |
| `value::SafeJsonValue` | `Value` → 受保护所有者；`as_value` → `&Value`；`into_value` → `Value` | 持有值树，复制、比较和清理等操作使用显式栈。`new` 只接管所有权，不执行预算准入。 |
| `value::traverse::JsonTreeReader` | `&Value` + `JsonTreeVisitor` → 进入、离开节点的回调 | 借用值事务，在遍历中对节点进行准入计量；只需计量整棵树时使用 `account`。 |
| `value::traverse::JsonTreeMutator` | `&mut Value` + `JsonTreeMutVisitor` → 修改后的树 | 分别借用输入、输出值事务，先检查输入，再执行修改，最后检查输出；后续失败会保留此前的修改。 |
| `decode::JcsDecoder` / `encode::JcsEncoder`（`jcs`） | `decode_value_*`：文本 → `SafeJsonValue`（兼容旧 `decode_*` → `Value`）；`&Value` → 规范化字节 | 解码采用有限 binary64 数字并拒绝重复键；编码遵循 RFC 8785，另设辅助工作容量上限。 |
| `format::JsonPrettyOptions` + `format_json_tokens` / `write_json_tokens` | JSON 字节 → 排版字节或字节输出源 | 可设置缩进宽度、显示缩进深度和末尾换行。排版保留标量 token 原文，不生成 JCS 规范形式。 |

底层值树沿用 Serde 的数据模型：

```text
Value = Null | Bool | Number | String | Array(Vec<Value>) | Object(Map<String, Value>)
```

应用可以围绕同一种值树组合这些 API：

```text
严格 JSON ── JsonDecoder::decode_value_* ──> SafeJsonValue
                                                │ as_value()
                                                v
                                           &Value ── JsonTreeReader ──> 访问器回调

&T: Serialize ── JsonValueEncoder::encode_in_transaction ──> SafeJsonValue
                                                               │
                                                               v
                                                          JsonEncoder ──> JSON 字节
                                                                              │
                                                                              v
                                                                  format_json_tokens
```

编解码器通常带有 `<'budget, R = JsonResource, Q = usize>` 参数：`'budget` 表示借用预算的
生命周期，`R` 标识资源，`Q` 表示计量数值。使用 `with_limits` 从限制创建自有会话，或使用
`new` 传入已有会话；同一会话的资源用量会跨调用累计。值事务中的计量则在调用方提交后生效。

可变访问器通过 `JsonTreeControl::Descend` 或 `SkipSubtree` 决定是否继续访问后代。
回调收到的 `JsonTreeContext` 包含从根节点算起的深度，以及当前节点在直接父容器中的位置。
编解码失败分别通过 `JsonDecodeError`、`JsonEncodeError` 等结构化类型返回，调用方可读取
`kind()` 判断稳定的错误类别。

受保护值树的组合方式见[用户手册](doc/user_guide.zh_CN.md#受保护值验证解码与排版)；
规范化文档、事务和访问器的详细用法见[进阶用法](doc/user_guide.zh_CN.md#进阶用法)。

普通解码默认使用 `JsonDecodeNumberModel::Integer64`；有限 fallback 与 JCS binary64 契约见
[JSON 数字契约](doc/number_contract.zh_CN.md)。

JCS 编码中，`Budget` 表示值或输出限制拒绝了操作；`WorkingMemory` 表示单独配置的辅助容量上限不足，
该上限通过 `JcsEncoder::with_limits(limits, max_working_bytes)` 设置。
值准入发生在渲染之前，准入遍历栈不受这项工作内存限制约束。需要限制这段遍历时，应同时配置
`max_depth` 和 `max_nodes`。工作内存限制约束的是 renderer 集合容量，不是精确的堆内存上限。

## 明确边界

- 普通严格准入会检查 JSON 语法和数字范围，但不会自动要求对象键唯一。若唯一性属于输入契约，
  可使用 `DuplicateKeyRejectingJsonValue`；JCS 原始文本路径则使用 `JcsDecoder`。
- 普通文本解码默认采用 `Integer64`：负整数装入 `i64`，非负整数装入 `u64`。显式 fallback 控制范围外整数能否准入；typed 输入路径的实际表示仍由目标类型 `T` 和启用的 `serde_json` feature 决定。启用
  `arbitrary_precision` 时，保留的宽整数仍可能无法通过严格重编码。validated 解码配合
  `JsonNumberPolicy::Preserve` 会将 fallback 整数物化为有限 `f64`，并可能舍入。`JcsDecoder` 将所有数字转换为有限 binary64，因此
  `9007199254740993` 会变成 `9007199254740992`。要求精确保留各位数字时，使用字符串或领域类型。
- `JsonValueEncoder` 只识别 `serde_json::Number` 对应的精确私有 Serde struct 协议；普通 map 即使字段同名也仍是对象。格式错误的 `RawValue` 与 `Number` 协议分别返回 `JsonSerializerStateError::InvalidRawValueProtocol` 和 `InvalidNumberProtocol`。seed 只看到原生数字事件，无法从普通 map 猜出私有协议来源。
- `JsonDecoder::decode_value_*` 返回 `SafeJsonValue`，其自有树操作和清理使用显式栈。唯一的深度限制是调用方配置的 `max_depth`（包含根值）；旧 `decode_*<T>` 仍依赖目标类型自身的构造能力，不保证任意深度都能构造或析构。`JcsDecoder` 保留历史深度错误变体，但不再另设固定的 127 层上限。其 `decode_value_str`、`decode_value_utf8` 和 `decode_value_input` 方法直接返回 `SafeJsonValue`，是深树推荐入口。只有兼容性的 `decode_str`、`decode_utf8` 和 `decode_input` 方法返回裸 `serde_json::Value`；若使用这些旧入口，处理深结果时应立即用 `SafeJsonValue::new` 接管。
- `decode_unique_validated_*` 使用 `JsonValidatedDecodeOptions`，并为每个原始数字 token 调用带路径的策略回调。额外的节点数和字符串长度限制按文档计算；`JsonDecodeSession` 的资源用量跨调用累计。若之后遇到语法错误，先前回调产生的副作用不会回滚。
- `JsonValueEncoder::encode_in_transaction` 会把测量结果暂存在调用方的值事务中，并返回受保护的 owner。可选的 `encode_in_transaction_with_private_text_limit` 分别限制每个 RawValue/Number 私有 payload 的 UTF-8 原文字节；额度不累计、不限制普通字符串、不影响旧入口，并独立于 `JsonEncoder` 的输出字节上限。遇到 `PrivateTextLimitExceeded` 后应丢弃整笔事务。详见[投影指南](doc/user_guide.zh_CN.md#投影值树并排版输出)。可以继续把 owner 交给 `JsonEncoder` 生成严格 JSON，再交给 `format_json_tokens` 排版；后者验证 JSON 语法并保留数字、字符串 token 原文及重复键，不构造 `Value`，也不执行规范化。
- `JsonSerializationErrorKind::InvalidRawValue` 表示原始值未通过严格物化契约，失败原因可能不只是语法错误。普通 newtype 仍保持透明；需要按原始 JSON 物化时应使用 `serde_json::value::RawValue`。
- 树遍历按现有 `serde_json::Number` 的紧凑显示长度计量。启用 `arbitrary_precision` 时，树读取器可计量已保存的宽数字（例如 `1e400`）；普通和 JCS 编码器仍按各自规则拒绝超出有限范围的数字。
- 所有不可信边界都应设置有限资源上限。`unlimited()` 会同时取消值、输出和工作内存限制，
  只应用于可信输入或已由其他层完成准入的数据；外层输出上限不能替代解析前的输入与结构准入。
- 严格解码和规范化解码的切片入口都会处理传入的完整 `&str` 或 `&[u8]`；输入字节限制只准入该切片，
  无法追溯限制传输层或 body 聚合器已经分配的内存。`JsonDecoder` 的切片方法可以返回借用输入的结果。
  一次性 `NormalizingJsonDecoder` 方法要求目标实现 `DeserializeOwned`，因为规范化文本可能是临时数据。
  若需在规范化后借用文本，应先准备 `NormalizedJsonDocument` 再解码，借用结果的生命周期不能超过该文档。
  `JsonDecoder::decode_input` 仍会从 `Input<Item = u8>` 读取到 EOF，并返回拥有数据的 `DeserializeOwned` 结果；
  它会暂存整份文档，未设置 `max_input_bytes` 时这部分临时内存没有配置上限。解码前仍应在传输层限制读取或
  body 聚合大小。这个严格 JSON 入口不能用于原始文本的 JCS 规范化。
- `JcsDecoder::decode_input` 也会读取并缓冲完整输入；有限输入预算下可能多读取一个不计费且无法放回输入源的探测字节。
  词法预算不是堆内存上限，结果容器容量、分配器元数据和输入缓冲区容量不受精确约束。JCS Value 构造复用共享 builder，
  只受调用方配置的、包含根值的 `max_depth` 限制，没有额外固定层数上限。历史变体 `MaterializationDepthLimit` 仍保留，
  当前配置深度超限会作为 `Decode` 中的预算错误返回。`decode_value_str`、`decode_value_utf8` 和
  `decode_value_input` 直接返回 `SafeJsonValue`，是深树推荐入口。只有兼容性的 `decode_str`、
  `decode_utf8` 和 `decode_input` 方法返回裸 `Value`；若使用这些旧入口，保留深结果前应立即用
  `SafeJsonValue::new` 接管。重复键按反转义后的字符串比较，
  不做 Unicode 规范化；该 API 不验证规范字节。
- `JcsEncoder::write_buffered` 和 `write_incremental` 在后续写入或 flush 失败时，可能在外部目标留下已经接受的字节，
  这些字节会计入输出预算。增量模式直接写入，并在成功时 flush；值预算只在完整操作成功后提交。
  `max_working_bytes` 限制遍历栈和对象排序表的辅助容量，不限制输入 `Value`、分配器元数据或输出缓冲区容量。
- 诊断信息默认脱敏。严格解码器通过 `JsonDecoder::with_diagnostic_policy` 配置，规范化
  解码器通过其规范化策略配置。脱敏错误只保留稳定分类和源坐标，不保留意外字节、token、
  键/值文本或 parser/Serde 来源。只有在输入派生来源可以安全保留和记录时，才启用
  `DiagnosticPolicy::Detailed`。

## 延伸阅读

- [中文用户手册](doc/user_guide.zh_CN.md) ·
  [English user guide](doc/user_guide.md)
- [JSON 数字契约](doc/number_contract.zh_CN.md) ·
  [JSON number contract](doc/number_contract.md)
- [设计文档](doc/json_design.zh_CN.md) ·
  [Design documents](doc/json_design.md)
- [基准测试指南](doc/benchmark_baseline.zh_CN.md) ·
  [Benchmark guide](doc/benchmark_baseline.md)
- [已发布 API 文档](https://docs.rs/qubit-json)；当前分支 API 请运行
  `cargo doc --all-features --open`

## 测试

`--all-features` 覆盖根 crate 自身声明的 feature。兼容矩阵会以根 crate 测试验证
`serde_json 1.0.135` 基线和可解析到的最新版本，各运行 `jcs`、`serde_json/arbitrary_precision`、
`serde_json/preserve_order` 的八种组合。`1.0.135` 是由 API 边界证据确定的最低兼容版本：
`serde_json::Map::into_values()` 从该版本开始提供。若现有 `Cargo.lock` 锁定了更早版本，
先运行 `cargo update -p serde_json`，再执行使用 `--locked` 的验证命令。最低版本不要求启用
`arbitrary_precision` 或 `preserve_order`。

```bash
# 使用默认 feature 运行单元测试和公开行为集成测试
cargo test --locked --lib --test tests

# 使用当前 crate 声明的全部 feature 运行测试
cargo test --locked --all-features

# 使用默认 feature 和 jcs 运行文档示例
cargo test --locked --doc
cargo test --locked --doc --features jcs

# 运行根 crate 的 serde_json 版本 × feature 矩阵（两个版本各八组）
python3 .infra/ci/check-json-feature-matrix.py --case all

# 运行本项目检查
./.infra/bin/ci-check.sh

# 检查代码覆盖率
./.infra/bin/coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./.infra/bin/align-ci.sh`格式化代码，运行`./.infra/bin/ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-json](https://github.com/qubit-ltd/rs-json)
