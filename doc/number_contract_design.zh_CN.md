# `qubit-json` 64 位数字收敛设计

## 背景

历史实现启用 serde_json `arbitrary_precision`，并识别其未公开的 Number Serde marker。这使合法
对象 `{"$serde_json::private::Number":"123"}` 可能改变为 number，造成文本结构、最终 value 和
预算类别不一致；同一协议还会使不认识 marker 的 seed 把真实大数构造成 object。

## 决策

`qubit-json` 的根本职责是资源感知的 JSON 边界，而不是任意精度计算库或自有 JSON parser。
因此保留 lexical admission、Serde typed decode、budget/session/transaction 和
`serde_json::Value`，删除全部 arbitrary-precision Number 私有协议。

公开表示固定为：负整数 `i64`、非负整数 `u64`、小数/指数有限 `f64`。该范围支持 Java `long`
以及现有前端自定义 parser/BigInt，但不承诺 JavaScript `Number` 安全整数。更宽整数和精确十进制
由字符串或明确的领域 wire 表示。

## 数据流与错误

```text
input bytes
  -> lexical syntax and budget admission
  -> integer/floating-point range validation
  -> serde_json deserializer without arbitrary_precision
  -> target type
  -> commit staged value budget after complete success
```

`NumberBytes` 准入先于范围校验，以维持资源失败优先。整数越界返回
`IntegerOutOfRange`，浮点溢出返回 `FloatOutOfRange`；错误仅携带安全位置，不复制完整 token。
编码端对 `i128/u128` 做对称转换检查，失败时返回序列化错误，不截断或隐式转字符串。

## 私有协议边界

生产代码不识别 `$serde_json::private::Number`，该键永远是普通 object key。RawValue 是独立、
仍有公开用途的 serde_json 集成，继续保留并单独测试。有限浮点的紧凑 JSON lexeme 长度通过
serde_json 的公开 `CompactFormatter` 接口计算，不直接耦合其底层浮点格式化依赖。

## Seed 与物化 value

`JsonValueSeed` 只能观察已解码 Serde 事件。`i128/u128` 超出 i64/u64 联合范围时返回可恢复错误，
但 seed 无法核验原始 number token、词法预算或文本范围；这些保证必须由 `JsonDecoder` 提供。

## 验收不变量

- marker 同名对象保持 object，并按 object/key/string 记账。
- 大于 `u64::MAX` 或小于 `i64::MIN` 的整数不会进入 serde_json typed decode。
- 整个发布依赖图不启用 `serde_json/arbitrary_precision`。
- 生产代码没有 Number marker 识别、生成或 reserved-key 逻辑。
- 双语 README、用户手册、数字契约、Rustdoc、设计和依赖维护文档语义一致。
- 不引入自有 parser、新 JsonValue 模型或与本问题无关的 budget/tree 重构。
