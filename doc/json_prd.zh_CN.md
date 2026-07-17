# `rs-json` 产品需求文档（PRD）

## 文档信息

- 文档名称：`rs-json` 产品需求文档（PRD）
- 文档版本：`v3.0`
- 创建日期：`2026-04-12`
- 更新日期：`2026-07-15`
- 状态：`Draft`
- 对齐设计文档：`json_design.zh_CN.md`

## 1. 产品定义

`rs-json` 的核心产品是一个通用的宽松 JSON 解码器
`LenientJsonDecoder`。该组件用于处理“非完全可信文本”下的 JSON 解码场景，
例如来自日志、模板或通道输出的文本 JSON。

解码器目标是：

1. 对输入文本执行可配置的、有限且可预测的规范化。
2. 以 `serde_json::Value` 或强类型结果返回解码结果。
3. 在解码阶段统一错误语义，并默认移除输入派生的 serde 诊断内容。

它不替代 `serde_json`，也不做激进修复式解析。

## 2. 设计目标

1. **对象化 API**
   - 对外核心能力由 `LenientJsonDecoder` 提供，不是零散工具函数集合。
2. **可复用配置**
   - 通过 `JsonDecodeOptions` 固化解码策略。
3. **稳定解码入口**
   - 提供 `decode<T>()`、`decode_object<T>()`、`decode_array<T>()`
     与 `decode_value()` 四类主要入口。
4. **一致错误模型**
   - 通过 `JsonDecodeError` 与 `JsonDecodeErrorKind` 表达空输入、语法错误、
     顶层结构冲突、反序列化失败。
5. **默认保护输入隐私**
   - 默认错误可用于普通日志；完整 serde 诊断必须由调用方显式启用。
6. **实现与上层隔离**
   - 避免平台、provider、网络、日志等上层上下文依赖。

## 3. 非目标

1. 不提供通用 JSON 重建能力，不补全缺失引号/逗号/括号。
2. 不提供 JSON Schema、业务字段校验。
3. 不在公共 API 暴露 `Option<&str>`，调用方需在上游明确输入缺失语义。
4. 不公开 LLM 专属命名或场景化方法名。

## 4. 用户场景

1. 解码被 Markdown 包裹的 JSON 片段。
2. 处理含 UTF-8 BOM、首尾噪声或轻度格式问题的文本。
3. 解码常见对象、数组及动态 JSON 值。
4. 在同一业务上下文复用同一套规范化策略。

## 5. 目标范围

### MVP（P0）

1. `LenientJsonDecoder` 对象与默认构造能力。
2. `LenientJsonDecoder::new(options)` 与 `LenientJsonDecoder::default()`。
3. 四个公开解码方法。
4. `JsonDecodeOptions` 配置项与默认值。
5. `JsonDecodeError`、`JsonDecodeErrorKind`、`JsonTopLevelKind` 与
   `ErrorPrivacyPolicy`。
6. 内部规范化管线：空输入检查、trim、BOM 移除、代码块移除、控制字符转义。
7. 配套单元测试与文档说明。

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
    `max_input_bytes`、`error_privacy_policy`。
  - 所有字段保持私有，每个选项均有只读 getter 和值式 `with_*` builder。
  - `with_max_input_bytes(Some(limit))` 设置限制，传入 `None` 可清除限制。
  - `markdown_fence_policy` 用单一枚举表达禁用、任意语言和仅 JSON 围栏，以及
    可选或必须闭合，避免多个布尔字段组合出矛盾状态。
  - 默认值覆盖高频轻度脏数据场景。
  - 默认实例与配置实例行为可回归验证。

### PRD-RSJSON-003：统一规范化顺序

- 验收标准
  - 规范化顺序固定为：
    `require_within_size_limit -> require_non_empty -> trim -> strip_bom -> trim -> strip_fence -> trim -> escape_control_chars`。
  - 不发生修改时尽量复用输入，减少分配。

### PRD-RSJSON-004：`decode<T>()`

- 验收标准
  - 可将规范化后的文本反序列化为任意 `T: DeserializeOwned`。
  - 与 `decode_object`、`decode_array` 区分顶层约束责任。
  - 区分 `InvalidJson` 与 `Deserialize` 两类失败。

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
  - 支持 `InputTooLarge`、`EmptyInput`、`InvalidJson`、
    `UnexpectedTopLevel`、`Deserialize`。
  - 保留失败阶段、行列号和输入字节数信息用于排障。
  - 初始空输入的规范化长度为 `None`；完整管线处理后变为空时为 `Some(0)`。

### PRD-RSJSON-009：可配置错误隐私

- 验收标准
  - `ErrorPrivacyPolicy::Redacted` 为所有预设和默认配置的策略。
  - `Redacted` 的 `message`、`Display`、`Debug` 和标准错误链均不保留
    serde 提供的输入派生内容，但继续提供结构化行列信息。
  - `Detailed` 只能显式配置，并保留完整 serde 消息和标准 error source。
  - 每个 `JsonDecodeError` 均通过 `privacy_policy()` 暴露实际生效策略。

## 7. 风险与约束

1. 规则过少导致覆盖不足：M1 只覆盖“温和修复”规则，避免过度猜测。
2. 规则过多导致行为不可预测：通过配置开关逐步收敛，不做隐式增强。
3. API 演进影响：`JsonDecodeOptions` 使用私有字段与完整 getter/builder，避免新增
   配置迫使下游修改 struct literal。
4. 诊断泄露风险：默认丢弃 serde 明细；启用 `Detailed` 的调用方负责保护日志与
   错误链。

## 8. 与实现对齐检查

- 配置模型与当前实现一致：`JsonDecodeOptions` 与默认值与代码保持一致。
- 隐私模型与当前实现一致：默认 `Redacted`，`Detailed` 需显式启用。
- 解码入口与实现一致：`decode` / `decode_object` / `decode_array` / `decode_value` 均通过
  内部统一规范化。
- 解析流程与实现一致：`normalize` 托管在 `internal/lenient_json_normalizer.rs`，对外不暴露底层 helper。
- 错误模型与实现一致：`JsonDecodeErrorKind` 与 `JsonTopLevelKind` 已对齐。
- 复用与对象语义一致：`LenientJsonDecoder` 持有不可变的
  `LenientJsonNormalizer`，可安全多次复用。

## 9. 文档与测试一致性

- 文档中的行为描述必须与 [json_design.zh_CN.md](json_design.zh_CN.md) 保持一致。
- 公开能力必须在测试目录中可观测：
  - `tests/lenient_json_decoder_tests.rs`
  - `tests/internal/lenient_json_normalizer_tests.rs`
  - `tests/internal/control_character_escaper_tests.rs`
  - `tests/json_decode_options_tests.rs`
  - `tests/error_privacy_policy_tests.rs`
  - `tests/json_decode_error_tests.rs`
  - `tests/json_top_level_kind_tests.rs`
  - `tests/json_decode_error_kind_tests.rs`
  - `tests/json_decode_stage_tests.rs`
  - `tests/markdown_fence_closing_tests.rs`
  - `tests/markdown_fence_policy_tests.rs`

- `benches/decoder_bench.rs` 提供公开解码路径的 Criterion 基准。
- `fuzz/fuzz_targets/decoder.rs` 覆盖主要配置组合，并由
  `.github/workflows/fuzz.yml` 定时执行有时限的 fuzz。

以上清单与目前代码目录保持一致，避免文档与实现的漂移。
