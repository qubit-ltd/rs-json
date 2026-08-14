# `rs-json` 产品需求文档（PRD）

## 文档信息

- 文档名称：`rs-json` 产品需求文档（PRD）
- 文档版本：`v4.0`
- 创建日期：`2026-04-12`
- 更新日期：`2026-08-14`
- 状态：`Implemented`
- 对齐设计文档：`json_design.zh_CN.md`

## 1. 产品定义

`rs-json` 是通用 JSON 基础设施。它面向需要稳定资源核算和明确失败边界的 Rust
调用方，提供四个公开能力域：`lenient`、`text`、`value`、`tree`。

解码器目标是：

1. 通过 `lenient` 对非完全可信文本执行有限规范化，并直接得到强类型结果。
2. 通过 `text` 严格编解码，通过 `value` 构造预算感知 DOM，通过 `tree` 迭代访问或修改
   已物化 JSON。
3. 使用 `qubit_budget::json` session 统一核算输入、输出和 value 资源。
4. 在错误模型中稳定表达隐私、失败阶段和结构化 budget rejection。

它不替代 `serde_json` 的数据模型和 Serde 行为，也不做激进修复式解析。

## 2. 设计目标

1. **对象化 API**
   - 对外核心能力由 `LenientJsonDecoder` 提供，不是零散工具函数集合。
2. **可复用配置**
   - 通过 `JsonDecodeOptions` 固化解码策略。
3. **稳定解码入口**
   - 提供 `decode<T>()`、`decode_slice<T>()`、`decode_object<T>()`、
     `decode_array<T>()` 与 `decode_value()` 五个主要入口。
4. **一致错误模型**
   - 通过 `JsonDecodeError` 与 `JsonDecodeErrorKind` 表达空输入、语法错误、
     顶层结构冲突、反序列化失败和预算拒绝。
5. **默认保护输入隐私**
   - 默认错误可用于普通日志；完整 serde 诊断必须由调用方显式启用。
6. **实现与上层隔离**
   - 避免平台、provider、网络、日志等上层上下文依赖。
7. **模块边界明确**
   - `lenient` 只负责宽松文本入口；`text` 只负责严格编解码；`value` 只公开 budgeted
     seed；`tree` 负责物化树遍历。

## 3. 非目标

1. 不提供通用 JSON 重建能力，不补全缺失引号/逗号/括号。
2. 不提供 JSON Schema、业务字段校验。
3. 不在公共 API 暴露 `Option<&str>`，调用方需在上游明确输入缺失语义。
4. 不公开 LLM 专属命名或场景化方法名。
5. 不承诺错误时回滚累计 session 消费或 `process_mut` 已完成的 mutation。
6. 不在当前 crate 重导出由 `qubit-budget` 或 `serde_json` 所有的外部类型。

## 4. 用户场景

1. 解码被 Markdown 包裹的 JSON 片段。
2. 处理含 UTF-8 BOM、首尾噪声或轻度格式问题的文本。
3. 解码常见对象、数组及动态 JSON 值。
4. 在同一业务上下文复用同一套规范化策略。
5. 在同一调用方 session 中累计限制多次 decode 尝试的 raw、normalized 和 value 资源。
6. 严格编解码受限 JSON，或在预算内构造、遍历和修改 `serde_json::Value`。

## 5. 目标范围

### MVP（P0）

1. `LenientJsonDecoder` 对象与默认构造能力。
2. `LenientJsonDecoder::new(options)` 与 `LenientJsonDecoder::default()`。
3. 五个公开解码方法。
4. `JsonDecodeOptions` 配置项与默认值。
5. `JsonDecodeError`、`JsonDecodeErrorKind`、`JsonTopLevelKind` 与
   `ErrorPrivacyPolicy`。
6. 内部规范化管线：空输入检查、trim、BOM 移除、代码块移除、控制字符转义。
7. 配套单元测试与文档说明。
8. `decode_with_session<T>()` 三阶段累计预算准入。
9. `text`、`value`、`tree` 通用 JSON 基础设施。
10. `serde_json` 私有协议集中兼容边界和精确版本升级门禁。

### M2（候选）

1. 更细粒度的错误上下文扩展（保留现有错误分类）。
2. 更灵活的代码块识别策略（如白名单标签、闭合策略增强）。
3. 需要时增加比 `Redacted`/`Detailed` 更细粒度的诊断输出策略。

## 6. 需求与验收标准

### PRD-RSJSON-001：对象化对外模型

- 验收标准
  - 调用方可实例化并复用 `LenientJsonDecoder`。
  - 方法为不可变调用，不依赖内部可变状态。
  - 通过 `options()` 能读取生效配置。

### PRD-RSJSON-002：配置化规范化

