# `rust-json` 实现方案（`LenientJsonDecoder`）

## 版本信息

- 文档版本：`v2.0`
- 创建日期：`2026-04-10`
- 目标目录：`rust-common/rust-json/doc`
- 对齐 PRD：`rust-common/rust-json/doc/json_prd.zh_CN.md`

## 1. 背景与目标

当前 `rust-json` 的能力最初来自 LLM SDK 迁移需求，但如果继续把它设计成“若干 JSON 清洗函数”，独立成库的价值并不高。  
本轮调整后的目标是把它收敛为一个明确的公共产品：`LenientJsonDecoder`。

它的职责是：

1. 接收不完全可信的 JSON 文本输入。
2. 在解码前执行有限、温和、可配置的规范化。
3. 输出 `serde_json::Value` 或强类型结果。
4. 在顶层结构约束和错误语义上保持稳定。

它不负责提供通用 JSON 全家桶能力，也不负责“尽一切办法把坏 JSON 修好”。

## 2. 基本需求分析（Checklist + Java 现状 + 当前决策）

### 2.1 Checklist 的硬性要求

来自 `llmsdk/llmsdk-rust/rust-llmsdk-core/doc/java-porting-checklist.zh_CN.md`（4.3）：

1. 需要一层宽松 JSON 处理能力。
2. 需要覆盖：
   1. 去 Markdown 代码块包裹
   2. 修复字符串中的控制字符
   3. 非空检查
   4. 宽松 JSON 对象解码
   5. 宽松 JSON 列表解码
   6. 统一解码错误类型
3. 技术栈限定为：
   1. `serde`
   2. `serde_json`

### 2.2 Java 现有语义

Java 侧目前的核心调用链是：

1. `JsonUtils.checkNonEmpty()`
2. `JsonUtils.fixJsonOutput()`
3. `JsonObjectDecoder` / `JsonObjectListDecoder`

这说明现有需求本质上是“解码前规范化 + 解码”，而不是一套独立的工具函数产品。

### 2.3 当前架构决策

本轮明确做以下调整：

1. 公共 API 从函数中心改为对象中心。
2. 中心类型命名为 `LenientJsonDecoder`。
3. 原先函数式能力下沉为解码器内部管线或内部 helper。
4. 公共库不再暴露 LLM 专属命名。
5. 公共解码入口统一接收 `&str`，不再把 `Option<&str>` 作为产品语义。

## 3. 非目标（边界冻结）

1. 不实现另一个 `serde_json`。
2. 不自动修复缺失引号、缺失逗号、括号不匹配、单引号 JSON 等激进问题。
3. 不承担 JSON Schema 校验和业务字段校验。
4. 不将 logger、task id、provider 上下文放入公共 API。
5. 不把低层规范化 helper 作为主要产品能力对外兜售。

## 4. 总体架构

```text
input text
    |
    v
LenientJsonDecoder
    |
    |-- options
    |-- normalize_input()
    |    |-- trim_whitespace
    |    |-- strip_utf8_bom
    |    |-- strip_markdown_code_fence
    |    |-- escape_control_chars_in_strings
    |
    |-- parse_value()
    |-- enforce_top_level()
    |-- deserialize<T>()
    |
    v
serde_json / typed output
```

设计原则：

1. 对外只暴露“解码器对象 + 配置对象 + 错误模型”。
2. 对内实现保持纯函数风格，便于测试。
3. 规范化是解码的一部分，不作为与解码并列的产品中心。

## 5. 核心 API 设计

### 5.1 核心对象

```rust
#[derive(Debug, Clone)]
pub struct LenientJsonDecoder {
    options: LenientJsonDecoderOptions,
}
```

职责说明：

1. `LenientJsonDecoder`：封装一套宽松解码配置，并通过方法提供统一的 JSON 解码入口。

设计约束：

1. 对象本身无运行时状态。
2. 方法均只接收 `&self`。
3. 对象可以安全复用、复制和共享。

### 5.2 配置对象

```rust
#[derive(Debug, Clone)]
pub struct LenientJsonDecoderOptions {
    pub trim_whitespace: bool,
    pub strip_utf8_bom: bool,
    pub strip_markdown_code_fence: bool,
    pub escape_control_chars_in_strings: bool,
}
```

职责说明：

1. `LenientJsonDecoderOptions`：定义解码前规范化规则的开关集合。

默认值建议：

1. `trim_whitespace = true`
2. `strip_utf8_bom = true`
3. `strip_markdown_code_fence = true`
4. `escape_control_chars_in_strings = true`

原因：

1. 这些规则都属于“宽松但可预测”的最小集合。
2. 它们能覆盖当前 LLM 场景，也适用于 Markdown、CLI、复制粘贴文本等通用场景。

### 5.3 公开方法

