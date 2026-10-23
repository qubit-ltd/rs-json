# 迁移到 `qubit-json` 0.12

[English](migration_0_12.md)

本文说明从 `qubit-json` 0.11 升级到 0.12 时的源码与行为变化。候选版本要求 Rust 1.94，并与 `qubit-budget` 0.9.1 配套使用。

## f32 值与对象键表示

本 checkout 修复了 f32 物化及对象键一致性：使用 serde_json 的 f32 JSON 表示。因此，`1.0_f32` 对象键会从 `"1"` 变为 `"1.0"`，与直接文本编码一致。依赖旧键名的查询需要更新；确需整数键时，应显式序列化为整数或字符串。启用 `arbitrary_precision` 时，有限 f32 也可以物化并通过严格编码。本 checkout 的修改不表示包含该修改的版本已发布。无需下游源码迁移或版本调整。

## 同步更新依赖版本

依赖图中的 JSON limits、resource、session 和 transaction 类型应统一来自同一版 `qubit-budget`：

```toml
[dependencies]
qubit-json = "0.12"
qubit-budget = { version = "0.9.1", features = ["json"] }
serde_json = "1.0"
```

`qubit-budget` 0.8 和 0.9.1 的公开类型身份不同。一版的 `JsonDecodeLimits`、`JsonEncodeLimits`、resource 或 transaction 不能传给另一版 API。下游应删除 JSON 数据交换路径上的旧 budget 直接依赖，更新 lockfile，并编译真实的 JSON 类型传递路径。不要添加类型转换包装层，也不要为了连接两种类型而同时保留两个 budget 版本。

七个直接交换这些 JSON/budget 类型的下游候选版本如下：

| Crate | 当前版本 | 候选版本 |
| --- | ---: | ---: |
| `qubit-datatype` | 0.14.0 | 0.15.0 |
| `qubit-redact` | 0.9.0 | 0.10.0 |
| `qubit-value` | 0.12.2 | 0.13.0 |
| `qubit-config` | 0.14.3 | 0.15.0 |
| `qubit-metadata` | 0.6.2 | 0.7.0 |
| `qubit-http` | 0.15.1 | 0.16.0 |
| `qubit-web` | 0.2.0 | 0.3.0 |

这些 crate 的 JSON 路径统一更新为 `qubit-json = "0.12"`、`qubit-budget = "0.9.1"`，随后同步调整各自版本与 lockfile。表中是候选版本，不表示已经发布。未交换 JSON budget 类型的其他 crate 不在此表内，不应仅为这次迁移一并升级。

rs-json 候选还会带入提交 `ec16e78` 中已有的 Cargo 修改；整合该修改后，manifest 使用 `qubit-io 0.19`。这是延续已有 Cargo 决定，不是 0.12 计划新增的 IO 迁移。

## 更新穷举 serializer 错误匹配

`JsonSerializerStateError` 新增 `InvalidNumberProtocol`。对该枚举做穷举 `match` 的代码需要增加这一分支。手写 Number 协议形状错误时，现在会以稳定原因 `InvalidNumberProtocol` 返回 `JsonSerializationErrorKind::InvalidSerializerState`。格式错误的 RawValue 协议仍返回 `InvalidRawValueProtocol`。

Number 协议是 serde_json 的私有互操作协议：只有精确的私有 struct 名称携带一个字符串 payload 时才会识别；payload 通过 `serialize_str` 或一次 `collect_str` 提交。零字段、多字段、跳过字段、非字符串或重复 payload 都会被拒绝。即使 serializer 捕获并忽略 payload 的错误，encoder 仍会保留失败状态，整个操作不能返回成功。错误诊断不会暴露 payload 文本。

`JsonValueEncoder` 处理真实 `serde_json::Number` 时会构造 Number 值，不再留下 `$serde_json::private::Number` marker 对象。用户普通 map 中同名键仍是普通对象；仅凭键名不会启用私有协议。公开错误结构不提供 deprecated alias 或兼容 facade。

