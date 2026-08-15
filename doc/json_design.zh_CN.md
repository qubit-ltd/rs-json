# `rs-json` 通用 JSON 基础设施实现方案

## 版本信息

- 文档版本：`v4.0`
- 创建日期：`2026-04-12`
- 更新日期：`2026-08-14`
- 对齐 PRD：`json_prd.zh_CN.md`

## 1. 背景与目标

`rs-json` 定位为通用 JSON 基础设施，不依赖业务场景。它把宽松文本入口、严格文本
编解码、带预算的 value 构造和非递归 tree 处理组织在同一 crate 中，并复用
`qubit-budget` 定义的 JSON 资源和 session。

目标行为是：

1. `lenient` 对输入文本执行有限、可预测、可配置的规范化，并支持调用方持有的累计
   decode session。
2. `text` 提供严格、带预算的 decode/encode adapter；`value` 提供预算感知的
   `serde_json::Value` seed；`tree` 提供迭代遍历与修改。
3. 规范化后先做非递归词法预算准入，再直接反序列化目标 `T`，不以中间
   `serde_json::Value` 改写 Serde 语义。
4. 用稳定错误分类、阶段和结构化 budget error 统一上游分支。

## 2. 核心边界

### 非目标

1. 不代替 `serde_json` 的数据模型、Serde 实现或完整工具链。
2. 不做激进修复（补引号、补逗号、括号匹配、引号风格转换等）。
3. 不承诺失败回滚：decode session 采用累计记账，`process_mut` 采用递增修改。
4. 不引入 `Option<&str>` 作为公共解码语义，也不引入 runtime、provider 或网络依赖。

## 3. 总体架构

```text
                           qubit_budget::json sessions
                                      |
         +----------------------------+---------------------------+
         |                            |                           |
         v                            v                           v
      lenient                        text                    value / tree
  normalize text             strict decode/encode       seed / iterative walk
         |
         +-- decode() -------------------------------> direct Serde T
         |
         `-- decode_with_session()
                | raw input charge
                | normalized input charge
                | lexical value admission
                `------------------------------------> direct Serde T
```

设计原则：

1. 四个公开能力域边界固定为 `lenient`、`text`、`value`、`tree`。
2. 宽松入口以对象 API 为中心；严格入口使用显式 session 和函数 API。
3. 规范化作为解码内部阶段，保持对象边界稳定。
4. 内部组件按职责拆分：`internal/lenient_json_normalizer.rs` 承载预处理策略，
   `internal/markdown_fence.rs` 负责 Markdown 围栏识别与剥离，
   `internal/control_character_escaper.rs` 先以 C0 快速预检跳过不含控制字符的输入；命中后
   进行状态扫描，并仅在首次替换时惰性分配处理字符串内 C0 控制字符。

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
3. 对 `&self` 调用提供可复用、可克隆的行为。

### 4.2 `LenientJsonNormalizer`（内部对象）

`LenientJsonNormalizer` 是内部对象，`lenient_json_decoder.rs` 中通过实例调用其
`normalize()`。

```rust
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonDecodeOptions {
    trim_whitespace: bool,
    strip_utf8_bom: bool,
    markdown_fence_policy: MarkdownFencePolicy,
    escape_control_chars_in_strings: bool,
    max_input_bytes: Option<usize>,
    max_normalized_bytes: Option<usize>,
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
- `with_max_normalized_bytes(Some(limit))` 设置规范化后 JSON 上限，
  `with_max_normalized_bytes(None)` 清除上限。
- `with_error_privacy_policy(policy)`。

默认值：

- `trim_whitespace = true`
- `strip_utf8_bom = true`
- `markdown_fence_policy = JsonOnly { closing: Optional }`
- `escape_control_chars_in_strings = true`
- `max_input_bytes = None`
- `max_normalized_bytes = None`
- `error_privacy_policy = ErrorPrivacyPolicy::Redacted`

### 4.4 错误模型

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonTopLevelKind { Object, Array, Other }

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonDecodeErrorKind {
    Budget,
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
2. `stage` 用于标识失败发生在字节转文本、规范化、预算准入、解析、顶层检查或
   反序列化阶段；value 预算拒绝稳定表示为 `Budget`/`Admission`。
3. `normalized_line()`/`normalized_column()` 用于解析和反序列化阶段定位，
   坐标相对于规范化后的 JSON 文本，无法定位时保持 `None`。
4. `expected_top_level`/`actual_top_level` 仅用于 `UnexpectedTopLevel`。
5. `raw_input_bytes()`、`normalized_input_bytes()`、`max_input_bytes()` 与
   `max_normalized_bytes()` 用于输入大小限制和解析诊断。
6. `privacy_policy()` 记录错误构造时实际生效的诊断策略，并参与稳定字段相等性
   比较。
7. 默认 `Redacted` 在错误构造时只保留稳定前缀和规范化后行列，不格式化或保存
   `serde_json::Error`；因此 `message`、`Display`、`Debug` 和标准 error source
   均不含 serde 提供的输入派生内容。
8. 显式 `Detailed` 保留 `{prefix}: {serde_error}` 消息及底层 source，可能暴露
   输入值，只适用于受控诊断环境。
9. 规范化和顶层类型检查错误本身不含输入内容，但同样记录生效隐私策略。
10. `measured_budget_error()` 只在 `Budget` 错误上返回完整
    `MeasuredBudgetError<JsonResource, usize>`；其它错误返回 `None`。

## 5. 公开 API 设计

### 5.1 `LenientJsonDecoder` 方法

```rust
impl LenientJsonDecoder {
    pub const fn new(options: JsonDecodeOptions) -> Self;
    pub const fn options(&self) -> &JsonDecodeOptions;

