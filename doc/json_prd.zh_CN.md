# `rust-json` 产品需求文档（PRD：`LenientJsonDecoder`）

## 文档信息

- 文档名称：`rust-json` 产品需求文档（PRD：`LenientJsonDecoder`）
- 文档版本：`v2.0`
- 创建日期：`2026-04-10`
- 状态：`Draft`
- 对齐设计文档：`rust-common/rust-json/doc/json_design.zh_CN.md`
- 需求来源：`llmsdk/llmsdk-rust/rust-llmsdk-core/doc/java-porting-checklist.zh_CN.md`（4.3）
- 定位修正：从“LLM 专用 JSON 工具函数集合”调整为“通用宽松 JSON 解码器”
- 参考实现：
  1. `llmsdk/llmsdk-java/llmsdk-core/src/main/java/ltd/qubit/ai/llm/util/JsonUtils.java`
  2. `llmsdk/llmsdk-java/llmsdk-core/src/main/java/ltd/qubit/ai/llm/engine/chat/JsonChatResponseDecoder.java`
  3. `llmsdk/llmsdk-java/llmsdk-core/src/main/java/ltd/qubit/ai/llm/engine/chat/JsonChatResponseListDecoder.java`

## 1. 背景

迁移清单要求 Rust 侧补齐一层“宽松 JSON 处理”能力，因为标准 JSON 框架只解决“输入是合法 JSON”的情况，而真实工程里经常面对的是“不完全可信的 JSON 文本”：

1. LLM 结构化输出会被 Markdown 代码块包裹。
2. 复制粘贴、CLI 输出、模板渲染结果里可能混入 UTF-8 BOM 或首尾杂质。
3. JSON 字符串字面量里可能出现未转义控制字符。
4. 上层调用方经常需要的不只是 `serde_json::from_str()`，而是“一套带温和预处理、稳定错误语义、可复用配置的解码器”。

因此，`rust-json` 不应继续被设计成一组零散工具函数，而应被设计成一个面向更多场景的对象化宽松解码器：`LenientJsonDecoder`。

## 2. 产品定义

`rust-json` 的核心产品是 `LenientJsonDecoder`。

它是一个可配置、可复用、面向不完全可信 JSON 文本输入的解码对象，负责：

1. 在解码前执行有限且可控的输入规范化。
2. 将规范化后的文本解析为 `serde_json::Value` 或强类型结果。
3. 对顶层 JSON 结构进行稳定约束检查。
4. 提供一致的错误模型，便于上层定位失败原因。

它不负责成为另一个 JSON 框架，也不负责激进修复任意格式错误。

## 3. 目标与非目标

### 3.1 产品目标

1. 提供以 `LenientJsonDecoder` 为中心的对象化 API，而不是散函数 API。
2. 提供配置对象 `LenientJsonDecoderOptions`，把宽松解码策略固化为实例属性。
3. 提供四个高频公开方法：
   1. `decode<T>()`
   2. `decode_object<T>()`
   3. `decode_array<T>()`
   4. `decode_value()`
4. 预处理步骤仅覆盖温和且高频的规范化能力：
   1. 去首尾空白
   2. 去 UTF-8 BOM
   3. 去 Markdown 代码块包装
   4. 修复 JSON 字符串字面量内的控制字符
5. 提供统一错误模型 `JsonDecodeError`，可稳定区分空输入、JSON 语法错误、顶层结构不匹配、反序列化失败。
6. 保持纯函数式实现内核，不依赖 logger、runtime、provider 或全局状态。

### 3.2 非目标

1. 不替代 `serde` / `serde_json`。
2. 不做“自动补引号、补逗号、补括号、单引号转双引号”这类激进修复。
3. 不承担 JSON Schema 校验、业务字段校验或领域语义校验。
4. 不在公共 API 中保留 LLM 专属命名，例如 `sanitize_llm_json()`。
5. 不将“输入缺失 `None`”纳入公共 API 语义，公共解码入口统一接收 `&str`。

## 4. 目标用户与使用场景

### 4.1 目标用户

1. `rust-llmsdk-core` 结构化输出维护者。
2. provider、CLI、配置加载、模板渲染等需要处理“非完全可信 JSON 文本”的开发者。
3. 需要在多个调用点复用同一套解码规则的基础库开发者。

### 4.2 核心场景

1. 解码被 `````json ... ````` 包裹的 JSON 文本。
2. 解码含有 BOM、首尾空白或字符串内控制字符的 JSON 文本。
3. 将文本解码为强类型对象。
4. 将文本解码为强类型数组。
5. 将文本解码为 `serde_json::Value` 以供后续动态处理。
6. 在同一业务组件内复用一套稳定的宽松解码配置。

## 5. 范围（Release Scope）

### 5.1 MVP（必须）

