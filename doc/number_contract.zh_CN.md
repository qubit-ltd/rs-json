# JSON 数字契约

[English](number_contract.md)

本文规定 `qubit-json` 接受的数字表示范围。这是明确的实现契约，不是对 JSON 语法的扩展。
RFC 8259 没有规定 number token 的最大长度，但允许实现限制数字范围和精度。

## 默认严格模型

除非入口明确选择其他模型，普通严格和规范化文本解码默认采用
`Integer64`。下表范围描述的是默认模型，不代表所有解码器。

## 接受范围

| JSON token | 契约 |
| --- | --- |
| 负整数 | `i64::MIN..=-1` |
| 非负整数 | `0..=u64::MAX` |
| 负零 token `-0` | 允许准入；受控值构造保留浮点负零 |
| 小数或指数 | 必须可解析为有限 `f64` |

边界包含在内：`-9223372036854775808` 和 `18446744073709551615` 合法；两侧相邻越界值返回
`IntegerOutOfRange`。`1e400` 之类溢出 binary64 的小数或指数返回 `FloatOutOfRange`；下溢和
普通十进制舍入沿用 `f64`/serde_json 语义。

严格编码有独立的范围校验：所有 `i64`、`u64` 都合法；Serde `i128` 仅在可表示为 `i64`，
或非负且可表示为 `u64` 时合法；`u128` 必须可表示为 `u64`。越界值直接失败，不截断，也不隐式转字符串。
非有限浮点数同样拒绝。解码 fallback 不会改变此编码契约。

## 选择文本解码数字模型

普通解码默认使用 `JsonDecodeNumberModel::Integer64`。可选的
`Integer64WithFiniteFallback` 只改变超出整数范围的整数词法：若能转换为有限 `f64`，则允许并可能舍入。
普通小数和指数词法在两种模型下都必须是有限值。`JcsDecoder` 使用另一套契约：包括整数范围内的值在内，
所有数字都会转换为有限 binary64，以符合 JCS。

上文对整数表示的概括有一个负零例外：受控拥有型值构造（包括唯一键值解码，以及
validated 解码的 `Preserve` 策略）将 `-0` 表示为浮点负零。`ExactSafeInteger` 会将其
转换为整数 `0`，不保留符号。typed 解码的最终表示由目标类型和启用的 Serde feature
决定；整数范围说明不保证 `-0` 会得到整数表示。保留负零符号也不意味着保留原始 token
的拼写。

只有明确公开数字模型选择的解码入口才能启用 fallback。规范化解码和普通切片入口不会因此自动启用它。

| 入口 | 范围内整数 | 超范围整数 token | 小数/指数 token |
| --- | --- | --- | --- |
| typed `decode_input_with_number_model`，默认 `Integer64` | 按 `i64`/`u64` 准入，再由 Serde 构造 `T` | 拒绝 | 按有限 `f64` 准入，再由 Serde 构造 `T` |
| typed `decode_input_with_number_model`，有限 fallback | 按 `i64`/`u64` 准入，再由 Serde 构造 `T` | 可转为有限 `f64` 时准入；实际表示由 `T` 和启用的 feature 决定 | 按有限 `f64` 准入，再由 Serde 构造 `T` |
| validated 拥有型解码配合 `Preserve`，默认 `Integer64` | 物化为 `i64`/`u64` | 拒绝 | 物化为有限 `f64` |
| validated 拥有型解码配合 `Preserve`，有限 fallback | 物化为 `i64`/`u64` | 物化为有限 `f64`，可能舍入 | 物化为有限 `f64` |
| `JcsDecoder` | 转为 `f64` | 转为有限 `f64`，可能舍入 | 有限 `f64` |

表中 validated 结果以 `JsonNumberPolicy::Preserve` 为前提。使用
`ExactSafeInteger` 时，`1.00e2` 会物化为整数 `100`，而 `0.1` 会被拒绝；不能据此推断所有策略都会产生 `f64`。