`JsonValueEncoder` 对普通 newtype struct 一律透明处理，包括名称为
`$serde_json::private::RawValue` 的自定义 newtype：字符串、布尔值或 `Option` payload 都按其自身
类型序列化，不会再次当作 JSON 解析。确实需要把原始 JSON 文本物化为 Value 时，应使用真正的
`serde_json::value::RawValue`；其精确私有 struct 协议仍受支持。若手写的零长度 compound serializer
继续发送字段或元素，现在会失败，不再可能以无效 JSON 返回成功；合法空容器不受影响。

## 分清三种数字边界

- **普通严格文本、序列化与原生 Serde 整数事件：** 负整数须装入 `i64`，非负整数须装入 `u64`，小数或指数须转换为有限 `f64`。`i128`、`u128` 事件会显式检查范围。文本解码先准入原始 `NumberBytes` 测量，再检查数值范围。Serde seed 只能看到解码后的事件，无法检查原始 token，也无法执行 `NumberBytes` 限制。如果上游 `arbitrary_precision` 把数字呈现为 map 事件，seed 无法区分真实对象，会按对象保留。
- **已经物化的 `serde_json::Value` 树：** `JsonTreeReader` 按已有 Number 当前紧凑显示表示的字节长度计量。启用 `arbitrary_precision` 后，它可安全核算已有的 `1e400`，并把 39 位整数按 39 字节计量；不会先转成 `f64`，也不会套用普通文本编码的范围检查。这不代表普通或 JCS encoder 会接受该数字。
- **JCS 文本：** `JcsDecoder` 把每个数字（包括整数词法）转换为有限 IEEE-754 binary64。因此 `9007199254740993` 会舍入为 `9007199254740992`，`1e400` 这类溢出值会被拒绝。对象键在反转义后查重，JCS Value 构造仍有 127 层容器上限。

## Display 输出增长前进行检查

`qubit-budget` 0.9.1 新增 `JsonValueTransaction::check_admit`。它会检查 quantity 转换、单点限制，以及当前暂存用量之上的累计节点数和 payload；成功时不应用该测量。检查失败会记录首个 `MeasuredBudgetError` 并毒化 transaction，之后 `commit` 不能发布暂存的 value 用量。

当 `Display` 分块生成 JSON 字符串或对象键时，encoder 会在追加每一块之前检查追加后的完整预计字符串/键。预检成功不扣预算；格式化成功后才对完整字符串/键调用一次 `try_admit`，节点和 payload 因而只计费一次。如果格式化阶段发生错误，而此前已经有预检错误，返回预算/数量错误。

私有 RawValue/Number 原始输出 payload 必须等完整文本收集后才能校验 JSON 结构；收集期间受 output 字节限制约束。`check_admit` 和 collector 都无法控制用户 `Display::fmt` 内部的任意分配、分配器元数据或进程 RSS；保证针对的是库自身的逻辑准入和追加边界。

## 保持不变的操作边界

- 累计 session 继续保留已接受的输入计费。有限流式输入上限可能多读一个用于确认超限的 probe byte；它会被观察到，但不计费。
- value 测量仍通过 transaction 暂存，完整操作成功后才发布。外部 writer 已接受的字节无法回滚；后续写入或 flush 失败时，这些字节仍计入预算。
- RawValue 文本必须先验证，再写入外部 writer。增量输出后续失败时，外部目标可能保留此前已接受的合法前缀。
- JCS Value 构造固定的 127 层容器限制仍独立于可配置、包含根节点的深度预算。
- `JsonValueEncoder` 投影真正 `RawValue` 时也适用相同的固定 127 层容器限制；它是物化保护，不是普通编解码入口的通用深度限制。`InvalidRawValue` 表示严格 RawValue 物化契约被拒绝，不专指语法错误。
- 默认诊断继续脱敏，不保留输入文本、私有 payload 或任意 `Serialize::custom` 文本。

私有源码文件如何组织属于实现细节；迁移不承诺保留旧私有模块路径、deprecated alias 或跨 budget 类型身份转换。

## 相关文档

- [中文用户手册](user_guide.zh_CN.md) · [English user guide](user_guide.md)
- [JSON 数字契约](number_contract.zh_CN.md) · [Number contract](number_contract.md)
- [设计文档](json_design.zh_CN.md) · [Design](json_design.md)
