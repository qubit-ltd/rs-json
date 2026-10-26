# JSON 数字契约

[English](number_contract.md)

本文规定 `qubit-json` 接受的数字表示范围。这是明确的实现契约，不是对 JSON 语法的扩展。
RFC 8259 没有规定 number token 的最大长度，但允许实现限制数字范围和精度。

## 接受范围

| JSON token | 契约 |
| --- | --- |
| 负整数 | `i64::MIN..=-1` |
| 非负整数 | `0..=u64::MAX` |
| 小数或指数 | 必须可解析为有限 `f64` |

边界包含在内：`-9223372036854775808` 和 `18446744073709551615` 合法；两侧相邻越界值返回
`IntegerOutOfRange`。`1e400` 之类溢出 binary64 的小数或指数返回 `FloatOutOfRange`；下溢和
普通十进制舍入沿用 `f64`/serde_json 语义。

编码规则对称：所有 `i64`、`u64` 都合法；Serde `i128` 仅在可表示为 `i64`，或非负且可表示
为 `u64` 时合法；`u128` 必须可表示为 `u64`。越界值直接失败，不截断，也不隐式转字符串。
非有限浮点数同样拒绝。

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

JCS `Value` 构造最多接受 127 层嵌套容器。`JsonValueEncoder` 投影真正的
`serde_json::value::RawValue` 时也受同一物化上限约束；普通 JSON 编解码入口没有这项通用深度限制。
`JsonValueEncoder` 返回 `InvalidRawValue` 表示严格物化契约拒绝了输入，原因可能是重复键、数字约束或
深度超限，也可能是语法错误。

## 精确值和更宽的值

小于 `i64::MIN` 或大于 `u64::MAX` 的整数使用 JSON 字符串或显式领域结构。金额等精确十进制
使用十进制字符串或 coefficient/scale 对象。裸小数 JSON number 采用 binary64 语义，不是精确
十进制 wire 格式。

## 已物化树与 Serde seed

数字规则取决于入口：

- `JsonDecoder::decode_unique_str`、`decode_unique_utf8` 和 `decode_unique_seed_utf8` 先准入源文档，按严格整数/有限 f64 契约构造无重复键的树，再将规范构造的 `Number` 作为原生数字事件交给 Serde。即使启用 `arbitrary_precision`，也无需猜测对象键的含义即可保留数字类型。结果拥有数据所有权，物化最多接受 127 层容器；仅在目标反序列化成功后提交源值预算。中间树会增加内存开销，目标反序列化错误没有原始源码坐标。目标自身的 Serde 语义仍然适用；需要按字面保留私有同名字段对象时，应选择 `DuplicateKeyRejectingJsonValue`。
- 文本解码、规范化解码、普通序列化及 Serde 原生整数事件采用上文普通契约。`i128`/`u128` 只有能表示为相应 `i64` 或 `u64` 时才接受。
- `AccountingJsonValueSeed` 与 `DuplicateKeyRejectingJsonValueSeed` 接收的是已解码事件，而不是源文本，因此都看不到原始 number lexeme。`AccountingJsonValueSeed` 会检查原生整数事件是否在 64 位范围内，并按构造后 `Number` 的紧凑显示长度执行 `NumberBytes` 预算；该长度可能与源 token 长度不同。`DuplicateKeyRejectingJsonValueSeed` 只检查重复键，本身不执行资源预算。如果 `arbitrary_precision` 将数字呈现为 map 事件，任一 seed 都无法区分它与同形状的真实对象，因此按对象保留。需要可靠文本来源时，使用受控文本、RawValue 或 JCS 入口。
- `JsonTreeReader` 读取的是已存在的 `serde_json::Value`，按 Number 当前紧凑显示表示的字节长度计量；不转换为 `f64`，也不套用普通文本 encoder 的范围限制。启用 `arbitrary_precision` 后，已有的 `1e400` 可安全计量，39 位整数则按 39 字节计费。普通 `JsonEncoder` 与 `JcsEncoder` 仍会拒绝超出各自契约的数字。

crate 自身不启用 serde_json 的 `arbitrary_precision`。若其他依赖统一启用了该 feature，encoder 会识别 serde_json 精确的私有 Number struct 协议：只有一个字符串 payload，经 `serialize_str` 或一次 `collect_str` 提交。它会用共享 JSON 数字 scanner 校验后再输出或构造 Value。普通对象即使有 `$serde_json::private::Number` 键也仍按对象处理。单独的 `RawValue` 私有协议同样受支持并会验证。私有协议形状错误返回 `JsonSerializationErrorKind::InvalidSerializerState`，具体状态原因是 `JsonSerializerStateError::InvalidNumberProtocol` 或 `InvalidRawValueProtocol`；诊断文本不泄露 payload。

JCS 保持独立：原始文本对所有数字使用有限 binary64，溢出值（例如 `1e400`）会被拒绝；在 `arbitrary_precision` 下已有宽 Number 不会让 `JcsEncoder` 接受它。