例如 `18446744073709551617` 超出 `u64`：默认模型拒绝，fallback 则允许准入。validated 解码配合
`Preserve` 会将其物化为舍入后的有限 binary64 值 `18446744073709551616`。typed 输入路径将最终表示交给 Serde；
启用 `arbitrary_precision` 时可能保留宽整数，之后严格编码会因超出范围而拒绝。`1e400` 仍会被拒绝，因为它不是有限值。
选择模型只决定数字 token 能否准入，不保证目标类型或应用 schema 一定接受生成的值。

validated 拥有型解码可在 options 中选择模型：

```rust
use qubit_json::decode::{
    JsonDecodeNumberModel, JsonDecoder, JsonNumberPolicy,
    JsonValidatedDecodeOptions,
};
use serde_json::Value;

let options = JsonValidatedDecodeOptions::builder()
    .number_model(JsonDecodeNumberModel::Integer64WithFiniteFallback)
    .build();
let mut decoder = JsonDecoder::unlimited();
let value = decoder
    .decode_unique_validated_str("18446744073709551617", &options,
        |_, _| Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve))
    .unwrap();
assert!(value.as_value().is_f64());
let expected = Value::Number(serde_json::Number::from_f64(18446744073709551616_f64).unwrap());
assert_eq!(value.as_value().to_string(), expected.to_string());
```

输入流 typed 入口也支持显式选择模型：

```rust
use qubit_json::decode::{JsonDecodeNumberModel, JsonDecoder};
use serde_json::Value;
use std::io::Cursor;

let mut decoder = JsonDecoder::unlimited();
let mut input = Cursor::new(b"18446744073709551617".to_vec());
let value: Value = decoder
    .decode_input_with_number_model(
        &mut input,
        JsonDecodeNumberModel::Integer64WithFiniteFallback,
    )
    .unwrap();
let expected: Value = serde_json::from_str("18446744073709551617").unwrap();
assert_eq!(value.to_string(), expected.to_string());
assert_eq!(value.is_f64(), expected.is_f64());

let mut input = Cursor::new(b"1e400".to_vec());
assert!(decoder
    .decode_input_with_number_model::<Value, _>(
        &mut input,
        JsonDecodeNumberModel::Integer64WithFiniteFallback,
    )
    .is_err());
```

## 资源预算与表示范围相互独立

`NumberBytes` 限制原始 number token 的词法字节数，不定义数字精度或数值范围。文本解码先执行
预算准入，再校验范围；若同一个 token 同时违反两者，优先返回预算错误。

## JavaScript 客户端

本契约有意接受超过 `Number.MAX_SAFE_INTEGER` 的整数，因为既有 Java 服务使用 64 位 `long`
标识符。浏览器必须使用能保留整数文本并把不安全整数转换为 `BigInt`（或字符串）的 JSON
parser。`n` 后缀仅属于 JavaScript 源码；`123n` 不是合法 JSON，绝不能出现在 wire 中。

## JCS 原始文本解码

启用 `jcs` feature 后，`JcsDecoder` 使用独立于普通路径的 binary64 数字契约。所有 JSON 数字
（包括整数词法）都转为 `f64`，再构造 `serde_json::Number`。因此 `9007199254740993` 舍入为
`9007199254740992`；有限值下溢为零是允许的，负零在解码后的值中保留符号位；会溢出为非有限值的
数字（如 `1e400`）被拒绝。整数的精确十进制表示不能从结果中恢复。

使用 JSON 字符串承载必须精确保留的 ID、金额或其他十进制值。普通 `JsonDecoder`、normalizing
解码和 `RawValue` 协议采用普通严格契约；只有显式调用 `JcsDecoder` 才使用 JCS binary64 模型。
JCS 解码的 `NumberBytes` 仍按原始 token 字节数计量，并先于范围检查；预算限制计量资源，不是精确堆内存上限。