    pub fn decode<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: serde::de::DeserializeOwned;

    pub fn decode_with_session<T>(
        &self,
        input: &str,
        session: &mut JsonDecodeSession<'_, JsonResource>,
    ) -> Result<T, JsonDecodeError>
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

- `decode<T>()`：不限定顶层结构，规范化后直接反序列化为 `T`；为保留快速路径，不做
  value 词法预检。
- `decode_with_session<T>()`：使用调用方 session 依次累计 raw input、normalized input
  和 value 三阶段预算。value 阶段由 `JsonLexicalPreflight` 直接扫描规范化字节，准入
  后仍直接反序列化为 `T`。任何成功消费在后续错误发生时均不回滚。
- `decode_slice<T>()`：先按原始字节检查上限，再完整校验 UTF-8，借用为
  `&str` 后复用 `decode<T>()`；校验发生在目标类型反序列化之前，任何选项组合
  都不得绕过，有效输入不复制。
- `decode_object<T>()`：先检查首个 JSON token。若 token 为对象，直接从规范化文本
  反序列化为 `T`；若 token 不匹配，再借助 `RawValue` 验证完整语法，以区分
  `InvalidJson` 与 `UnexpectedTopLevel`。
- `decode_array<T>()`：先检查首个 JSON token。若 token 为数组，直接从规范化文本
  反序列化为 `Vec<T>`；若 token 不匹配，再借助 `RawValue` 验证完整语法。
- `decode_value()`：先规范化再直接解析为 `serde_json::Value`。

### 5.3 三阶段预算准入

`decode_with_session()` 的顺序是：

1. normalizer 在处理前累计 raw input bytes；
2. 完成所有规范化规则后累计 normalized input bytes；
3. `JsonLexicalPreflight` 非递归扫描规范化 JSON，按解码后的键/字符串字节、数字词法
   字节、节点、单容器成员数、深度和共享 payload 进行 value admission；
4. 准入成功后，以 `serde_json` 直接反序列化目标 `T`。

词法预检和直接反序列化有意形成双扫描。这样既能在构造目标值前拒绝超限输入，又不会用
`serde_json::Value` 中转破坏重复字段检测、`u128` 精确数值或目标类型自定义
`Deserialize` 行为。session 是调用方持有的单调账本：失败只保证“本次被拒绝增量不写入”，
不会撤销更早的成功消费。

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
8. `require_within_normalized_size_limit(input, raw_input_bytes)`：按配置的
   `max_normalized_bytes` 预计算控制字符修复后的字节数和是否需要修复，在分配修复
   文本前拒绝超限输入，并将扫描结果交给转义阶段复用。
9. `ControlCharacterEscaper::escape(input, enabled)`：可配置转义字符串内控制字符；
   配置了规范化大小限制时，通过 `escape_with_scan` 复用上一步扫描结果并精确预分配。
10. 最终空值检查并返回 `Cow<'_, str>`。

该管线通过 `LenientJsonNormalizer::normalize()` 单一入口触发，保证顺序不变。

### 6.1 关键算法要点

- `strip_markdown_code_fence`（由 `markdown_fence_policy` 决定启用、语言范围和
  闭合要求）
  - 启用 trim 时，先移除整个输入的外层空白，再识别以 3 个或更多反引号或波浪线
    开头的输入；因此 opening fence 原始行首的任意深度空白会先被移除。
  - 禁用 trim 时，opening fence 前只允许 0—3 个 ASCII 空格缩进；tab、非 ASCII
    空白或 4 个及以上空格不构成 opening fence。
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

## 7. `text`、`value` 与 `tree` 契约

### 7.1 严格文本

`text` 使用 `JsonDecodeSession` 和 `JsonEncodeSession` 提供严格编解码。decode 不做
宽松规范化；encode 对 output 采用暂存后提交，因此 Serde 失败不会消费 output budget，
但此前发生的 value 计费可保留。原始 JSON 片段无效时，
`JsonEncodeError::InvalidRawJson(JsonSyntaxError)` 直接保留 reason、offset、line、
column；下游不需要从重建的 `serde_json::Error` 文本中解析诊断。

### 7.2 Value 构造

`value::AccountingJsonValueSeed` 是预算感知 `serde_json::Value` seed 的唯一公开路径。
实现仍位于私有 budget 子模块，`budget` 和 `tree` 不再提供兼容 alias。该 seed 用于只能
观察解码后 Serde 事件、无法访问原始 JSON 字节的调用链。

### 7.3 Tree 遍历

`JsonTreeReader::process<'value, V>` 将输入 value 生命周期与 processor 持有的 budget
生命周期分离。`process_mut` 是非事务操作：visitor mutation 和 budget consumption
按遍历进度立即生效；若中途返回 visitor 或 budget 错误，已完成部分不会回滚。
`RootRestoreGuard` 只负责把暂时拆出的子树重新组装成结构有效的 root，不恢复原始内容。

## 8. `serde_json` 私有协议兼容边界

预算 serializer 需要识别 `serde_json` arbitrary-precision Number 和 RawValue 使用的
私有 struct name。生产代码仅允许
`src/budget/internal/serde_json_compat.rs` 持有并分类这些 token，依赖必须精确固定为
`serde_json = "=1.0.151"`。

升级清单：

1. 修改 `Cargo.toml` 中的精确版本，并分别更新根目录与 `fuzz/Cargo.lock`。
2. 阅读上游 Number、RawValue serializer 的私有名称和 payload 形状，更新 compat module。
3. 确认生产代码没有在 compat module 外直接比较 token。
4. 运行 `json_private`、`json_encode_serializer`、真实 Number/RawValue、伪造近似名称和
   `collect_str` 回归。
5. 运行 root/fuzz 两棵 `cargo tree -i serde_json`，确认版本唯一；再运行 fuzz workspace
   check 与项目完整质量门禁。

## 9. 与实现对齐性

1. `src/internal/lenient_json_normalizer.rs` 采用 `LenientJsonNormalizer` 对象模型，而非全局函数集合。
2. `lenient_json_decoder.rs` 的普通路径使用单一 `normalize` 调用；session 路径只在
   normalize 与直接反序列化之间插入 lexical admission。
3. `decode_object` 与 `decode_array` 通过 `JsonTopLevelKind` 做一致约束检查。
4. 错误映射路径：
   - value 资源拒绝 -> `Budget` / `Admission`。
   - 解析失败 -> `InvalidJson`。
   - 顶层不匹配 -> `UnexpectedTopLevel`。
   - 反序列化失败 -> `Deserialize`。
5. normalizer 和 decoder 均从同一 `JsonDecodeOptions` 读取隐私策略，并将其传入
   所有错误构造路径。
6. crate root 只公开 `lenient`、`text`、`value`、`tree` 四个能力域；资源类型由
   `qubit_budget::json` 的所有者 crate 直接提供。

## 10. 目录结构

```text
rust-common/rs-json/
  ├─ src/
  │   ├─ lib.rs
  │   ├─ budget/
  │   │   ├─ budgeted_json_value_seed.rs
  │   │   └─ internal/
  │   │       ├─ json_lexical_preflight.rs
  │   │       ├─ json_encode_serializer.rs
  │   │       └─ serde_json_compat.rs
  │   ├─ lenient_json_decoder.rs
  │   ├─ json_top_level_kind.rs
  │   ├─ error/
  │   │   ├─ mod.rs
  │   │   ├─ error_privacy_policy.rs
  │   │   ├─ internal/
  │   │   │   ├─ mod.rs
  │   │   │   └─ json_input_size_limit.rs
  │   │   ├─ json_decode_error.rs
  │   │   ├─ json_decode_error_kind.rs
  │   │   └─ json_decode_stage.rs
  │   ├─ options/
  │   │   ├─ mod.rs
  │   │   ├─ json_decode_options.rs
  │   │   ├─ markdown_fence_closing.rs
  │   │   └─ markdown_fence_policy.rs
  │   ├─ text/
  │   │   ├─ json_decode.rs
  │   │   ├─ json_deserialize_error.rs
  │   │   ├─ json_deserialize_error_category.rs
  │   │   ├─ json_encode.rs
  │   │   └─ json_encode_error.rs
  │   ├─ tree/
  │   │   ├─ json_tree_processor.rs
  │   │   ├─ json_tree_visitor.rs
  │   │   └─ json_tree_mut_visitor.rs
  │   ├─ value/
  │   │   └─ mod.rs
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
  │   │   ├─ internal/
  │   │   │   ├─ mod.rs
  │   │   │   └─ json_input_size_limit_tests.rs
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
  │   ├─ budgeted_serde_json.rs
  │   └─ internal/
  │       ├─ mod.rs
  │       └─ benchmark_record.rs
  ├─ fuzz/
  │   ├─ Cargo.lock
  │   └─ fuzz_targets/
  │       ├─ decoder.rs
  │       ├─ json_budget_invariants.rs
  │       ├─ json_decode_differential.rs
  │       ├─ json_encode_invariants.rs
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

## 11. 测试策略

### 11.1 解码路径测试

- `tests/lenient_json_decoder_tests.rs`
  - `decode`、`decode_slice`、`decode_object`、`decode_array`、
    `decode_value`、`decode_with_session` 的正常与失败路径，包括三阶段累计计费、预算拒绝
    不回滚、serde 语法位置和孤立 surrogate 不 panic。

### 11.2 配置与错误模型测试

- `tests/options/json_decode_options_tests.rs`：预设、getter、builder 与可清除大小限制。
- `tests/error/error_privacy_policy_tests.rs`：隐私策略默认值和类型契约。
- `tests/error/json_decode_error_tests.rs`、`tests/error/json_decode_error_kind_tests.rs`、`tests/json_top_level_kind_tests.rs`：
  - 错误种类、顶层类型映射、默认脱敏和显式详细诊断。

### 11.3 规范化测试

- `tests/internal/control_character_escaper_tests.rs`：通过公开 decoder 行为覆盖字符串状态、已有转义、全部 C0 控制字符、反斜杠奇偶语义和字符串外控制字符。
- `tests/internal/lenient_json_normalizer_tests.rs`：通过公开 decoder 行为覆盖 BOM、围栏换行、空输入诊断、trim 与控制字符修复的管线交互，不扩大内部实现可见性。
- `tests/internal/markdown_fence_tests.rs`：通过公开 decoder 行为覆盖围栏 marker、缩进、info string、换行和闭合策略。

### 11.4 Text、value、tree 与兼容测试

- `tests/text/**`：严格编解码与稳定语法错误载荷。
- `tests/budget/budgeted_json_value_seed_tests.rs`：唯一公开 value seed 路径与递增拒绝。
- `tests/tree/**`：短 value 生命周期、深度优先遍历和非事务 mutation/budget 契约。
- `tests/budget/internal/json_private_tests.rs`：精确私有 token、near-miss、真实
  Number/RawValue 和 payload 分类。

### 11.5 性能与模糊测试

- `benches/decoder_bench.rs`：覆盖普通 JSON、围栏 JSON、原始控制字符输入，并按 1 KiB、
  64 KiB、1 MiB 评估严格字节解码与宽松类型解码，以及代表性失败路径。
- `benches/budgeted_serde_json.rs`：按相同三种规模比较 strict、lenient、owned/borrowed/
  reused session 的 decode/encode 路径。
- `fuzz/fuzz_targets/decoder.rs`：覆盖默认、严格、任意可选闭合围栏和仅 JSON 必须闭合围栏策略，
  对严格字节解码路径与 `serde_json` 执行接受性及结果差分检查，并验证错误 stage、原始长度和
  默认脱敏 source 等稳定不变量；`.github/workflows/fuzz.yml` 定时执行有时限的 fuzz，失败时
  上传复现产物，不进入每个 pull request 的快速检查。

## 12. 接入与发布边界

本库对外只承诺可复用的解码器对象，不约束调用方上游协议。
如需传入可能缺失的输入、重试策略、缓存或来源特异规则，建议由上层做封装。