1. `LenientJsonDecoder`：封装宽松解码配置和解码方法的核心对象。
2. `LenientJsonDecoderOptions`：声明宽松解码规则的配置对象。
3. `LenientJsonDecoder::default()`：提供默认宽松策略实例。
4. `LenientJsonDecoder::new(options)`：使用显式配置创建解码器实例。
5. `LenientJsonDecoder::decode<T>()`：将文本解码为任意目标类型。
6. `LenientJsonDecoder::decode_object<T>()`：要求顶层 JSON 为对象并解码为目标类型。
7. `LenientJsonDecoder::decode_array<T>()`：要求顶层 JSON 为数组并解码为目标类型列表。
8. `LenientJsonDecoder::decode_value()`：将文本解码为 `serde_json::Value`。
9. `JsonDecodeError`：统一表示解码失败原因。
10. 内部规范化管线：空输入检查、去 BOM、去代码块、控制字符修复。
11. 覆盖上述路径的单元测试和文档。

### 5.2 增强项（M2）

1. `LenientJsonDecoderBuilder`：面向多选项构造场景的 builder。
2. 更细粒度的错误阶段信息。
3. 更灵活的代码块提取策略，例如大小写、标签白名单、自定义 fence 识别。
4. 低层调试接口，例如暴露“规范化后的文本”，但不作为首版中心能力。

## 6. 需求列表（含验收标准）

### PRD-LJD-001：对象化解码器

- 需求描述：库的公共 API 必须以 `LenientJsonDecoder` 为中心，而不是零散自由函数。
- 优先级：`P0`
- 验收标准：
  1. 调用方可以持有一个解码器实例并重复使用。
  2. 实例通过属性持有配置，不要求每次解码都显式传 `options`。
  3. 实例方法均为只读方法，不依赖内部可变状态。

### PRD-LJD-002：配置对象

- 需求描述：提供 `LenientJsonDecoderOptions`，用于配置宽松解码行为。
- 优先级：`P0`
- 验收标准：
  1. 至少支持以下选项：
     1. `trim_whitespace`
     2. `strip_utf8_bom`
     3. `strip_markdown_code_fence`
     4. `escape_control_chars_in_strings`
  2. 提供默认值。
  3. 默认值应覆盖最常见“宽松但不过度修复”的场景。

### PRD-LJD-003：统一规范化管线

- 需求描述：解码器在正式解析 JSON 前，应自动执行一套固定顺序的轻量规范化流程。
- 优先级：`P0`
- 验收标准：
  1. 规范化顺序明确且文档化。
  2. 默认行为只做温和修复，不做猜测式重写。
  3. 未发生修改时尽量避免不必要分配。

### PRD-LJD-004：通用解码入口

- 需求描述：提供 `decode<T>()`，用于将规范化后的 JSON 文本解码为任意目标类型。
- 优先级：`P0`
- 验收标准：
  1. 对调用方隐藏底层规范化过程。
  2. JSON 语法错误和目标类型反序列化失败可被区分。
  3. 不对顶层结构做额外约束。

### PRD-LJD-005：对象顶层约束解码

- 需求描述：提供 `decode_object<T>()`，用于强制要求顶层为 JSON object。
- 优先级：`P0`
- 验收标准：
  1. 顶层不是对象时返回明确错误。
  2. 顶层是对象但字段不匹配时返回反序列化错误。

### PRD-LJD-006：数组顶层约束解码

- 需求描述：提供 `decode_array<T>()`，用于强制要求顶层为 JSON array。
- 优先级：`P0`
- 验收标准：
  1. 顶层不是数组时返回明确错误。
  2. 元素字段不匹配时返回反序列化错误。

### PRD-LJD-007：动态值解码

- 需求描述：提供 `decode_value()`，用于返回 `serde_json::Value`。
- 优先级：`P0`
- 验收标准：
  1. 可复用同一套规范化逻辑。
  2. 适合需要后续自行判断结构的场景。

### PRD-LJD-008：统一错误模型

- 需求描述：提供 `JsonDecodeError`，统一表示公共解码流程中的失败类型。
- 优先级：`P0`
- 验收标准：
  1. 至少包含以下错误种类：
     1. `EmptyInput`
     2. `InvalidJson`
     3. `UnexpectedTopLevel`
     4. `Deserialize`
  2. JSON 语法错误可透出必要诊断信息，如行列号。
  3. 默认不持有完整原始文本，避免敏感内容泄露。

### PRD-LJD-009：纯净与可复用

- 需求描述：解码器应保持纯净、无副作用、便于在不同场景复用。
- 优先级：`P0`
- 验收标准：
  1. 模块仅依赖标准库、`serde`、`serde_json`。
  2. 不依赖 logger、runtime、provider、HTTP、SSE。
  3. 实例可安全复制或共享使用。

