# `rs-json` 实现方案（`LenientJsonDecoder`）

## 版本信息

- 文档版本：`v3.1`
- 创建日期：`2026-04-12`
- 更新日期：`2026-07-18`
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
    |    |-- markdown_fence_policy
    |    |-- escape_control_chars_in_strings
    |    |-- max_input_bytes
    |    `-- error_privacy_policy
    |
    |-- decode<T>()                 // normalized text -> T
    |-- decode_slice<T>()           // raw UTF-8 bytes -> T
    |-- decode_value()              // normalized text -> Value
    |-- decode_object<T>()          // top-level token check -> direct T decode
    |-- decode_array<T>()           // top-level token check -> direct Vec<T> decode
    |
    v
serde_json / typed output
```

设计原则：

1. 对外以对象 API 为中心，不以工具函数列表为中心。
2. 规范化作为解码内部阶段，保持对象边界稳定。
3. 内部组件按职责拆分：`internal/lenient_json_normalizer.rs` 承载预处理策略，
   `internal/markdown_fence.rs` 负责 Markdown 围栏识别与剥离，
   `internal/control_character_escaper.rs` 以单次、惰性分配扫描处理字符串内 C0 控制字符。

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
    trim_whitespace: bool,
    strip_utf8_bom: bool,
    markdown_fence_policy: MarkdownFencePolicy,
    escape_control_chars_in_strings: bool,
    max_input_bytes: Option<usize>,
    error_privacy_policy: ErrorPrivacyPolicy,
}
```

字段全部私有，以便后续增加配置时不破坏下游 struct literal。每个字段均提供同名
getter，并提供以下值式 builder：

- `JsonDecodeOptions::lenient()`：返回默认宽松配置。
- `JsonDecodeOptions::strict()`：禁用所有文本改写规则，但保留空输入分类、可选原始
  输入大小限制、隐私策略与稳定错误映射。
- `with_trim_whitespace(enabled)`。
- `with_strip_utf8_bom(enabled)`。
- `with_markdown_fence_policy(policy)`。
- `with_escape_control_chars_in_strings(enabled)`。
- `with_max_input_bytes(Some(limit))` 设置上限，`with_max_input_bytes(None)` 清除上限。
- `with_error_privacy_policy(policy)`。

默认值：

- `trim_whitespace = true`
- `strip_utf8_bom = true`
- `markdown_fence_policy = JsonOnly { closing: Optional }`
- `escape_control_chars_in_strings = true`
- `max_input_bytes = None`
- `error_privacy_policy = ErrorPrivacyPolicy::Redacted`

### 4.4 错误模型

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonTopLevelKind { Object, Array, Other }

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonDecodeErrorKind {
    InputTooLarge,
    EmptyInput,
    InvalidUtf8,
    InvalidJson,
    UnexpectedTopLevel,
    Deserialize,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorPrivacyPolicy {
    #[default]
    Redacted,
    Detailed,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct JsonDecodeError {
    // all fields are private; immutable accessors expose diagnostics
    privacy_policy: ErrorPrivacyPolicy,
    source: Option<Arc<dyn Error + Send + Sync>>,
}
```

设计说明：

1. `JsonDecodeError` 承担错误场景聚合与诊断信息承载。
2. `stage` 用于标识失败发生在字节转文本、规范化、解析、顶层检查或反序列化阶段。
3. `normalized_line()`/`normalized_column()` 用于解析和反序列化阶段定位，
   坐标相对于规范化后的 JSON 文本，无法定位时保持 `None`。
4. `expected_top_level`/`actual_top_level` 仅用于 `UnexpectedTopLevel`。
5. `raw_input_bytes()`、`normalized_input_bytes()` 与 `max_input_bytes()` 用于
   输入大小限制和解析诊断。
6. `privacy_policy()` 记录错误构造时实际生效的诊断策略，并参与稳定字段相等性
   比较。
7. 默认 `Redacted` 在错误构造时只保留稳定前缀和规范化后行列，不格式化或保存
   `serde_json::Error`；因此 `message`、`Display`、`Debug` 和标准 error source
   均不含 serde 提供的输入派生内容。
8. 显式 `Detailed` 保留 `{prefix}: {serde_error}` 消息及底层 source，可能暴露
   输入值，只适用于受控诊断环境。
9. 规范化和顶层类型检查错误本身不含输入内容，但同样记录生效隐私策略。

## 5. 公开 API 设计

### 5.1 `LenientJsonDecoder` 方法

