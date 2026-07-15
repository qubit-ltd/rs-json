# rs-json 隐私策略与正确性修复设计

## 背景

`qubit-json` 负责把非完全可信的文本规范化并反序列化为 Rust 类型。当前实现的
整体边界保持不变，但存在以下问题：

1. `JsonDecodeError` 把完整 `serde_json::Error` 写入 `message` 并保留为标准
   `source`，类型不匹配时可能把输入标量暴露到普通日志。
2. JSON 字符串中，未配对反斜杠后紧跟的原始 ASCII 控制字符不会被修复。
3. Markdown 正文和 closing fence 使用混合换行时，结束 fence 可能无法识别。
4. 输入在完整规范化后变为空时，错误没有报告 `Some(0)` 的规范化长度。
5. `JsonDecodeOptions` 字段公开，新增选项会破坏外部 struct literal；现有
   builder 也无法完整配置或清除所有选项。

本次允许破坏公开 API，不保留旧字段访问、struct literal 或旧 builder 签名。
按照 Cargo 的 `0.x` 兼容性约定，crate 版本从 `0.3.6` 提升到 `0.4.0`。

## 目标

- 提供默认安全、可显式切换为详细诊断的错误隐私策略。
- 修复三个已确认的规范化和诊断缺陷，并为每个缺陷增加回归测试。
- 将 `JsonDecodeOptions` 重构为私有字段、公开 getter 和完整值式 builder。
- 保持现有解码边界、错误分类、顶层约束和直接 serde 反序列化语义。
- 更新直接下游 `rs-llmsdk-core` 以使用新 Options API。

## 非目标

- 不优化对象和数组成功路径的双遍解析。
- 不改变 Markdown fence 的语言标签策略或 closing-required 语义。
- 不增加从 surrounding prose 中搜索 JSON 子串的恢复能力。
- 不修复 OpenAI provider 锁文件、decoder factory hook 或 entity retry 策略。
- 不引入 `qubit-sanitize` 或其他新运行时依赖。

## 方案选择

采用“错误构造时执行隐私策略”的方案。它保证 `Redacted` 模式下敏感数据从未
进入错误对象，因此标准 `Display`、派生 `Debug`、`message()` 和标准错误链都
不会意外泄露输入内容。

不采用以下方案：

- 仅在 `Display` 时脱敏：完整 source 仍会通过 `Debug` 或错误链泄露。
- 下游包装安全错误：要求每个调用者主动处理，容易漏用且增加 SDK 集成成本。

## 公开 API

### `ErrorPrivacyPolicy`

