# JSON 数字契约

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

## 精确值和更宽的值

小于 `i64::MIN` 或大于 `u64::MAX` 的整数使用 JSON 字符串或显式领域结构。金额等精确十进制
使用十进制字符串或 coefficient/scale 对象。裸小数 JSON number 采用 binary64 语义，不是精确
十进制 wire 格式。

## Serde 边界

`qubit-json` 不启用 serde_json 的 `arbitrary_precision` feature，也不解释其旧私有 Number marker。
键为 `$serde_json::private::Number` 的对象是普通对象，并按 object/key/string 正常记账；独立的
`RawValue` 集成仍保留。

`AccountingJsonValueSeed` 看到的是已解码的 Serde 事件。它可以拒绝无法装入 value 模型的宽整数事件，
但不能检查原始 token 或执行词法预算。需要语法、范围和 `NumberBytes` 保证时，JSON 文本必须
经过 `JsonDecoder`。