JCS 与唯一键 `Value` 构造复用共享 builder，没有独立固定容器上限；root-inclusive 配置项 `max_depth`
控制深度准入。历史 `DepthLimit` 与 `JcsDecodeError::MaterializationDepthLimit` 原因/变体为源码兼容而保留；
当前共享 builder 将配置深度拒绝报告为预算错误。`JsonValueEncoder` 投影真正的
`serde_json::value::RawValue` 时也遵循同一配置深度契约。
`JsonValueEncoder` 返回 `InvalidRawValue` 表示严格物化契约拒绝了输入，原因可能是重复键、数字约束或
深度超限，也可能是语法错误。

## 精确值和更宽的值

小于 `i64::MIN` 或大于 `u64::MAX` 的整数使用 JSON 字符串或显式领域结构。金额等精确十进制
使用十进制字符串或 coefficient/scale 对象。裸小数 JSON number 采用 binary64 语义，不是精确
十进制 wire 格式。

## 已物化树与 Serde seed

数字规则取决于入口：

- `JsonDecoder::decode_unique_str`、`decode_unique_utf8` 和 `decode_unique_seed_utf8` 先准入源文档，按严格整数/有限 f64 契约构造无重复键的树，再将规范构造的 `Number` 作为原生数字事件交给 Serde。即使启用 `arbitrary_precision`，也无需猜测对象键的含义即可保留数字类型。结果拥有数据所有权，由包含根值的配置 `max_depth` 控制构造，没有独立固定容器上限；仅在目标反序列化成功后提交源值预算。中间树会增加内存开销，目标反序列化错误没有原始源码坐标。目标自身的 Serde 语义仍然适用；需要按字面保留私有同名字段对象时，应选择 `DuplicateKeyRejectingJsonValue`。
- 文本解码、规范化解码、普通序列化及 Serde 原生整数事件采用上文普通契约。`i128`/`u128` 只有能表示为相应 `i64` 或 `u64` 时才接受。
- `AccountingJsonValueSeed` 与 `DuplicateKeyRejectingJsonValueSeed` 接收的是已解码事件，而不是源文本，因此都看不到原始 number lexeme。`AccountingJsonValueSeed` 会检查原生整数事件是否在 64 位范围内，并按构造后 `Number` 的紧凑显示长度执行 `NumberBytes` 预算；该长度可能与源 token 长度不同。`DuplicateKeyRejectingJsonValueSeed` 只检查重复键，本身不执行资源预算。如果 `arbitrary_precision` 将数字呈现为 map 事件，任一 seed 都无法区分它与同形状的真实对象，因此按对象保留。需要可靠文本来源时，使用受控文本、RawValue 或 JCS 入口。
- `JsonTreeReader` 读取的是已存在的 `serde_json::Value`，按 Number 当前紧凑显示表示的字节长度计量；不转换为 `f64`，也不套用普通文本 encoder 的范围限制。启用 `arbitrary_precision` 后，已有的 `1e400` 可安全计量，39 位整数则按 39 字节计费。普通 `JsonEncoder` 与 `JcsEncoder` 仍会拒绝超出各自契约的数字。

crate 自身不启用 serde_json 的 `arbitrary_precision`。若其他依赖统一启用了该 feature，encoder 会识别 serde_json 精确的私有 Number struct 协议：只有一个字符串 payload，经 `serialize_str` 或一次 `collect_str` 提交。它会用共享 JSON 数字 scanner 校验后再输出或构造 Value。普通对象即使有 `$serde_json::private::Number` 键也仍按对象处理。单独的 `RawValue` 私有协议同样受支持并会验证。私有协议形状错误返回 `JsonSerializationErrorKind::InvalidSerializerState`，具体状态原因是 `JsonSerializerStateError::InvalidNumberProtocol` 或 `InvalidRawValueProtocol`；诊断文本不泄露 payload。

JCS 保持独立：原始文本对所有数字使用有限 binary64，溢出值（例如 `1e400`）会被拒绝；在 `arbitrary_precision` 下已有宽 Number 不会让 `JcsEncoder` 接受它。