新增独立公开类型：

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorPrivacyPolicy {
    #[default]
    Redacted,
    Detailed,
}
```

- `Redacted` 是默认策略。
- `Detailed` 显式保留当前完整 serde 诊断能力。
- 类型从 crate 根模块重导出，并遵循一文件一个公开类型的现有布局。

### `JsonDecodeOptions`

所有字段改为私有，并新增 `error_privacy_policy`：

```rust
pub struct JsonDecodeOptions {
    trim_whitespace: bool,
    strip_utf8_bom: bool,
    markdown_fence_policy: MarkdownFencePolicy,
    escape_control_chars_in_strings: bool,
    max_input_bytes: Option<usize>,
    error_privacy_policy: ErrorPrivacyPolicy,
}
```

公开 getter：

```rust
pub const fn trim_whitespace(&self) -> bool;
pub const fn strip_utf8_bom(&self) -> bool;
pub const fn markdown_fence_policy(&self) -> MarkdownFencePolicy;
pub const fn escape_control_chars_in_strings(&self) -> bool;
pub const fn max_input_bytes(&self) -> Option<usize>;
pub const fn error_privacy_policy(&self) -> ErrorPrivacyPolicy;
```

公开值式 builder：

```rust
pub const fn with_trim_whitespace(self, enabled: bool) -> Self;
pub const fn with_strip_utf8_bom(self, enabled: bool) -> Self;
pub const fn with_markdown_fence_policy(self, policy: MarkdownFencePolicy) -> Self;
pub const fn with_escape_control_chars_in_strings(self, enabled: bool) -> Self;
pub const fn with_max_input_bytes(self, limit: Option<usize>) -> Self;
pub const fn with_error_privacy_policy(self, policy: ErrorPrivacyPolicy) -> Self;
```

`lenient()`、`strict()`、`json_code_fences_only()` 和 `Default` 均使用
`ErrorPrivacyPolicy::Redacted`。隐私策略独立于规范化严格程度；选择 `strict()`
不会自动开启详细错误。

### `JsonDecodeError`

错误新增私有 `privacy_policy` 字段及 getter：

```rust
pub const fn privacy_policy(&self) -> ErrorPrivacyPolicy;
```

所有错误种类都记录生效策略，以便错误脱离 decoder 后仍可审计其诊断语义。
该字段参与稳定字段的 `PartialEq` 比较。

## 隐私行为

### `Redacted`

对 `InvalidJson` 和 `Deserialize`：

- `message()` 只包含稳定前缀和可用的规范化后行列位置。
- 不格式化或解析 `serde_json::Error` 的文本。
- 不在错误对象中保留 `serde_json::Error`；`Error::source()` 返回 `None`。
- `Display` 使用安全 `message()`。
- 派生 `Debug` 只包含安全消息、结构化字段和 `source: None`。

建议消息形式：

```text
Failed to parse JSON at normalized line 2 column 7
Failed to deserialize JSON value at normalized line 1 column 12
```

若行列不可用，只保留前缀。结构化 `normalized_line()` 和
`normalized_column()` 仍是定位信息的权威来源。

规范化、大小限制和顶层类型错误本来不包含输入内容，保持现有消息语义，但同样
记录 `Redacted` 策略。

### `Detailed`

对 `InvalidJson` 和 `Deserialize`：

- `message()` 保持 `{prefix}: {serde_error}` 形式。
- 保存 `Arc<serde_json::Error>`。
- `Display`、`Debug` 和标准错误链允许包含 serde 提供的输入值片段。

调用者必须显式通过 `with_error_privacy_policy(ErrorPrivacyPolicy::Detailed)`
启用该行为。

## 控制字符修复

状态机继续区分字符串外、字符串内和反斜杠 escape 状态。修复后的规则为：

- 字符串内、非 escape 状态遇到原始 C0：写入完整 JSON escape，例如 LF 写为
  `\\n`、NUL 写为 `\\u0000`。
- 字符串内、escape 状态遇到原始 C0：复用已经存在的反斜杠，只写 escape
  后缀，例如 LF 写为 `n`、NUL 写为 `u0000`。
- escape 状态遇到非 C0：保持现有行为，不尝试修复其他非法 escape。
- 偶数个连续反斜杠已经成对；其后的 C0 使用完整 escape。
- 奇数个连续反斜杠留下一个 escape 引导符；其后的 C0 只写 escape 后缀。

计数遍和改写遍必须使用相同判定规则，避免零计数导致错误返回借用输入。

## Markdown closing fence 修复

`strip_markdown_closing_fence` 应分别查找最后一个 LF 和最后一个 CR，并选择两者
中索引较大的位置作为 closing line 的前一换行。这样可正确处理：

- 纯 LF；
- 纯 CR；
- CRLF；
- 正文使用 LF、closing fence 前使用 CR；
- 正文使用 CR、closing fence 前使用 LF。

其余 marker、长度、缩进和独占行规则不变。

## 空输入诊断修复

`EmptyInput` 构造器接受 `normalized_input_bytes: Option<usize>`：

- 原始空字符串或在管线开始检查时判定为空白：传 `None`，表示规范化未完成。
- BOM、Markdown fence 或最终 trim 等完整管线处理后变为空：传
  `Some(input.len())`，当前结果为 `Some(0)`。

`raw_input_bytes()` 始终保持原始字节数。

## 数据流

1. `LenientJsonDecoder` 持有不可变 `JsonDecodeOptions`。
2. normalizer 从 Options getter 读取规范化和隐私配置。
3. normalizer 创建的错误携带有效隐私策略。
4. parser/deserialize helper 接收有效隐私策略，先提取安全的行列，再由策略决定
   是否格式化和保留 serde error。
5. 错误通过现有具体 `JsonDecodeError` API 返回；错误 kind 和 stage 不变。

## 测试设计

所有行为遵循失败测试先行。

### 隐私策略

- 默认 preset 和 `Default` 均为 `Redacted`。
- `Redacted` 类型不匹配错误的 `message`、`Display`、`Debug` 均不包含秘密标量。
- `Redacted` 解析错误不保留 source，但保留规范化行列。
- `Detailed` 类型不匹配错误的消息和 source 保留 serde 明细。
- `Detailed` 解析错误保留 source。
- 所有错误 kind 的 `privacy_policy()` 返回创建时策略。
- `PartialEq` 能区分不同隐私策略。

### Options 重构

- 三个 preset 的全部 getter 值符合契约。
- 每个 builder 只改变目标字段。
- `with_max_input_bytes(Some(limit))` 设置限制。
- `with_max_input_bytes(None)` 清除限制。
- `JsonDecodeOptions` 继续满足 `Copy + Eq`。

### 控制字符

- 遍历 `U+0000..=U+001F`，覆盖单个未配对反斜杠后的全部 C0。
- 覆盖偶数和奇数连续反斜杠后的 LF 与 NUL。
- 继续验证已有合法 escape 不被二次转义。
- 禁用控制字符修复时保持拒绝行为。

### closing fence

- 新增正文 LF、closing 前 CR 的最小回归用例。
- 新增正文 CR、closing 前 LF 的对称用例。
- 保留并运行现有 LF、CRLF、marker 和 closing-required 测试。

### 空输入诊断

- 原始空和初始空白输入返回 `None`。
- BOM-only 返回 `Some(0)`。
- 空 fenced body 返回 `Some(0)`。

### 下游

- 更新 `rs-llmsdk-core` 的精确依赖为 `qubit-json = "=0.4.0"`，并改用
  `with_max_input_bytes(Some(limit))`。
- 验证默认 `EntityDecoder` 仍使用 1 MiB 限制和 `Redacted` 策略。
- 运行现有 entity decoder 测试。

## 文档与发布面

- crate 根 rustdoc 和 README 中说明默认错误是可安全记录的脱敏诊断。
- 明确 `Detailed` 可能暴露输入值，只能在受控环境显式开启。
- 更新 Options API 示例，不再使用公开字段或旧 size builder。
- 同步中英文 README、PRD 和设计文档的 Options 与错误模型。
- 为默认解码和隐私策略增加可编译 rustdoc 示例。
- 将 `qubit-json` 包版本和对应锁文件更新到 `0.4.0`；不更新不在本次范围内的
  OpenAI provider 锁文件。

## 验收标准

- 新增测试在旧实现上分别因目标缺陷失败。
- 修复后 `rs-json` 全部测试、Clippy、rustdoc、项目 style check 通过。
- 覆盖率不低于修改前的 98.46% 行覆盖率，新增公开类型和 getter/builder 有测试。
- `rs-llmsdk-core` 相关 entity decoder 测试通过。
- `Redacted` 的标准错误表面不包含测试秘密值，`Detailed` 明确保留该值。
- 不包含性能路径重构及列明的其他下游修复。
