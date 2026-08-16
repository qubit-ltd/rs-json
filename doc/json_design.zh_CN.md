# `qubit-json` 0.8 设计

## 目标与边界

`qubit-json` 为 JSON 输入、文本、value 和 tree 提供资源感知基础设施。它不定义新的 JSON
数据模型，也不代替 Serde 的类型反序列化。限制、资源标识、预算和 session 均由
`qubit-budget` 所有；本 crate 仅实现 JSON 领域行为。

公开目录树固定为：

```text
src/
├── internal/   # crate-private scanner 与共享实现
├── lenient/    # 可配置的文本规范化和宽松解码
├── text/       # 严格、有状态的文本 decoder/encoder
├── value/      # 带 transaction 的 Value seed
└── tree/       # 非递归 reader、mutator 与 tracker
```

`internal` 不是公共接口。四个领域不通过根级错误或选项模块共享公共 API。

## Lenient

`LenientJsonDecoder` 持有不可变 `LenientJsonDecodeOptions`，只执行显式配置的规则：空白、
BOM、Markdown 围栏及字符串内控制字符处理。它不推测缺失的 JSON 标点或结构。

普通 `decode` 等便捷入口默认只执行规范化，并仅强制 raw/normalized 输入字节限制；
它们不会进行词法 value 准入。若要在这些入口上限制 depth、nodes 或 payload，请在
`LenientJsonDecodeOptions` 中配置 `value_limits`。`decode_with_session` 先累计原始输入与规范化输入，
再进行词法准入并将解码后 value 的消耗暂存；完整强类型解码成功才提交 value 消耗。失败后
输入消耗仍留在 session 中。

`LenientJsonDecodeError` 是该领域唯一公开的解码错误。其 kind、stage、top-level 及安全位置
元数据表达空输入、UTF-8、大小限制、规范化、语法、准入或类型反序列化失败。默认
`ErrorPrivacyPolicy::Redacted` 不暴露输入派生诊断；`Detailed` 必须由调用方明确选择。

## Strict text

严格文本接口由对象承载，避免无状态自由函数扩散：

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonDecodeSession};
use qubit_json::text::JsonTextDecoder;

let mut session = JsonDecodeSession::owned(JsonDecodeLimits::<JsonResource, usize>::new());
let value: serde_json::Value = JsonTextDecoder::new(&mut session)
    .decode(br#"{"ok":true}"#)?;
# Ok::<(), qubit_json::text::JsonDecodeError<
#     qubit_budget::json::JsonResource,
# >>(())
```

`JsonTextDecoder` 借用一个 `JsonDecodeSession`，提供 `decode`、`decode_seed` 和 `validate`。
每次尝试先记录输入，再经共享 scanner 准入；值 transaction 只在完整成功时提交。

`JsonTextEncoder` 借用一个 `JsonEncodeSession`，提供 `to_vec`、`write_buffered` 和
`write_incremental`。buffered 写入在完整字节序列可用后才写入目标；incremental 写入可在失败
时保留已接受前缀。

严格 decode 只返回 `JsonDecodeError`：`Budget`、`Syntax` 或带 category、line、column 的
`Deserialize`。严格 encode 只返回 `JsonEncodeError`：`Budget`、`InvalidRawJson`、
`Serialize` 或 `Write`。`JsonSyntaxError` 单独持有稳定的语法原因、偏移、行和列。

## Value

`JsonValueSeed` 是从 Serde 解码事件构造 `serde_json::Value` 的公开 seed。调用方将其绑定到
`JsonValueTransaction`；它适用于原始 JSON 字节不可用而仍需对物化 value 记账的场景。词法
准入与该 seed 的职责不同：前者验证 JSON 文本，后者观察已解码值。

## Tree

`JsonTreeReader` 对不可变 `Value` 执行深度优先 enter/leave 遍历；`JsonTreeMutator` 对可变
`Value` 执行 visitor 驱动的非递归遍历。两者均在回调前对节点、容器、字符串、数字和 object
key 进行准入。

`JsonTreeBudgetTracker` 为整棵 materialized tree 提供自有、可 reset 的 budget。`JsonTreeMutator`
在预算拒绝时由 visitor 返回 `JsonTreeBudgetRejection` 选择终止或跳过子树。可变处理是增量的：
已接受的预算和已执行的变更不回滚。`JsonTreeProcessError` 区分基础设施 budget 失败和业务
visitor 失败。

## 公开错误模型

公开错误恰好按领域划分为五种：

1. `lenient::LenientJsonDecodeError`
2. `text::JsonDecodeError`
3. `text::JsonEncodeError`
4. `text::JsonSyntaxError`
5. `tree::JsonTreeProcessError`

每种错误只暴露其领域能稳定提供的上下文；没有根级错误聚合或兼容别名。