```rust
impl LenientJsonDecoder {
    /// 使用给定选项创建宽松 JSON 解码器。
    pub fn new(options: LenientJsonDecoderOptions) -> Self;

    /// 返回默认宽松策略下的 JSON 解码器。
    pub fn default() -> Self;

    /// 将输入文本宽松解码为任意目标类型。
    pub fn decode<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: serde::de::DeserializeOwned;

    /// 将输入文本宽松解码为顶层 JSON 对象对应的强类型结构。
    pub fn decode_object<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: serde::de::DeserializeOwned;

    /// 将输入文本宽松解码为顶层 JSON 数组对应的强类型列表。
    pub fn decode_array<T>(&self, input: &str) -> Result<Vec<T>, JsonDecodeError>
    where
        T: serde::de::DeserializeOwned;

    /// 将输入文本宽松解码为 serde_json::Value。
    pub fn decode_value(&self, input: &str) -> Result<serde_json::Value, JsonDecodeError>;
}
```

职责说明：

1. `new()`：使用显式配置构建解码器实例。
2. `default()`：提供最常见宽松规则的默认实例。
3. `decode<T>()`：执行规范化并解码为任意目标类型，不约束顶层结构。
4. `decode_object<T>()`：要求顶层必须是 JSON object，再解码为目标类型。
5. `decode_array<T>()`：要求顶层必须是 JSON array，再解码为目标类型列表。
6. `decode_value()`：执行规范化并返回 `serde_json::Value`，适合后续动态处理。

说明：

1. `decode<T>()` 是通用入口。
2. `decode_object<T>()` 和 `decode_array<T>()` 是常见场景的语义化便捷方法。
3. 不再公开 `sanitize_llm_json()` 这类场景化自由函数。

### 5.4 错误模型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonTopLevelKind {
    Object,
    Array,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonDecodeErrorKind {
    EmptyInput,
    InvalidJson,
    UnexpectedTopLevel,
    Deserialize,
}

#[derive(Debug)]
pub struct JsonDecodeError {
    pub kind: JsonDecodeErrorKind,
    pub message: String,
    pub expected_top_level: Option<JsonTopLevelKind>,
    pub actual_top_level: Option<JsonTopLevelKind>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}
```

职责说明：

1. `JsonTopLevelKind`：表示顶层 JSON 结构类型，用于对象 / 数组约束错误。
2. `JsonDecodeErrorKind`：枚举公共解码流程中的稳定失败类别。
3. `JsonDecodeError`：携带错误种类、可读消息和必要诊断信息。

设计取舍：

1. 移除 `MissingInput`，因为公共 API 不接收 `Option<&str>`。
2. 保留 `EmptyInput`，用于表示空字符串或纯空白字符串。
3. 不在错误对象中默认持有完整原始输入。

## 6. 内部解码管线设计

### 6.1 规范化管线

内部建议流程：

1. `require_non_empty(input)`
2. `normalize_whitespace(input, options)`
3. `strip_utf8_bom(input, options)`
4. `strip_markdown_code_fence(input, options)`
5. `escape_control_chars_in_json_strings(input, options)`

说明：

1. 这些步骤是 `LenientJsonDecoder` 的内部实现细节。
2. 它们可以放在 `normalize.rs` 中，以纯函数方式实现。
3. 公共 API 不要求直接暴露这些函数。

### 6.2 `decode_value()` 流程

推荐实现流程：

1. 调用内部规范化管线，得到 `Cow<'_, str>`。
2. 使用 `serde_json::from_str::<serde_json::Value>()` 解析。
3. 若解析失败，映射为 `JsonDecodeErrorKind::InvalidJson`。
4. 保留 `serde_json` 提供的行列号信息。

### 6.3 `decode<T>()` / `decode_object<T>()` / `decode_array<T>()` 流程

#### `decode<T>()`

1. 先调用 `decode_value()`。
2. 再调用 `serde_json::from_value::<T>()`。
3. 若失败，映射为 `JsonDecodeErrorKind::Deserialize`。

#### `decode_object<T>()`

1. 先调用 `decode_value()`。
2. 判断顶层必须为 `Value::Object`。
3. 若顶层不匹配，返回 `JsonDecodeErrorKind::UnexpectedTopLevel`。
4. 顶层匹配后再执行 `serde_json::from_value::<T>()`。

#### `decode_array<T>()`

1. 先调用 `decode_value()`。
2. 判断顶层必须为 `Value::Array`。
3. 若顶层不匹配，返回 `JsonDecodeErrorKind::UnexpectedTopLevel`。
4. 顶层匹配后再执行 `serde_json::from_value::<Vec<T>>()`。

这样可以稳定区分三类问题：

1. 输入文本本身不适合解析。
2. JSON 语法本身非法。
3. JSON 合法，但顶层结构或目标类型不匹配。