## 7. 非功能需求

1. 一致性：相同输入问题在不同上层场景中必须产生一致语义。
2. 性能：未发生修复时尽量借用输入，减少堆分配。
3. 可维护性：公共 API 围绕单一对象，不演化成工具函数散点集合。
4. 可扩展性：未来加新规则时应优先扩展 `Options` / `Builder`，而不是破坏方法签名。
5. 安全性：错误信息默认不回显整段原始 JSON 文本。

## 8. 依赖与约束

### 8.1 技术依赖

1. `serde`
2. `serde_json`
3. Rust 标准库

### 8.2 上下游约束

1. `rust-json` 必须保持 provider 无关。
2. `rust-llmsdk-core` 可以基于它再封装 LLM 语义，但这些语义不应反向污染公共库。
3. 公共库只接受 `&str` 输入；上层如果存在“可能无输出”的语义，应在进入解码器前自行处理。

## 9. 里程碑与交付

### M1：可用的宽松 JSON 解码器（P0）

- 交付项：
  1. `LenientJsonDecoder`
  2. `LenientJsonDecoderOptions`
  3. `decode<T>()`
  4. `decode_object<T>()`
  5. `decode_array<T>()`
  6. `decode_value()`
  7. `JsonDecodeError`
  8. 内部规范化管线
  9. 单元测试与基础文档
- 里程碑完成定义：
  1. 可覆盖 Java 侧 `JsonUtils + JsonChatResponseDecoder/ListDecoder` 的核心行为。
  2. 可服务 LLM 场景，也可服务非 LLM 的宽松 JSON 文本解码场景。

### M2：构造与诊断增强（P1）

- 交付项：
  1. `LenientJsonDecoderBuilder`
  2. 更细粒度的阶段性错误上下文
  3. 更灵活的代码块策略
- 里程碑完成定义：
  1. 配置更复杂时不影响主 API 清晰度。
  2. 排障信息足够支撑线上使用。

## 10. 验收方案

### 10.1 单元测试验收

1. 空输入解码失败。
2. 带 UTF-8 BOM 的 JSON 文本可解码。
3. 被 `````json ... ````` 包裹的 JSON 文本可解码。
4. 字符串字面量含控制字符的 JSON 文本可解码。
5. `decode<T>()` 可成功解码普通结构体。
6. `decode_object<T>()` 在顶层为数组时返回 `UnexpectedTopLevel`。
7. `decode_array<T>()` 在顶层为对象时返回 `UnexpectedTopLevel`。
8. 非法 JSON 返回 `InvalidJson`。
9. 字段类型不匹配返回 `Deserialize`。

### 10.2 集成级验收

1. `rust-llmsdk-core` 结构化输出链可直接持有一个 `LenientJsonDecoder` 实例复用。
2. 非 LLM 文本来源也可复用同一对象完成宽松解码。

### 10.3 回归验收

1. 新增宽松规则时，不需要将公共 API 再拆回散函数模式。
2. 与 `json_design.zh_CN.md` 中的对象模型、错误模型、模块结构保持一致。

## 11. 风险与应对

1. 风险：名字叫 `JsonDecoder` 容易与标准解码混淆。
   - 应对：明确使用 `LenientJsonDecoder`，把“宽松”作为产品差异点。
2. 风险：过度泛化后又回到“大而空的 JSON 工具库”。
   - 应对：公共 API 只围绕解码器对象，不再扩散出大量无组织工具函数。
3. 风险：修复规则过多导致不可预测。
   - 应对：首版仅保留温和规范化规则，禁止激进修复。
4. 风险：错误模型过弱影响排障。
   - 应对：M1 先稳定错误种类，M2 再增强阶段信息。

## 12. PRD 与设计文档对齐矩阵

| PRD 需求ID | 对齐设计章节 | 对齐说明 |
| --- | --- | --- |
| PRD-LJD-001 | 4, 5.1 | `LenientJsonDecoder` 对象模型 |
| PRD-LJD-002 | 5.2 | `LenientJsonDecoderOptions` 设计 |
| PRD-LJD-003 | 6.1 | 内部规范化管线 |
| PRD-LJD-004 | 5.3, 6.2 | `decode<T>()` 通用解码流程 |
| PRD-LJD-005 | 5.3, 6.3 | `decode_object<T>()` 顶层对象约束 |
| PRD-LJD-006 | 5.3, 6.3 | `decode_array<T>()` 顶层数组约束 |
| PRD-LJD-007 | 5.3, 6.2 | `decode_value()` 动态值解码 |
| PRD-LJD-008 | 5.4 | `JsonDecodeError` 模型 |
| PRD-LJD-009 | 3, 7, 8 | 纯净依赖边界与复用原则 |