```rust
impl LenientJsonDecoder {
    pub const fn new(options: JsonDecodeOptions) -> Self;
    pub const fn options(&self) -> &JsonDecodeOptions;

    pub fn decode<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: serde::de::DeserializeOwned;

    pub fn decode_slice<T>(&self, input: &[u8]) -> Result<T, JsonDecodeError>
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
- `decode_slice<T>()`：先按原始字节检查上限，再完整校验 UTF-8，借用为
  `&str` 后复用 `decode<T>()`；校验发生在目标类型反序列化之前，任何选项组合
  都不得绕过，有效输入不复制。
- `decode_object<T>()`：先检查首个 JSON token。若 token 为对象，直接从规范化文本
  反序列化为 `T`；若 token 不匹配，再借助 `RawValue` 验证完整语法，以区分
  `InvalidJson` 与 `UnexpectedTopLevel`。
- `decode_array<T>()`：先检查首个 JSON token。若 token 为数组，直接从规范化文本
  反序列化为 `Vec<T>`；若 token 不匹配，再借助 `RawValue` 验证完整语法。
- `decode_value()`：先规范化再直接解析为 `serde_json::Value`。

## 6. 规范化管线

管线编排统一在 `src/internal/lenient_json_normalizer.rs`，领域行为委托给对应内部对象，
对外不直接暴露独立函数 API。
核心处理顺序如下：

1. `require_within_size_limit(input)`：按字节数上限拒绝过大输入。
2. `require_non_empty(input)`：按 trim 策略判定空输入。
3. `trim_if_enabled(input)`：首尾空白清理。
4. `strip_utf8_bom(input)`：可配置移除 UTF-8 BOM。
5. `trim_if_enabled(input)`：移除 BOM 后再次按需裁剪。
6. `MarkdownFence::strip_outer(input, policy)`：根据
   `markdown_fence_policy` 可配置去除外层代码块。
7. `trim_if_enabled(input)`：去除代码块后再次按需裁剪。
8. `ControlCharacterEscaper::escape(input, enabled)`：可配置转义字符串内控制字符。
9. 最终空值检查并返回 `Cow<'_, str>`。

该管线通过 `LenientJsonNormalizer::normalize()` 单一入口触发，保证顺序不变。

### 6.1 关键算法要点

- `strip_markdown_code_fence`（由 `markdown_fence_policy` 决定启用、语言范围和
  闭合要求）
  - 在管线先执行外层 trim 后，仅处理以 3 个或更多反引号或波浪线开头的输入。
  - opening fence 前只允许 0—3 个 ASCII 空格缩进；tab、非 ASCII 空白或 4 个及
    以上空格不构成 opening fence。
  - 支持语言标签和无标签两种 fence 开头。
  - JSON-only 模式按 info string 的首个空白分隔 token 判断是否为 JSON-like。
  - closing fence 前最多允许 3 个 ASCII 空格缩进；tab、非 ASCII 空白或 4 个及以上空格不构成 closing fence。
  - closing marker 后仅允许 ASCII 空格或 tab；marker 类型必须相同，且长度不得短于 opening fence。
  - 查找 closing line 时从末尾定位最后一个 LF 或 CR，支持正文和结束 fence 使用
    混合换行。
  - 不存在有效结束 fence 时，默认仍移除开头并保留剩余内容；严格模式下保持输入不变。
- `ControlCharacterEscaper::escape`
  - 通过字符串状态机识别 `in_string` 与 `in_escape`。
  - 仅处理 JSON 字符串中的 `0x00..=0x1F`。
  - 已有合法转义序列不二次转义。
  - 未配对反斜杠后紧跟原始控制字符时复用该反斜杠作为 escape 引导符；连续
    反斜杠按照奇偶配对语义处理。
- `require_non_empty`
  - 默认通过 trim 后判断空串。
  - 禁用 trim 时仅判空 `""`。

## 7. 与实现对齐性

1. `src/internal/lenient_json_normalizer.rs` 采用 `LenientJsonNormalizer` 对象模型，而非全局函数集合。
2. `lenient_json_decoder.rs` 使用单一 `normalize` 调用，避免重复逻辑。
3. `decode_object` 与 `decode_array` 通过 `JsonTopLevelKind` 做一致约束检查。
4. 错误映射路径：
   - 解析失败 -> `InvalidJson`。
   - 顶层不匹配 -> `UnexpectedTopLevel`。
   - 反序列化失败 -> `Deserialize`。
5. normalizer 和 decoder 均从同一 `JsonDecodeOptions` 读取隐私策略，并将其传入
   所有错误构造路径。

## 8. 目录结构