## 7. 关键内部算法

### 7.1 `strip_markdown_code_fence`

职责：

1. 当输入被 `````json ... ````` 或裸 ````` ... ````` 包裹时，提取内部 JSON 文本。

实现建议：

1. 仅当文本起始位置是三反引号 fence 时触发。
2. 支持语言标签和无标签两种模式。
3. 若存在起始 fence 但缺失结束 fence，仍尽量去掉起始 fence。
4. 返回切片，尽量避免分配。

### 7.2 `escape_control_chars_in_json_strings`

职责：

1. 仅修复 JSON 字符串字面量内部的非法控制字符。

实现建议：

1. 使用单遍状态机：
   1. `in_string`
   2. `in_escape`
2. 对 `0x00..=0x1F` 的控制字符做合法 JSON 转义。
3. 已有转义序列不得被破坏。
4. 未发生修改时返回借用结果。

### 7.3 `require_non_empty`

职责：

1. 保证公共解码流程不会把空字符串误当成 JSON 语法错误。

实现建议：

1. `trim_whitespace = true` 时，先按 trim 后结果判断是否为空。
2. 若为空，返回 `JsonDecodeErrorKind::EmptyInput`。

## 8. 与 `serde_json` 的关系

直接依赖：

1. `serde::de::DeserializeOwned`
2. `serde_json::Value`
3. `serde_json::from_str`
4. `serde_json::from_value`

不做的事情：

1. 不包装 `serde_json` 的完整 API。
2. 不自己维护一套 JSON AST。
3. 不引入与 `serde_json` 平行的序列化 / 反序列化体系。

## 9. 目录与模块落盘建议

```text
rust-common/rust-json/
  ├─ Cargo.toml
  ├─ src/
  │   ├─ lib.rs
  │   ├─ decoder.rs
  │   ├─ options.rs
  │   ├─ error.rs
  │   └─ normalize.rs
  ├─ tests/
  │   ├─ decoder_tests.rs
  │   ├─ top_level_tests.rs
  │   └─ normalization_tests.rs
  └─ doc/
      ├─ json_prd.zh_CN.md
      └─ json_design.zh_CN.md
```

模块职责：

1. `decoder.rs`：实现 `LenientJsonDecoder` 及其公开方法。
2. `options.rs`：定义 `LenientJsonDecoderOptions`。
3. `error.rs`：定义 `JsonDecodeError` 相关类型。
4. `normalize.rs`：承载内部规范化 helper。

## 10. 测试方案

### 10.1 `decoder_tests.rs`

1. `decode<T>()` 解码普通结构体成功。
2. `decode_value()` 解码为 `serde_json::Value` 成功。
3. 非法 JSON 返回 `InvalidJson`。
4. 字段类型不匹配返回 `Deserialize`。

### 10.2 `top_level_tests.rs`

1. `decode_object<T>()` 在顶层对象场景成功。
2. `decode_object<T>()` 在顶层数组场景返回 `UnexpectedTopLevel`。
3. `decode_array<T>()` 在顶层数组场景成功。
4. `decode_array<T>()` 在顶层对象场景返回 `UnexpectedTopLevel`。

### 10.3 `normalization_tests.rs`

1. UTF-8 BOM 去除成功。
2. Markdown 代码块剥离成功。
3. 字符串字面量控制字符修复成功。
4. 已有转义序列不被破坏。
5. 未发生修复时返回借用结果。
6. 空输入返回 `EmptyInput`。

## 11. 分阶段实施建议

### Phase 1：对象模型落地

1. 实现 `LenientJsonDecoderOptions`。
2. 实现 `JsonDecodeError`。
3. 实现 `LenientJsonDecoder` 的四个公开方法。

### Phase 2：内部规范化落地

1. 实现 `normalize.rs` 中的 helper。
2. 将 helper 串成固定顺序管线。
3. 补齐规范化测试。

### Phase 3：接入验证

1. 在 `rust-llmsdk-core` 结构化输出链中接入 `LenientJsonDecoder`。
2. 验证其在非 LLM 文本来源中的复用性。
3. 如果需要更复杂构造方式，再补 `Builder`。

## 12. 风险与应对

1. 风险：对象化后仍然泄露过多底层 helper，导致 API 再次发散。
   - 应对：仅承诺对象方法和配置对象，内部 helper 不作为核心产品能力。
2. 风险：宽松规则增多，行为不可预测。
   - 应对：通过 `Options` 显式开关控制，首版仅支持四项温和规则。
3. 风险：不同场景对代码块策略诉求不同。
   - 应对：首版提供最小可用策略，复杂策略放到 M2。
4. 风险：上层仍希望传 `Option<&str>`。
   - 应对：由上层包装器负责缺失输入语义，公共库只处理“给定字符串如何解码”。
