# `qubit-json` 设计

[English](json_design.md)

## 目标与边界

`qubit-json` 为 JSON 输入、文本、value 和 tree 提供资源感知基础设施。它不定义新的 JSON
数据模型，也不代替 Serde 的类型反序列化。限制、资源标识、预算和 session 均由
`qubit-budget` 所有；本 crate 仅实现 JSON 领域行为。

当前公开目录树为：

```text
src/
├── decode/             # 规范化、严格解码和错误模型
├── encode/             # 严格、有状态的文本 encoder
├── format/             # 校验语法并保留 token 的排版
├── lexical/            # crate-private scanner 与共享实现
└── value/
    └── traverse/       # 非递归 reader、mutator 与 tracker
```

`lexical` 不是公共接口。领域不通过根级错误或选项模块共享公共 API。

## 解码 facade 与共享核心

公开层提供三个职责明确的 facade：`JsonDecoder` 负责不修改输入的严格 JSON 解码，
`NormalizingJsonDecoder` 负责显式允许的文本规范化，feature-gated `JcsDecoder` 则按 binary64
数字规则解码原始文本并拒绝重复键。它们都委托给 crate-private、对资源类型 `R` 和数量类型 `Q`
泛化的 `JsonDecodeEngine<'budget, R, Q>`。共享核心统一负责 session、lexical admission、数字契约、
顶层类型检查、Serde 物化和 transaction 提交边界；各 facade 选择适用的输入规则与公开入口形态。

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

两个 decoder 都接收完整的 `&str` 或 `&[u8]`。输入字节限制不能追溯约束外层 transport
或 body aggregator 已经分配的内存；有界读取或聚合上限必须设置在该外层边界。

`JsonDecoder::decode_input` 则从 `Input<Item = u8>` 读取到 EOF，并返回拥有所有权的结果。输入预算
有限时，每次读取最多包含剩余额度加一个探针字节。探针用于区分恰好到达限额时的 EOF 与后续仍有
数据的情况；它不会计费或进入缓冲区，且 `Input` 无法将它放回输入源。即使探针导致预算错误，
额度内已读前缀仍会计费并保留。

## Strict text