- 验收标准
  - 配置字段包含 `trim_whitespace`、`strip_utf8_bom`、
    `markdown_fence_policy`、`escape_control_chars_in_strings`、
    `max_input_bytes`、`max_normalized_bytes`、`error_privacy_policy`。
  - 所有字段保持私有，每个选项均有只读 getter 和值式 `with_*` builder。
  - `with_max_input_bytes(Some(limit))` 设置限制，传入 `None` 可清除限制。
  - `with_max_normalized_bytes(Some(limit))` 在控制字符修复分配前限制规范化后 JSON
    字节数，传入 `None` 可清除限制。
  - `markdown_fence_policy` 用单一枚举表达禁用、任意语言和仅 JSON 围栏，以及
    可选或必须闭合，避免多个布尔字段组合出矛盾状态。
  - 默认只移除空标签、`json` 或 `jsonc` 围栏；任意语言需显式选择 `Any`。
  - 默认实例与配置实例行为可回归验证。

### PRD-RSJSON-003：统一规范化顺序

- 验收标准
  - 规范化顺序固定为：
    `require_within_size_limit -> require_non_empty -> trim -> strip_bom -> trim -> strip_fence -> trim -> require_within_normalized_size_limit -> escape_control_chars`。
  - 不发生修改时尽量复用输入，减少分配。

### PRD-RSJSON-004：`decode<T>()`

- 验收标准
  - 可将规范化后的文本反序列化为任意 `T: DeserializeOwned`。
  - 与 `decode_object`、`decode_array` 区分顶层约束责任。
  - 区分 `InvalidJson` 与 `Deserialize` 两类失败。
  - `decode_slice<T>()` 先按原始字节执行大小限制，再完整校验 UTF-8，并复用
    相同的字符串解码管线。UTF-8 校验必须发生在调用目标类型的反序列化逻辑
    之前，任何选项组合都不得绕过该校验。

### PRD-RSJSON-005：`decode_object<T>()`

- 验收标准
  - 输入不是合法 JSON 时返回 `InvalidJson`，不提前归类为顶层类型不匹配。
  - 完成顶层对象检查后直接反序列化目标类型，不经由 `Value` 中转，从而保留
    serde 的重复字段检测与精确数值语义。
  - 输入顶层非对象时返回 `UnexpectedTopLevel`。
  - 顶层为对象但结构不匹配时返回 `Deserialize`。

### PRD-RSJSON-006：`decode_array<T>()`

- 验收标准
  - 输入不是合法 JSON 时返回 `InvalidJson`，不提前归类为顶层类型不匹配。
  - 输入顶层非数组时返回 `UnexpectedTopLevel`。
  - 顶层为数组且元素可反序列化时返回 `Vec<T>`。

### PRD-RSJSON-007：`decode_value()`

- 验收标准
  - 返回动态 `serde_json::Value`，可供上游二次处理。
  - 与结构化反序列化路径共享同一规范化逻辑。

### PRD-RSJSON-008：错误模型稳定性

- 验收标准
  - 支持 `InputTooLarge`、`EmptyInput`、`InvalidUtf8`、`InvalidJson`、
    `UnexpectedTopLevel`、`Deserialize`。
  - 保留失败阶段、行列号和输入字节数信息用于排障。
  - 初始空输入的规范化长度为 `None`；完整管线处理后变为空时为 `Some(0)`。

### PRD-RSJSON-009：可配置错误隐私

- 验收标准
  - `ErrorPrivacyPolicy::Redacted` 为所有预设和默认配置的策略。
  - `Redacted` 的 `message`、`Display`、`Debug` 和标准错误链均不保留
    serde 提供的输入派生内容，但继续提供结构化行列信息。
  - `Detailed` 只能显式配置，并保留完整 UTF-8 或 serde 标准 error source。
  - 每个 `JsonDecodeError` 均通过 `privacy_policy()` 暴露实际生效策略。

### PRD-RSJSON-010：完整 decode session 预算

- 验收标准
  - `decode_with_session<T>()` 接受调用方持有的
    `JsonDecodeSession<'_, JsonResource>`。
  - 按 raw input、normalized input、value admission 的顺序累计计费。
  - value admission 覆盖节点、深度、单容器成员数、键、字符串、数字和共享 payload。
  - 成功消费在后续 budget、syntax 或 target deserialize 失败时不回滚；被拒绝的单次增量
    保持原子性。
  - value 超限返回 `JsonDecodeErrorKind::Budget`、`JsonDecodeStage::Admission`，并通过
    `measured_budget_error()` 暴露资源和限制详情。
  - 普通 `decode()` 不执行 value preflight，继续作为快速路径。

### PRD-RSJSON-011：直接反序列化语义

- 验收标准
  - lexical preflight 准入后直接调用 `serde_json` 反序列化 `T`，不得经由
    `serde_json::Value` 中转。
  - 保留目标类型的重复字段检查、自定义 `Deserialize` 行为和 `u128` 等精确数值语义。
  - lexical parser 与 `serde_json` 对语法接受集合不一致时，不得 panic；普通 serde 语法
    错误继续保留 serde 的稳定行列位置。

