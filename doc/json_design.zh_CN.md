# `qubit-json` 0.8 设计

## 目标与边界

`qubit-json` 为 JSON 输入、文本、value 和 tree 提供资源感知基础设施。它不定义新的 JSON
数据模型，也不代替 Serde 的类型反序列化。限制、资源标识、预算和 session 均由
`qubit-budget` 所有；本 crate 仅实现 JSON 领域行为。

当前公开目录树为：

```text
src/
├── decode/             # 规范化、严格解码和错误模型
├── encode/             # 严格、有状态的文本 encoder
├── lexical/            # crate-private scanner 与共享实现
└── value/
    └── traverse/       # 非递归 reader、mutator 与 tracker
```

`lexical` 不是公共接口。领域不通过根级错误或选项模块共享公共 API。

## 两个 facade 与共享核心

公开层保留两个职责明确的 facade：`JsonDecoder` 负责不修改输入的严格 JSON 解码，
`NormalizingJsonDecoder` 负责显式允许的文本规范化。二者都委托给 crate-private、对资源类型 `R`
和数量类型 `Q` 泛化的 `JsonDecodeEngine<'budget, R, Q>`。共享核心统一负责 session、lexical
admission、数字契约、顶层类型检查、Serde 物化和 transaction 提交边界；facade 只定义输入是否
先经过规范化以及公开入口形态。这样既不会把两种信任边界混成一个带模式开关的对象，也不会
维护两套错误映射和记账实现。

## Normalizing facade

`NormalizingJsonDecoder` 持有不可变 `NormalizingJsonDecodePolicy`，只执行显式配置的规则：空白、
BOM、Markdown 围栏及字符串内控制字符处理。它不推测缺失的 JSON 标点或结构。

`NormalizingJsonDecodePolicy` 只定义文本规范化和诊断行为，不拥有预算。调用方通过
`NormalizingJsonDecoder::with_limits(policy, limits)` 显式传入 `JsonDecodeLimits`，或通过
`NormalizingJsonDecoder::new(policy, session)` 复用唯一的 `JsonDecodeSession`。
raw/normalized 输入字节以及 depth、nodes、collection、payload 等限制全部来自该 limits/session；
只有明确传入 `JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default()` 才表示无限预算。原始输入和规范化输入先累计，解码后
value 的消耗暂存；完整强类型解码成功才提交 value 消耗，失败后输入消耗仍留在 session 中。

一次性 `decode_str`/`decode_utf8` 为避免让临时规范化缓冲区逃逸，只接受
`DeserializeOwned`。需要借用反序列化、Serde seed 或重复物化时，调用方先通过
`prepare_str`/`prepare_utf8` 得到 `NormalizedJsonDocument<'input>`，再调用 document decode。
prepare 立即且只提交一次 raw/normalized 输入消耗；每次 document decode 独立暂存并在成功时
提交 value 消耗。document 不绑定创建它的 decoder。若规范化无需分配，其文本可借用原输入；
包含 JSON 转义的字符串仍需 owned 目标，因为 Serde 必须物化解转义结果。

## Strict text

严格文本接口由对象承载，避免无状态自由函数扩散：

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let mut decoder = JsonDecoder::with_limits(
    JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::new(),
);
let value: serde_json::Value = decoder.decode_utf8(br#"{"ok":true}"#)?;
# Ok::<(), qubit_json::decode::JsonDecodeError<
#     qubit_budget::json::JsonResource,
# >>(())
```

`JsonDecoder` 持有一个 `JsonDecodeSession`。`owned(limits)` 是显式 limits 的常用构造入口，
`new(session)` 用于复用调用方准备的 session；只有确实需要无限预算时才调用 `unlimited()`。
codec 不实现 `Default`，避免把无限预算伪装成安全默认值。decoder 提供 `decode_str`、`decode_utf8`、对应的 seed
入口和 `validate_str`/`validate_utf8`。每次尝试先记录输入，再经共享 scanner 准入；值
transaction 只在完整成功时提交。

`JsonEncoder` 持有一个 `JsonEncodeSession`，提供 `to_vec`、`write_buffered` 和
`write_incremental`。buffered 写入在完整字节序列可用后才写入目标；incremental 写入可在失败
时保留已接受前缀。

严格 decode 与 normalizing decode 都返回同一个泛型 `JsonDecodeError<R, Q>`。它的内部 failure
保持私有；调用方通过稳定的 `JsonDecodeErrorKind`、`JsonDecodeStage` 以及 budget、syntax、UTF-8、
top-level accessor 获取互斥的结构化信息。默认 `DiagnosticPolicy::Redacted` 不保留输入派生
source；`Detailed` 必须由调用方明确选择。严格 encode 只返回 `JsonEncodeError`：`Budget`、`InvalidRawJson`、
`Serialize` 或 `Write`。`JsonSyntaxError` 单独持有稳定的语法原因、偏移、行和列。

### 数字表示

strict 与 normalizing 文本路径共享同一数字契约：负整数装入 `i64`，非负整数装入 `u64`，
小数或指数装入有限 `f64`。lexical scanner 先完成 `NumberBytes` 预算准入，再执行范围校验；
因此资源限制与表示限制职责独立，且预算失败优先。实现不启用 serde_json arbitrary precision，
也不识别其旧 Number marker。完整规则见 [JSON 数字契约](number_contract.zh_CN.md)，决策背景见
[64 位数字收敛设计](number_contract_design.zh_CN.md)。

## Value

`JsonValueSeed` 是从 Serde 解码事件构造 `serde_json::Value` 的公开 seed。调用方将其绑定到
`JsonValueTransaction`；它适用于原始 JSON 字节不可用而仍需对物化 value 记账的场景。词法
准入与该 seed 的职责不同：前者验证 JSON 文本，后者观察已解码值。
seed 无法检查原始数字 lexeme；需要数字范围和 `NumberBytes` 保证时必须使用 `JsonDecoder`。

## Tree

`JsonTreeReader` 对不可变 `Value` 执行深度优先 enter/leave 遍历；`JsonTreeMutator` 对可变
`Value` 执行 visitor 驱动的非递归遍历。两者均在回调前对节点、容器、字符串、数字和 object
key 进行准入。

`JsonTreeReader::account` 复用 reader 的同一非递归遍历，在调用方已有 transaction 中暂存整棵树
的消耗，不调用 visitor，也不创建或提交 transaction；失败直接返回 `MeasuredBudgetError`。
`JsonTreeBudgetTracker` 在完整成功后提交该路径，为整棵 materialized tree 提供自有、可 reset
的 budget。`JsonTreeMutator` 先准入原始 tree，再执行 visitor 驱动的原地修改，最后准入完整的
修改后 tree。它返回 `JsonTreeMutateError::InputBudget`、`::Visitor` 或 `::OutputBudget`；visitor
和输出失败会保留已执行的修改。`JsonTreeControl::SkipSubtree` 只影响后代回调，最终输出记账
仍覆盖所有结果后代。`JsonTreeProcessError` 区分 reader 基础设施 budget 失败和业务 visitor
失败。

## 公开错误模型

公开错误和诊断类型按领域划分：

1. `decode::JsonDecodeError`：两个 decoder facade 共用。
2. `encode::JsonEncodeError`
3. `decode::JsonSyntaxError`
4. `value::traverse::JsonTreeProcessError`
5. `value::traverse::JsonTreeMutateError`

每种错误只暴露其领域能稳定提供的上下文；没有根级错误聚合或兼容别名。
