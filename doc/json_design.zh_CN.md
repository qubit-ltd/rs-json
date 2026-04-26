# `rs-json` 实现方案（`LenientJsonDecoder`）

## 版本信息

- 文档版本：`v2.1`
- 创建日期：`2026-04-12`
- 对齐 PRD：`json_prd.zh_CN.md`

## 1. 背景与目标

`LenientJsonDecoder` 定位为通用的宽松 JSON 解码器，不依赖业务场景。它关注的是
“文本到 JSON 的可复用解码预处理+错误统一化”，而不是构建一个通用 JSON 修复引擎。

目标行为是：

1. 对输入文本执行一组有限、可预测、可配置的规范化。
2. 在规范化后交给 `serde_json` 进行解析和反序列化。
3. 通过统一错误模型表达失败类型，避免上游分别处理多套异常语义。

## 2. 核心边界

### 非目标

1. 不代替 `serde_json` 提供完整 JSON 工具链。
2. 不做激进修复（补引号、补逗号、括号匹配、引号风格转换等）。
3. 不引入 `Option<&str>` 作为公共解码语义。
4. 不引入外部 runtime 或 provider 依赖。

## 3. 总体架构

```text
input text
    |
    v
LenientJsonDecoder
    |-- options-aware LenientJsonNormalizer
    |    |-- trim_whitespace
    |    |-- strip_utf8_bom
    |    |-- strip_markdown_code_fence
    |    |-- escape_control_chars_in_strings
    |
    |-- decode<T>()                 // normalized text -> T
    |-- decode_value()              // normalized text -> Value
    |-- decode_object<T>()          // normalized text -> Value -> top-level check -> T
    |-- decode_array<T>()           // normalized text -> Value -> top-level check -> Vec<T>
    |
    v
serde_json / typed output
```

设计原则：

1. 对外以对象 API 为中心，不以工具函数列表为中心。
2. 规范化作为解码内部阶段，保持对象边界稳定。
3. 内部组件按职责拆分，`lenient_json_normalizer.rs` 仅承载内部预处理策略。

## 4. 核心对象模型

### 4.1 `LenientJsonDecoder`

`LenientJsonDecoder` 封装只读配置，并持有内部 `LenientJsonNormalizer`。

```rust
#[derive(Debug, Clone)]
pub struct LenientJsonDecoder {
    normalizer: LenientJsonNormalizer,
}
```

职责：

1. 提供统一的公开解码入口。
2. 共享并复用同一套 `LenientJsonNormalizer` 行为。
3. 对 `&self` 调用提供可复用、可复制的行为。

### 4.2 `LenientJsonNormalizer`（内部对象）

`LenientJsonNormalizer` 是内部对象，`lenient_json_decoder.rs` 中通过实例调用其
`normalize()`。

```rust
#[derive(Debug, Clone, Copy)]
pub(crate) struct LenientJsonNormalizer {
    options: JsonDecodeOptions,
}
```

职责：

1. 在解析前执行统一的输入规范化。
2. 保持配置不变性：一次构造，全生命周期不变。
3. 当规则不要求改写时尽量返回借用视图，降低开销。

### 4.3 配置对象

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonDecodeOptions {
    pub trim_whitespace: bool,
    pub strip_utf8_bom: bool,
    pub strip_markdown_code_fence: bool,
    pub strip_markdown_code_fence_requires_closing: bool,
    pub strip_markdown_code_fence_json_only: bool,
    pub escape_control_chars_in_strings: bool,
    pub max_input_bytes: Option<usize>,
}
```

默认值：

- `trim_whitespace = true`
- `strip_utf8_bom = true`
- `strip_markdown_code_fence = true`
- `strip_markdown_code_fence_requires_closing = false`
- `strip_markdown_code_fence_json_only = false`
- `escape_control_chars_in_strings = true`
- `max_input_bytes = None`

### 4.4 错误模型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonTopLevelKind { Object, Array, Other }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonDecodeErrorKind {
    InputTooLarge,
    EmptyInput,
    InvalidJson,
    UnexpectedTopLevel,
    Deserialize,
}

#[derive(Debug)]
pub struct JsonDecodeError {
    pub kind: JsonDecodeErrorKind,
    pub stage: JsonDecodeStage,
    pub message: String,
    pub expected_top_level: Option<JsonTopLevelKind>,
    pub actual_top_level: Option<JsonTopLevelKind>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub input_bytes: Option<usize>,
    pub max_input_bytes: Option<usize>,
}
```

设计说明：

1. `JsonDecodeError` 承担错误场景聚合与诊断信息承载。
2. `stage` 用于标识失败发生在规范化、解析、顶层检查或反序列化阶段。
3. `line`/`column` 用于解析和反序列化阶段定位，无法定位时保持 `None`。
4. `expected_top_level`/`actual_top_level` 仅用于 `UnexpectedTopLevel`。
5. `input_bytes`/`max_input_bytes` 用于输入大小限制和解析诊断。

## 5. 公开 API 设计

### 5.1 `LenientJsonDecoder` 方法