### PRD-RSJSON-012：`text`、`value`、`tree` 公共边界

- 验收标准
  - `text` 提供严格 session decode/encode 和独立的公开错误类型；
    `JsonEncodeError::InvalidRawJson` 携带稳定 `JsonSyntaxError`。
  - `value::BudgetedJsonValueSeed` 是 budgeted `serde_json::Value` seed 的唯一公开路径，
    不保留 `budget` 或 `tree` 兼容 alias。
  - `JsonTreeProcessor::process` 的 value borrow 不与 budget borrow 绑定。
  - `process_mut` 文档和测试明确：错误时保留此前 mutation 和 budget 消费；guard 只保证
    root 仍为有效 `Value`。

### PRD-RSJSON-013：私有协议升级门禁

- 验收标准
  - 生产代码仅由 `serde_json_compat.rs` 持有并分类 serde_json 私有 Number/RawValue
    token。
  - `serde_json` 生产依赖精确固定为经验证版本。
  - 升级时同步更新 root/fuzz lockfile，复核上游私有 serializer，并运行真实私有形状、
    near-miss、`collect_str`、依赖树和 fuzz workspace 检查。

## 7. 风险与约束

1. 规则过少导致覆盖不足：M1 只覆盖“温和修复”规则，避免过度猜测。
2. 规则过多导致行为不可预测：通过配置开关逐步收敛，不做隐式增强。
3. API 演进影响：`JsonDecodeOptions` 使用私有字段与完整 getter/builder，避免新增
   配置迫使下游修改 struct literal。
4. 诊断泄露风险：默认丢弃 serde 明细；启用 `Detailed` 的调用方负责保护日志与
   错误链。
5. 双扫描成本：`decode_with_session` 的 lexical preflight 与目标反序列化会各扫描一次
   规范化文本；这是“构造前资源准入”和“保留原生 Serde 语义”的明确取舍。
6. 私有协议风险：serde_json Number/RawValue 协议不是公共稳定 API，必须通过精确版本和
   升级清单控制。

## 8. 与实现对齐检查

- 配置模型与当前实现一致：`JsonDecodeOptions` 与默认值与代码保持一致。
- 隐私模型与当前实现一致：默认 `Redacted`，`Detailed` 需显式启用。
- 解码入口与实现一致：`decode` / `decode_slice` / `decode_object` /
  `decode_array` / `decode_value` 均通过内部统一规范化；`decode_with_session` 额外执行
  value admission。
- 解析流程与实现一致：`normalize` 托管在 `src/internal/lenient_json_normalizer.rs`，对外不暴露底层 helper。
- 错误模型与实现一致：`JsonDecodeErrorKind` 与 `JsonTopLevelKind` 已对齐。
- 复用与对象语义一致：`LenientJsonDecoder` 持有不可变的
  `LenientJsonNormalizer`，可安全多次复用。
- 公共能力域与实现一致：crate root 公开 `lenient`、`text`、`value`、`tree`。
- 失败语义与实现一致：decode session 和 mutable tree 处理均不回滚已经完成的工作。

## 9. 文档与测试一致性

- 文档中的行为描述必须与 [json_design.zh_CN.md](json_design.zh_CN.md) 保持一致。
- 公开能力必须在测试目录中可观测：
  - `tests/lenient_json_decoder_tests.rs`
  - `tests/internal/lenient_json_normalizer_tests.rs`
  - `tests/internal/control_character_escaper_tests.rs`
  - `tests/internal/markdown_fence_tests.rs`
  - `tests/options/json_decode_options_tests.rs`
  - `tests/error/error_privacy_policy_tests.rs`
  - `tests/error/internal/json_input_size_limit_tests.rs`
  - `tests/error/json_decode_error_tests.rs`
  - `tests/json_top_level_kind_tests.rs`
  - `tests/error/json_decode_error_kind_tests.rs`
  - `tests/error/json_decode_stage_tests.rs`
  - `tests/options/markdown_fence_closing_tests.rs`
  - `tests/options/markdown_fence_policy_tests.rs`
  - `tests/text/**`
  - `tests/budget/budgeted_json_value_seed_tests.rs`
  - `tests/budget/internal/json_private_tests.rs`
  - `tests/tree/json_tree_processor_tests.rs`
  - `tests/tree/json_tree_mut_processor_tests.rs`

- `benches/decoder_bench.rs` 与 `benches/budgeted_serde_json.rs` 分别提供宽松公开入口和
  多规模 budgeted strict/lenient 路径的 Criterion 基准。
- `fuzz/fuzz_targets/decoder.rs`、`json_budget_invariants.rs`、
  `json_decode_differential.rs`、`json_encode_invariants.rs` 覆盖主要配置和预算不变量，并由
  `.github/workflows/fuzz.yml` 定时执行有时限的 fuzz。

以上清单与目前代码目录保持一致，避免文档与实现的漂移。