严格文本接口由对象承载，避免无状态自由函数扩散：

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let mut decoder = JsonDecoder::with_limits(
    JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::new(),
);
let value: serde_json::Value = decoder.decode_utf8(br#"{"ok":true}"#)?;
Ok(())
}
```

`JsonDecoder` 持有一个 `JsonDecodeSession`。`with_limits(limits)` 是显式 limits 的常用构造入口，
`new(session)` 用于复用调用方准备的 session；只有确实需要无限预算时才调用 `unlimited()`。
codec 不实现 `Default`，避免把无限预算伪装成安全默认值。decoder 提供 `decode_str`、`decode_utf8`、对应的 seed
入口和 `validate_str`/`validate_utf8`。每次尝试先记录输入，再经共享 scanner 准入；值
transaction 只在完整成功时提交。

### 唯一键拥有型解码

`JsonDecoder::decode_unique_str`、`decode_unique_utf8` 与
`decode_unique_seed_utf8` 要求结果拥有所有权（分别使用 `DeserializeOwned`，或输出类型不依赖输入生命周期的
seed）。该路径先执行普通严格词法准入，并暂存源文档的 value 计量；随后再次扫描字节，构造中间
`serde_json::Value`。第二遍同步发出词法事件，但关闭重复计量，因此每项源资源只计费一次。目标 seed
接收来自该拥有型中间树的 Serde 事件，不会借用原始文本。

中间树使用严格的 `Integer64` 数字模型：负整数构造为 `i64`，非负整数为 `u64`，小数或指数数字为有限
`f64`。从该树反序列化会发出原生整数和浮点 Serde 事件，即使启用了 `arbitrary_precision` 也是如此。
整数 token 表示有一个例外：`-0` 构造为浮点负零，并以原生浮点事件交给目标；这不保证保留原始 token 拼写。
对象键按 JSON 反转义后的字符串查重，不做 Unicode 规范化。共享 builder 没有独立固定容器上限；包含根值的
配置项 `max_depth` 控制深度准入。历史 `DepthLimit { limit }` 原因因源码兼容而保留；当前配置深度超限会作为预算错误返回。
临时树与最终目标可能同时驻留内存，词法资源预算不是精确的堆内存上限。对象按
`serde_json::Map` 的迭代顺序处理。

`RawValue` 目标接收从中间树重新序列化的 JSON，而非原文切片。`serde_json::Value` 仍会按自身语义解释私有键
`$serde_json::private::RawValue`；若要求私有标记键保留为普通数据，应使用
`DuplicateKeyRejectingJsonValue`。重复键物化失败会作为反序列化错误返回，并附带第二个重复键起始引号处的一基源坐标；
配置深度超限则作为准入预算错误返回。目标 seed 自身的失败没有原文坐标。只有目标构造成功后才提交 value 计量；失败时
已接受的输入计量仍保留。详见[重复对象键指南](user_guide.zh_CN.md#重复对象键)。

`JsonDecodeError::materialization_error_reason()` 为唯一键构造失败提供可复制的旁路原因：
`DuplicateKey` 或防御性的 `InvalidNumber`。历史 `DepthLimit { limit }` 原因因源码兼容而保留，
但共享 builder 当前不会产生它。其他解码失败返回 `None`。
原因不受 `DiagnosticPolicy` 影响、不含输入文本，并会由 `Clone` 保留。调用 `into_source()` 前应先读取原因；
`JsonDecodeErrorSource` 及其公开变体保持不变，因此现有粗粒度错误映射无需迁移。构造错误坐标仍按
UTF-8 字节计列，列号从 1 开始，只有 LF 会开始新行。

`JsonEncoder` 持有一个 `JsonEncodeSession`，提供 `to_vec`、`write_buffered` 和
`write_incremental`。buffered 写入在完整字节序列可用后才写入目标；incremental 写入可在失败
时保留已接受前缀。

严格 decode 与 normalizing decode 都返回同一个泛型 `JsonDecodeError<R, Q>`。调用方通过稳定的
`JsonDecodeErrorKind`、`JsonDecodeStage` 以及 budget、syntax、UTF-8、top-level accessor 获取
互斥的结构化信息；取得错误所有权后，也可以调用 `into_source()` 穷举匹配
`JsonDecodeErrorSource`，无需克隆结构化数据。默认 `DiagnosticPolicy::Redacted` 只保留稳定
错误分类和源坐标，不保留意外字节或其他输入内容；严格 decoder 通过
`with_diagnostic_policy` 显式选择 `Detailed`，normalizing decoder 则从
`NormalizingJsonDecodePolicy` 读取该设置。严格 encode 只返回 `JsonEncodeError`：`Budget`、`InvalidRawJson`、
`Serialize` 或 `Write`。`JsonSyntaxError` 单独持有稳定的语法原因、偏移、行和列。

`JcsDecoder` 返回 `JcsDecodeError<R, Q>`，其中 `Decode` 包装既有解码错误，
`DuplicateKey { line, column }` 与 `MaterializationDepthLimit { limit, line, column }` 是结构化物化错误，
且不保存键名或 backend source。完整词法准入后，scanner 会再次运行并同步向迭代式 Value builder 发送事件；
builder 在插入值之前检查重复键。共享 builder 没有独立固定容器上限；包含根值的配置项 `max_depth` 控制准入，
超限作为预算错误返回。历史 `MaterializationDepthLimit` 变体因源码兼容而保留，但当前 builder 不会产生该变体。公开的 `decode_value_str`、`decode_value_utf8` 和
`decode_value_input` 直接返回 `SafeJsonValue`，适合深树所有权；兼容保留的旧
`decode_str`、`decode_utf8` 和 `decode_input` 返回裸 `serde_json::Value`。将受保护值交给
`JcsEncoder::to_vec` 时传入 `owner.as_value()`。
其他语法和预算失败仍保留普通解码错误分类。完整文档成功后才提交值资源；失败时已接受的输入字节仍计入 session。

### 数字表示

strict 与 normalizing 文本解码默认采用 `Integer64`：负整数装入 `i64`，非负整数装入 `u64`，
小数或指数装入有限 `f64`。可选的 `Integer64WithFiniteFallback` 允许准入可转换为有限 `f64` 的范围外整数 token；它不会放宽小数或指数 token 的有限值要求。validated `Preserve` 会将 fallback 整数物化为有限 `f64`，可能舍入。typed 输入路径的表示由目标类型 `T` 和启用的 `serde_json` feature 决定；启用 `arbitrary_precision` 时，保留的宽整数可能无法严格重编码。只有明确公开模型选择的解码入口才能启用
fallback；规范化与普通切片入口不会自动启用。严格编码有独立范围校验，不继承解码 fallback。feature-gated 的 `JcsDecoder` 单独采用
binary64 模型，将所有数字（包括整数范围内的值）转为 `f64`，并在物化时拒绝对象内重复键。普通两种模型都先准入
原始 `NumberBytes`，再校验范围，因此预算错误优先。crate 自身不启用 serde_json `arbitrary_precision`；但若其他依赖启用该 feature，仍会处理精确的私有 `Number` struct 协议。普通 map 即使有同名键也仍是对象。
模型对照和配置示例见 [JSON 数字契约](number_contract.zh_CN.md)。

上述表示说明适用于受控拥有型值构造，包括唯一值解码和 validated `Preserve`：`-0` 表示为浮点负零；`ExactSafeInteger` 则将其转换为整数 `0`。typed 目标的最终表示仍由目标类型及启用的 Serde feature 决定。

`JcsDecoder` 复用词法准入、session 与事务：第一遍完整扫描并计量，第二遍通过词法事件同步构造 Value，
不收集整份事件数组。事件 builder 在插入前按反转义后的键字符串检查重复项，不执行 Unicode 规范化。
JCS Value 构造复用共享 builder，没有独立固定容器上限；包含根值的配置项 `max_depth` 控制准入，深度超限作为
`Decode` 中的预算错误返回。历史 `JcsDecodeError::MaterializationDepthLimit { limit, line, column }` 变体因源码兼容而保留，
当前 builder 不会产生该变体。`decode_input` 沿用读取到 EOF 并缓冲完整文档的流程，有限输入上限允许一个不计费的探测字节。
预算描述被计量资源，不构成精确 malloc 上限。JCS 解码只检查并构造值，已规范字节验证须由调用方将编码结果与输入比较。根 crate 兼容矩阵以
`serde_json 1.0.135` 和可解析到的最新版本运行 `jcs`、`arbitrary_precision`、`preserve_order`
的八种组合，测试对象是当前 crate 自身。最低基线 `1.0.135` 由 API 边界确定：
`serde_json::Map::into_values()` 从此版本开始提供。若锁文件版本更早，应先执行
`cargo update -p serde_json` 再运行带 `--locked` 的验证；使用最低版本不需要启用
`arbitrary_precision` 或 `preserve_order`。

`JsonValueEncoder` 使用同一个 RawValue 物化器，因此真正的 `serde_json::value::RawValue` payload 也遵循同一配置深度契约。
`InvalidRawValue` 表示严格物化契约拒绝输入，可能涉及重复键、数字约束或深度，并非仅用于语法错误。

有界事务投影入口会在复制或追加超出配置长度前，按 UTF-8 原文字节数限制每个 RawValue 或 Number 私有 payload。
上限包含边界、按 payload 独立计算，不累计、不约束普通字符串，也不计量总内存；旧事务入口不受此上限影响。
超限返回 `PrivateTextLimitExceeded { maximum }`。投影失败后调用方应丢弃整笔事务。此临时文本上限独立于
`JsonEncoder` 的输出字节计量。

私有 Number 协议只有在 Serde struct 名称和 payload 形状完全匹配时才会识别；真实对象中名为 `$serde_json::private::Number` 的键仍是普通键。`RawValue` 私有协议也会识别并严格验证。
有限浮点的紧凑 JSON 文本长度通过公开的 `serde_json::ser::CompactFormatter` 接口计算，
不直接依赖其底层浮点格式化库。

## 受保护值与 Serde 桥接

`SafeJsonValue` 拥有一个 `serde_json::Value`，并通过受保护的迭代式操作执行复制、相等比较和清理，避免这些 owner 操作受树深度的递归调用影响。它不设置精确堆内存上限，也不改变调用方 Serde 目标自身的递归行为。

其通用 `Deserialize` 实现消费 Serde 事件并保护部分构造的值，但没有源 token 来源信息。特别是，map 事件无法区分字面对象和 `serde_json` 的私有任意精度数字协议，因此都作为普通对象处理。依赖启用 `arbitrary_precision` 时，直接桥接 JSON 数字得到的形态可能与未启用时不同。若从 JSON 文本解码并需要文本 decoder 的数字模型，请用 `JsonDecoder::decode_value_str` 或对应的 UTF-8/reader 入口；该路径能识别私有 struct 协议，同时保留使用同名键的字面对象。

`JsonValueRef<'a>` 是对 `&'a Value` 的 `Copy` 借用视图。构造时既不复制也不遍历值树。`Eq` 和 `Debug` 使用 owner
的显式工作栈只读操作；`Debug` 是 Rust 诊断格式，不是 JSON 文本。序列化与 `SafeJsonValue` 共用带栈增长保护的
Serde 桥接。该保护覆盖桥接层自身的递归遍历；具体 serializer 仍可能限制深度，或执行保护范围之外的递归工作；
序列化失败会传播后端错误。这不代表序列化无需分配或具有精确内存上限。借用树的析构仍由其所有者负责。视图不负责
独立清理、修改、反序列化或预算。构造为 O(1) 不代表比较或格式化不分配内存；输出语法和格式由所选 serializer 决定。

直接桥接会在键解码后拒绝重复键，不执行 Unicode 归一化。重复键索引使用额外的 `BTreeSet<String>`，并与已收集成员同时存在；键比较次数为 O(n log n)，每次 `String` 比较的字节成本取决于共同前缀长度。辅助键存储不包含在值预算中。对象仍通过 `SafeValueBuilder` 构造，并遵从当前 `serde_json::Map` 的顺序策略。

## 保留 token 的排版

公开的 `format` 领域提供 `format_json_tokens` 和 `write_json_tokens`。它校验输入恰好是一个完整 JSON 文档，只调整空白；原字符串和数字 token（包括重复键）保持逐字节不变。它不物化 `Value`、不应用数字模型，也不执行规范化键排序。`JsonPrettyOptions` 配置缩进和末尾换行；`JsonPrettyError` 通过 `JsonPrettyErrorKind` 区分语法、输出限额、容量和 writer 错误。

排版计划完成前不会开始输出，因此语法错误和输出限额错误不会写入目标。即使排版后的长度已超过限额，有限计划仍继续扫描和校验完整输入，从而保持后续语法错误优先；此后不再保存新的排版动作，但会保留已保存前缀，所以动作数组不会随剩余宽输入继续增长。扫描耗时仍随输入长度增长，容器栈随嵌套深度增长。格式化输出字节上限不是辅助内存或进程 RSS 上限。真实 writer 故障可能留下已接受的输出前缀。

## Value

`AccountingJsonValueSeed` 是从 Serde 解码事件构造 `serde_json::Value` 的公开 seed。调用方将其绑定到
`JsonValueTransaction`；它适用于原始 JSON 字节不可用而仍需对物化 value 记账的场景。词法
准入与该 seed 的职责不同：前者验证 JSON 文本，后者观察已解码值。seed 看不到原始数字 lexeme，文本级 `NumberBytes` 保证须由 `JsonDecoder` 或采用 JCS 契约的 `JcsDecoder` 提供。seed 收到的原生整数事件仍会按有符号/无符号 64 位并集检查；缺少 token 来源时，map 事件按对象处理，不猜测它是否模拟 Number 协议。

`JsonValidatedDecodeError` 区分普通解码失败、调用方数字策略拒绝、精确安全整数转换失败和附加 validated 限额失败。策略诊断保留源字节偏移，但不会格式化调用方提供的不透明错误内容。`JsonValueEncodeError` 则在把 Serde 值投影到调用方 transaction 时，区分预算拒绝与严格 JSON 序列化失败。

`JsonSyntaxError` 可选记录 scanner 已知的语法位置 `JsonDecodeContext`。原有 `new(...)` 仍创建 `None` context；scanner
生成的错误会在 redacted 与 detailed 诊断下都将 context 传入 validated 错误。该字段参与派生的 `Eq` 和 `Debug`，因此
结构化结果可能变化；`Display` 不显示它，文本保持不变。

validated 解码使用 `JsonValidatedAdmissionPolicy`，默认是 `Immediate`。只有 validated 解码 facade 应用可选的
`TokenFirst`。完整 literal 会先准入并发出事件，再由父对象检查后续分隔符：`{"a":truex}` 因此是在准入 `true` 后，
于对象分隔符处失败，上下文为 `ObjectSeparator`。typed 解码、仅检查语法的 pretty 排版以及 JCS 解码都不因此改变。
内部共用 scanner 或 consumer 不等于它们也共用此选项。

TokenFirst 字符串扫描先按引号内的源文本字节检查额度，再处理字符串解码或 decoded-value 计量。上限为 1 时，未闭合
的 `"ab` 在 EOF 先报语法 `UnexpectedEnd`；完整 `"ab"` 报 `EncodedStringBytes` limit；未闭合的 `"abc` 则在超额的
`ab` 之后还有可读字节时先报 limit。这是有意规定的顺序：每轮先检查 EOF，再检查当前字节是否使额度超限。只有 token
语法完整后，才继续准入解码后的字符串或键长度。

输入计量和值 transaction 分属不同提交边界。之后即使语法、策略或限额检查失败，已读取的输入字节仍计费；暂存的值资源
只有完整文档成功才提交。库无法回滚数字策略回调产生的副作用；此计量约定也不表示所有临时堆分配都受限。

## Tree

`JsonTreeReader` 按现有 `serde_json::Number` 的紧凑显示字节数计量，不转成 `f64`，也不改写当前表示。启用 `arbitrary_precision` 时可据此核算宽数字；这不代表普通或 JCS 编码器会接受它。

`JsonTreeReader` 对不可变 `Value` 执行深度优先 enter/leave 遍历；`JsonTreeMutator` 对可变
`Value` 执行 visitor 驱动的非递归遍历。reader 在对应回调前准入节点及其计量字段；mutator 则在首个
回调前准入完整输入树，并在遍历后准入完整结果。

`JsonTreeReader::account` 复用 reader 的同一非递归遍历，在调用方已有 transaction 中暂存整棵树
的消耗，不调用 visitor，也不创建或提交 transaction；失败直接返回 `MeasuredBudgetError`。
`JsonTreeBudgetTracker` 在完整成功后提交该路径，为整棵 materialized tree 提供自有、可 reset
的 budget。`JsonTreeMutator` 先准入原始 tree，再执行 visitor 驱动的原地修改，最后准入完整的
修改后 tree。它返回 `JsonTreeMutateError::InputBudget`、`::Visitor` 或 `::OutputBudget`；visitor
和输出失败会保留已执行的修改。`JsonTreeControl::SkipSubtree` 只影响后代回调，最终输出记账
仍覆盖所有结果后代。reader 在回调前逐节点准入；mutator 的输出准入则在全部回调完成后执行。
visitor 若创建新后代，需要自行限制回调工作量并确保遍历终止。`JsonTreeProcessError` 区分 reader
基础设施 budget 失败和业务 visitor 失败。

## 公开错误模型

公开错误和诊断类型按领域划分：

1. `decode::JsonDecodeError` 及其 owned `JsonDecodeErrorSource`：普通与规范化 decoder 共用；JCS 专属错误为 `JcsDecodeError`。
2. `decode::JsonValidatedDecodeError`：承载解码、策略、精确整数与附加限额失败及源偏移。
3. `encode::JsonEncodeError`
4. `decode::JsonSyntaxError`
5. `value::traverse::JsonTreeProcessError`
6. `value::traverse::JsonTreeMutateError`
7. `value::JsonValueEncodeError`：区分投影的预算拒绝与严格 JSON 序列化失败。
8. `format::JsonPrettyError`：其 kind 区分语法、输出限额、容量和 I/O 失败。
9. feature-gated 的 `decode::JcsDecodeError` 与 `encode::JcsEncodeError`
10. JSON encoder 共用的 `encode::JsonSerializationError`

每种错误只暴露其领域能稳定提供的上下文；没有根级错误聚合或兼容别名。

## 序列化错误与输出边界

`RawValue` 和 `Number` 私有协议会校验状态；形状错误分别映射为 `InvalidSerializerState` 下的 `InvalidRawValueProtocol`、`InvalidNumberProtocol`，诊断不包含私有 payload。`Display` 生成字符串或键时，先按本次写入后的预计完整长度检查，再追加分块；完成后只对完整字符串/键准入一次。这限制库 collector 的增长，但不能约束用户 `Display::fmt` 内部的分配、分配器元数据或 RSS。Raw JSON payload 仍在全文收集后校验结构。


`JsonEncoder` 与 `JsonValueEncoder` 共用 `JsonSerializationError`。调用方通过 `kind()`
识别精确原因，通过 `category()` 按 `Number`、`ObjectKey`、`RawValue`、`Capacity`、
`SerializerContract` 或 `Custom` 制定恢复策略。细节访问器只暴露整数符号、键形状、集合类型
或序列化状态等非敏感信息。第三方自定义错误文本映射为 `CustomSerialization`，不会进入
公开的 Display、Debug 或错误来源链。`JsonEncodeError::into_source()` 用于取得操作层错误
来源的所有权。具体错误类别见[用户手册](user_guide.zh_CN.md)。

`RawValue` 必须先完整验证，才能将其字节写入外部目标。缓冲写入先完成序列化和预算检查，
再接触目标，但 I/O 失败仍可能留下部分字节；增量写入同样不承诺回滚外部输出。性能优化
必须保持数字范围、重复键、预算提交、错误顺序和部分输出契约。

## JCS 编码与工作内存边界

启用可选 `jcs` feature 后，`JcsEncoder` 接收已经构造好的
`serde_json::Value`。`with_limits(limits, max_working_bytes)` 和
`new(session, max_working_bytes)` 都要求单独指定辅助内存上限；
`max_working_bytes()` 可读取该配置。`unlimited()` 会明确取消值、输出和工作内存限制，
只适合可信值或已由其他层准入的值。

renderer 以迭代方式遍历数组和对象。每个对象只对借用的成员引用按键的 UTF-16 顺序排序，
随后直接向输出写入标点和标量。遍历栈与活动成员表扩容前，会先将其保留容量计入
`max_working_bytes`。这个逻辑容量上限不包含调用方持有的输入 `Value`、分配器元数据或输出缓冲容量。
值限制配置后，渲染前还会执行准入遍历；该遍历栈也不计入 `max_working_bytes`。需要限制准入遍历时，
应设置 `max_depth`，并用 `max_nodes` 限制节点数。这个容量计量不承诺精确堆内存上限；
`max_output_bytes` 限制最终规范化字节数，不限制 renderer 工作内存。

renderer 前先暂存值资源用量，完整编码成功后才提交。`Budget` 表示值或输出记账失败；
`WorkingMemory` 表示辅助内存超限；真实分配失败映射为安全的序列化错误。`write_buffered` 会先
渲染并检查完整文档，再接触目标。`write_incremental` 边写边计入目标接受的字节，并在成功前 flush。
任一 writer 遇到写入失败，都可能留下已接受前缀；这些字节仍计费，但失败操作不会提交值用量。incremental 写入在最后 flush 失败时也可能留下已接受前缀。
错误优先级取决于这些阶段中最先发生的失败。

## 资源计量与性能验证

未启用限制时可跳过仅用于预算的测量和准入，仍须执行语法、数字和键校验。
`JsonValueTransaction::has_limits()` 允许 reader 跳过无限事务的预算计量；mutator 分别根据
输入和输出限制决定是否执行相应的完整树计量。无输出限制的自有缓冲路径直接写入 `Vec<u8>`。

基准应区分严格校验、值预算、输出预算、完整预算、会话构造与复用、缓冲与增量写入，
并覆盖数字、字符串、对象、`RawValue` 和不同树形。比较方法见[基准测试指南](benchmark_baseline.zh_CN.md)。

## 核心不变量与验证边界

- 规范化策略与资源限制独立；不可信输入必须在传输层和解码层分别设置上限。
- 解码失败后保留输入消耗，值消耗只在完整成功后提交；prepare 仅核算一次输入。
- 真正的私有 Number struct 会解析为数字；普通对象中的同名键仍按对象处理。数字文本预算先于范围检查。
- tree 遍历不依赖 Rust 递归；预算失败和 visitor 失败遵循各自的提交及修改边界。
- 默认诊断不保留输入内容，自定义序列化错误不泄露任意来源文本。
- 基准的成功路径必须确认 fixture 准入成功，避免累计预算耗尽后测到错误路径。
- 差分 fuzz 区分词法验证与目标类型构造。普通 typed decode 可能触发 Serde 构造递归保护；
  JCS Value 使用迭代式构造，并受包含根值的配置深度限制，不另设固定容器上限。因此只有保守的浅层文档才直接比较物化值。
  超长 fuzz 输入整体拒绝，不截断后比较。

## 相关文档

- [用户手册](user_guide.zh_CN.md) · [User guide](user_guide.md)
- [数字契约](number_contract.zh_CN.md) · [Number contract](number_contract.md)
- [基准测试指南](benchmark_baseline.zh_CN.md) · [Benchmark guide](benchmark_baseline.md)