```rust
impl LenientJsonDecoder {
    pub const fn new(options: JsonDecodeOptions) -> Self;
    pub const fn options(&self) -> &JsonDecodeOptions;

    pub fn decode<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: serde::de::DeserializeOwned;

    pub fn decode_object<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: serde::de::DeserializeOwned;

    pub fn decode_array<T>(&self, input: &str) -> Result<Vec<T>, JsonDecodeError>
    where
        T: serde::de::DeserializeOwned;

    pub fn decode_value(&self, input: &str) -> Result<serde_json::Value, JsonDecodeError>;
}
```

### 5.2 行为说明

- `decode<T>()`：不限定顶层结构，规范化后直接反序列化为 `T`。
- `decode_object<T>()`：先解析为 `Value`，确认顶层为对象后再反序列化为 `T`。
- `decode_array<T>()`：先解析为 `Value`，确认顶层为数组后再反序列化为 `Vec<T>`。
- `decode_value()`：先规范化再直接解析为 `serde_json::Value`。

## 6. 规范化管线

实现统一在 `src/lenient_json_normalizer.rs`，对外不直接暴露独立函数 API。
核心处理顺序如下：

1. `require_within_size_limit(input)`：按字节数上限拒绝过大输入。
2. `require_non_empty(input)`：按 trim 策略判定空输入。
3. `trim_if_enabled(input)`：首尾空白清理。
4. `strip_utf8_bom(input)`：可配置移除 UTF-8 BOM。
5. `trim_if_enabled(input)`：移除 BOM 后再次按需裁剪。
6. `strip_markdown_code_fence(input)`：可配置去除外层代码块。
7. `trim_if_enabled(input)`：去除代码块后再次按需裁剪。
8. `escape_control_chars_in_json_strings(input)`：可配置转义字符串内控制字符。
9. `trim_cow_if_enabled(input)`：规范化后再次处理尾部空白。
10. 最终空值检查并返回 `Cow<'_, str>`。

该管线通过 `LenientJsonNormalizer::normalize()` 单一入口触发，保证顺序不变。

### 6.1 关键算法要点

- `strip_markdown_code_fence`
  - 仅处理以 ````` 开头的输入。
  - 支持语言标签和无标签两种 fence 开头。
  - 若存在结束 fence，尝试一并去除；不存在时仍移除开头并保留剩余内容。
- `escape_control_chars_in_json_strings`
  - 通过字符串状态机识别 `in_string` 与 `in_escape`。
  - 仅处理 JSON 字符串中的 `0x00..=0x1F`。
  - 先有转义序列不二次转义。
- `require_non_empty`
  - 默认通过 trim 后判断空串。
  - 禁用 trim 时仅判空 `""`。

## 7. 与实现对齐性

1. `lenient_json_normalizer.rs` 采用 `LenientJsonNormalizer` 对象模型，而非全局函数集合。
2. `lenient_json_decoder.rs` 使用单一 `normalize` 调用，避免重复逻辑。
3. `decode_object` 与 `decode_array` 通过 `JsonTopLevelKind` 做一致约束检查。
4. 错误映射路径：
   - 解析失败 -> `InvalidJson`。
   - 顶层不匹配 -> `UnexpectedTopLevel`。
   - 反序列化失败 -> `Deserialize`。

## 8. 目录结构

```text
rust-common/rs-json/
  ├─ src/
  │   ├─ lib.rs
  │   ├─ lenient_json_decoder.rs
  │   ├─ json_decode_options.rs
  │   ├─ json_decode_error.rs
  │   ├─ json_decode_error_kind.rs
  │   ├─ json_top_level_kind.rs
  │   └─ lenient_json_normalizer.rs
  ├─ tests/
  │   ├─ mod.rs
  │   ├─ lenient_json_decoder_tests.rs
  │   ├─ json_decode_error_kind_tests.rs
  │   ├─ json_decode_error_tests.rs
  │   ├─ lib_tests.rs
  │   ├─ lenient_json_normalizer_tests.rs
  │   ├─ json_decode_options_tests.rs
  │   └─ json_top_level_kind_tests.rs
  └─ doc/
      ├─ json_prd.zh_CN.md
      └─ json_design.zh_CN.md
```

## 9. 测试策略

### 9.1 解码路径测试

- `tests/lenient_json_decoder_tests.rs`
  - `decode`、`decode_object`、`decode_array`、`decode_value` 的正常与失败路径。

### 9.2 配置与错误模型测试

- `tests/json_decode_options_tests.rs`：默认值、字段覆盖与行为一致性。
- `tests/json_decode_error_tests.rs`、`tests/json_decode_error_kind_tests.rs`、`tests/json_top_level_kind_tests.rs`：
  - 错误种类、顶层类型映射。

### 9.3 规范化测试

- `tests/lenient_json_normalizer_tests.rs`：BOM、代码块、控制字符、空输入、trim
  行为。

## 10. 接入与发布边界

本库对外只承诺可复用的解码器对象，不约束调用方上游协议。
如需传入可能缺失的输入、重试策略、缓存或来源特异规则，建议由上层做封装。