```text
rust-common/rs-json/
  ├─ src/
  │   ├─ lib.rs
  │   ├─ lenient_json_decoder.rs
  │   ├─ json_top_level_kind.rs
  │   ├─ error/
  │   │   ├─ mod.rs
  │   │   ├─ error_privacy_policy.rs
  │   │   ├─ json_decode_error.rs
  │   │   ├─ json_decode_error_kind.rs
  │   │   └─ json_decode_stage.rs
  │   ├─ options/
  │   │   ├─ mod.rs
  │   │   ├─ json_decode_options.rs
  │   │   ├─ markdown_fence_closing.rs
  │   │   └─ markdown_fence_policy.rs
  │   └─ internal/
  │       ├─ control_character_escaper.rs
  │       ├─ lenient_json_normalizer.rs
  │       └─ markdown_fence.rs
  ├─ tests/
  │   ├─ mod.rs
  │   ├─ lib_tests.rs
  │   ├─ fixtures/
  │   │   ├─ mod.rs
  │   │   ├─ byte_buffer.rs
  │   │   ├─ counted_failure.rs
  │   │   ├─ exact_integer.rs
  │   │   ├─ message.rs
  │   │   ├─ public_choice.rs
  │   │   ├─ single_value.rs
  │   │   ├─ user.rs
  │   │   └─ internal/
  │   │       ├─ mod.rs
  │   │       └─ byte_buffer_visitor.rs
  │   ├─ error/
  │   │   ├─ mod.rs
  │   │   ├─ error_privacy_policy_tests.rs
  │   │   ├─ json_decode_error_kind_tests.rs
  │   │   ├─ json_decode_error_tests.rs
  │   │   └─ json_decode_stage_tests.rs
  │   ├─ internal/
  │   │   ├─ mod.rs
  │   │   ├─ control_character_escaper_tests.rs
  │   │   ├─ lenient_json_normalizer_tests.rs
  │   │   └─ markdown_fence_tests.rs
  │   ├─ lenient_json_decoder_tests.rs
  │   ├─ json_top_level_kind_tests.rs
  │   └─ options/
  │       ├─ mod.rs
  │       ├─ json_decode_options_tests.rs
  │       ├─ markdown_fence_closing_tests.rs
  │       └─ markdown_fence_policy_tests.rs
  ├─ benches/
  │   ├─ decoder_bench.rs
  │   └─ internal/
  │       ├─ mod.rs
  │       └─ benchmark_record.rs
  ├─ fuzz/
  │   └─ fuzz_targets/
  │       ├─ decoder.rs
  │       └─ internal/
  │           ├─ mod.rs
  │           └─ fuzz_record.rs
  ├─ .github/
  │   └─ workflows/
  │       ├─ ci.yml
  │       └─ fuzz.yml
  └─ doc/
      ├─ json_prd.zh_CN.md
      └─ json_design.zh_CN.md
```

## 9. 测试策略

### 9.1 解码路径测试

- `tests/lenient_json_decoder_tests.rs`
  - `decode`、`decode_slice`、`decode_object`、`decode_array`、
    `decode_value` 的正常与失败路径。

### 9.2 配置与错误模型测试

- `tests/options/json_decode_options_tests.rs`：预设、getter、builder 与可清除大小限制。
- `tests/error/error_privacy_policy_tests.rs`：隐私策略默认值和类型契约。
- `tests/error/json_decode_error_tests.rs`、`tests/error/json_decode_error_kind_tests.rs`、`tests/json_top_level_kind_tests.rs`：
  - 错误种类、顶层类型映射、默认脱敏和显式详细诊断。

### 9.3 规范化测试

- `tests/internal/control_character_escaper_tests.rs`：通过公开 decoder 行为覆盖字符串状态、已有转义、全部 C0 控制字符、反斜杠奇偶语义和字符串外控制字符。
- `tests/internal/lenient_json_normalizer_tests.rs`：通过公开 decoder 行为覆盖 BOM、围栏换行、空输入诊断、trim 与控制字符修复的管线交互，不扩大内部实现可见性。
- `tests/internal/markdown_fence_tests.rs`：通过公开 decoder 行为覆盖围栏 marker、缩进、info string、换行和闭合策略。

### 9.4 性能与模糊测试

- `benches/decoder_bench.rs`：覆盖普通 JSON、围栏 JSON、原始控制字符输入，并按 1 KiB、
  64 KiB、1 MiB 评估严格字节解码与宽松类型解码，以及代表性失败路径。
- `fuzz/fuzz_targets/decoder.rs`：覆盖默认、严格、任意可选闭合围栏和仅 JSON 必须闭合围栏策略，
  对严格字节解码路径与 `serde_json` 执行接受性及结果差分检查，并验证错误 stage、原始长度和
  默认脱敏 source 等稳定不变量；`.github/workflows/fuzz.yml` 定时执行有时限的 fuzz，失败时
  上传复现产物，不进入每个 pull request 的快速检查。

## 10. 接入与发布边界

本库对外只承诺可复用的解码器对象，不约束调用方上游协议。
如需传入可能缺失的输入、重试策略、缓存或来源特异规则，建议由上层做封装。
