# Qubit JSON

[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

面向 Rust 的资源感知 JSON 基础设施：它把可预测的宽松输入规范化、严格文本编解码、
带预算的 value 构造和非递归 tree 处理放在同一套资源语义下，同时保留 Serde 原生数据
模型和反序列化行为。

## 按边界选择能力

| 模块 | 适用场景 | 明确边界 |
| --- | --- | --- |
| `lenient` | 规范化围栏包裹或有轻微噪声的文本，再反序列化为 `T` | 只做文档列出的修复，不猜测引号、逗号或括号 |
| `text` | 严格、带预算的 JSON 编解码 | 使用调用方持有的 decode/encode session，不修复文本 |
| `value` | 通过 Serde seed 构造 `serde_json::Value` | 核算解码后的 value 资源，具体实现保持私有 |
| `tree` | 迭代访问或修改已经物化的 `Value` | `process_mut` 不提供事务回滚 |

`qubit-budget` 负责 JSON 资源标识、限制、预算和可变 session；`qubit-json` 负责规范化、
词法准入、严格文本适配、value 构造和 tree 遍历。

## 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
qubit-json = "0.7"
qubit-budget = { version = "0.4", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
```

只有为强类型解码派生 `Deserialize` 时才需要直接添加 `serde`。若代码直接使用
`serde_json::Value` 或 `serde_json` 宏，请自行声明 `serde_json` 直接依赖；本 crate
有意不再导出它。

## 快速开始

### 以累计预算解码围栏包裹的响应

假设服务从文本通道接收 Markdown 围栏包裹的 JSON，需要直接得到强类型结果，并且要核算
跨重试的所有工作。此时复用一个 `JsonDecodeSession`：依次累计原始输入、规范化输入和
解码后 value 资源。规范化文档通过词法准入后直接反序列化为 `T`，不会先构造中间
`serde_json::Value`。

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonDecodeSession};
use qubit_json::lenient::LenientJsonDecoder;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Reply {
    ok: bool,
}

let limits = JsonDecodeLimits::empty()
    .with_max_input_bytes(64)
    .with_max_normalized_input_bytes(32)
    .with_max_nodes(2)
    .with_max_map_entries(1)
    .with_max_key_bytes(2)
    .with_max_payload_bytes(2);
let mut session = JsonDecodeSession::owned(limits);
let decoder = LenientJsonDecoder::default();

let reply: Reply = decoder.decode_with_session(
    "```json\n{\"ok\":true}\n```",
    &mut session,
)?;
assert_eq!(reply, Reply { ok: true });
assert_eq!(session.value_budget().structure_budget().used_nodes(), 2);
# Ok::<(), qubit_json::lenient::LenientJsonDecodeError>(())
```

只要某一步计费成功，即使后续预算检查、语法检查或目标类型反序列化失败，已消耗资源也
不会回滚。`Budget`/`Admission` 错误可通过 `measured_budget_error()` 读取结构化拒绝
详情。普通 `decode()` 仍走更快的“规范化后直接反序列化”路径，不执行 value 预检。

## 核心能力

### 宽松解码

- 可复用的解码器对象，内部持有不可变配置
- `decode<T>()`：把任意 JSON 顶层值解码为 `T`
- `decode_with_session<T>()`：直接反序列化前，累计核算原始输入、规范化输入和 value
- `decode_slice<T>()`：校验 UTF-8 字节并解码为 `T`
- `decode_value()`：解码为 `serde_json::Value`
- `decode_object<T>()`：要求顶层必须是 JSON 对象，并直接从规范化文本反序列化 `T`
- `decode_array<T>()`：要求顶层必须是 JSON 数组，并直接从规范化文本反序列化元素

### `JsonDecodeOptions`

- 每个选项都提供不可变 getter 和值式 builder
- 预设：`lenient()`、`strict()`；严格模式禁用文本改写，但仍保留空输入分类、
  可选大小限制、隐私处理与稳定错误映射
- `trim_whitespace`：裁剪首尾空白
- `strip_utf8_bom`：移除开头的 UTF-8 BOM
- `markdown_fence_policy`：统一表达禁用、任意语言或仅 JSON 围栏，以及可选或必须闭合
- 默认只接受空标签、`json` 和 `jsonc` 围栏；接受任意语言标签必须显式选择
  `MarkdownFencePolicy::Any`
- `jsonc` 仅作为 Markdown 代码块标签被识别；代码块内容仍按标准 JSON 解析，因此注释和
  尾随逗号依然无效
- `escape_control_chars_in_strings`：转义 JSON 字符串字面量里的 ASCII 控制字符
- `max_input_bytes`：规范化前的输入字节数上限（可选）
- `max_normalized_bytes`：规范化后 JSON 的字节数上限（可选），在控制字符修复分配前检查
- `error_privacy_policy`：选择默认安全的脱敏错误，或显式启用完整 serde 诊断

默认配置不会设置 `max_input_bytes` 和 `max_normalized_bytes`，避免库替应用强加资源上限。
来自不可信边界的输入应由调用方按自身内存与延迟预算显式限制。

### 严格文本、value 与 tree 基础设施

- `text::decode_slice` / `text::decode_slice_seed` 使用 `JsonDecodeSession` 严格
  解码字节；`text::encode_to_vec` / `text::encode_to_writer` 使用
  `JsonEncodeSession` 编码。
- `text::JsonEncodeError::InvalidRawJson` 直接保留稳定的 `JsonSyntaxError` reason、
  offset、line 和 column，不再用字符串重建 `serde_json::Error`。
- `value::BudgetedJsonValueSeed` 是递增核算 value 预算并构造
  `serde_json::Value` 的唯一公开路径。
- `tree::JsonTreeProcessor` 允许输入 value 的借用短于 budget 借用。
  `process_mut` 返回错误时保留此前的 mutation 和预算消费；恢复 guard 只保证 root
  仍是结构有效的 `Value`，不会恢复原值。

### 显式错误模型

- `Budget`：调用方持有的 value 限制在 `Admission` 阶段拒绝工作
- `InputTooLarge`：原始或规范化后的输入大小超过对应配置上限
- `EmptyInput`：输入在规范化之后为空
- `InvalidUtf8`：原始字节输入不是合法 UTF-8
- `InvalidJson`：规范化后的文本不是合法 JSON 语法
- `UnexpectedTopLevel`：JSON 顶层类型和调用的方法约束不一致
- `Deserialize`：JSON 语法合法，但无法反序列化为目标类型
- `JsonDecodeError` 通过不可变访问器提供失败种类、阶段、消息、顶层上下文、原始与
  规范化字节数及输入上限
- 解析行列访问器对应规范化后的 JSON 文本
- 非法 UTF-8 错误通过 `utf8_valid_up_to()` 和 `utf8_error_len()` 提供安全的
  字节偏移及可确定时的非法序列长度
- `privacy_policy()` 记录每个错误实际采用的隐私策略
- 默认 `Redacted` 策略不会在解析/反序列化消息中保留 serde 提供的输入片段，
  `Error::source()` 返回 `None`
- `Detailed` 会保留完整 UTF-8 或 serde source，因此可能暴露输入派生诊断；
  仅应在受控环境中显式启用

## 其他宽松解码示例

### 解码字符串中包含原始控制字符的 JSON

```rust
use qubit_json::lenient::LenientJsonDecoder;

fn main() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("{\"text\":\"line 1\nline 2\"}")
        .expect("decoder should escape raw control characters inside strings");

    assert_eq!(value["text"], "line 1\nline 2");
}
```

### 自定义解码选项

```rust
use qubit_json::lenient::JsonDecodeOptions;
use qubit_json::lenient::LenientJsonDecoder;

fn main() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::lenient()
            .with_max_input_bytes(Some(1024)),
    );

    let value = decoder
        .decode_value("{\"ok\":true}")
        .expect("plain JSON should still decode with custom options");

    assert_eq!(value["ok"], true);
}
```

### 为不可信来源设置输入上限

`JsonDecodeOptions::default()` 有意不设置 `max_input_bytes` 和
`max_normalized_bytes`，避免库强加与应用场景无关的限制。当输入跨越信任边界时，应根据调用方的
内存和延迟预算配置上限。

`max_input_bytes` 约束原始输入。`max_normalized_bytes` 在裁剪和移除代码块之后、控制字符
修复分配之前约束规范化后的 JSON。单个原始 ASCII 控制字节转义为 `\\u00XX` 时，内容最坏
可从 1 字节扩展为 6 字节。

```rust
use qubit_json::lenient::{JsonDecodeOptions, LenientJsonDecoder};

let decoder = LenientJsonDecoder::new(
    JsonDecodeOptions::default()
        .with_max_input_bytes(Some(1_048_576))
        .with_max_normalized_bytes(Some(6_291_456)),
);
let value = decoder.decode_value("{\"ok\":true}")?;

assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::lenient::LenientJsonDecodeError>(())
```

### 显式启用详细错误诊断

完整 serde 诊断可能包含输入值。只有当诊断存储及其读者均可信时才应启用。

```rust
use qubit_json::lenient::{
    ErrorPrivacyPolicy,
    JsonDecodeOptions,
    LenientJsonDecoder,
};

fn main() {
    let options = JsonDecodeOptions::default()
        .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed);
    let decoder = LenientJsonDecoder::new(options);

    let error = decoder
        .decode::<u64>(r#""not a number""#)
        .expect_err("the JSON string cannot deserialize into u64");
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Detailed);
    assert!(std::error::Error::source(&error).is_some());
}
```

## 行为契约

### 规范化规则

在对应选项启用时，解码器会按以下顺序处理输入：

1. 校验原始输入字节数是否超过可选上限
2. 校验输入非空
3. 裁剪首尾空白
4. 移除开头的 UTF-8 BOM
5. 再次裁剪首尾空白
6. 移除最外层反引号或波浪线 Markdown 代码块
7. 再次裁剪首尾空白
8. 在分配前校验规范化后 JSON 的可选字节数上限
9. 转义 JSON 字符串字面量中的 ASCII 控制字符

`lenient` 模块不会做下面这些事情：

- 自动补引号
- 自动补逗号
- 自动补花括号或方括号
- 把任意畸形 JSON 猜测性地修复成合法 JSON

### session 与 mutation 的失败语义

- Decode session 采用累计记账。错误发生前已经消费的原始输入、规范化输入和 value
  资源不会回滚；单次被拒绝的预算增量仍保持原子性，但更早的成功增量会保留。
- 严格编码只有在完整序列化成功后才提交 output 记账；若序列化稍后失败，value 记账
  可能已经发生。
- `JsonTreeProcessor::process_mut` 采用递增修改。visitor 或预算错误发生前已完成的
  mutation 和预算消费仍可观察。

## 适用场景

Qubit JSON 适合这些情况：

- 你希望文本、value 和 tree 共享同一套资源核算语义
- 你需要一个可复用、可配置的宽松 JSON 解码对象
- 输入大体是合法 JSON，只是外层可能有包裹或轻度噪声
- 你希望在 `serde_json` 之外再得到一层稳定且默认安全的错误语义

它不适合这些情况：

- 你需要对严重损坏的 JSON 做激进修复
- 输入本身并不是 JSON
- 直接调用 `serde_json::from_str()` 已能满足全部需求

## 兼容与升级

带预算的序列化会识别 `serde_json` 的私有 Number 和 RawValue 协议名。因此生产依赖
精确锁定为 `serde_json = 1.0.151`，且
`src/budget/internal/serde_json_compat.rs` 是这些 token 在生产代码中的唯一所有者。
升级 `serde_json` 时必须：

1. 更新 `Cargo.toml` 中的精确版本；
2. 更新根目录和 `fuzz/Cargo.lock`；
3. 对照 compat 模块复核上游私有 Number、RawValue serializer；
4. 运行私有协议、serializer 回归、两棵依赖树检查、fuzz workspace check 和项目完整
   质量门禁。

复核通过前，不得在 compat 模块外新增 token 判断，也不得放宽精确版本约束。

## 延伸阅读

- [English README](README.md)
- [设计说明](doc/json_design.zh_CN.md)
- [产品需求](doc/json_prd.zh_CN.md)
- [基准基线](doc/benchmark_baseline.zh_CN.md)
- [API 文档](https://docs.rs/qubit-json)

## 开发验证

先运行 `./align-ci.sh`，再运行 `./ci-check.sh`。Criterion 基准包含 1 KiB、64 KiB 和
1 MiB 的带预算 strict/lenient decode/encode 对比，可通过以下命令编译两个目标：

```bash
cargo bench --bench decoder_bench --no-run
cargo bench --bench budgeted_serde_json --no-run
```

可选 fuzz 目标仅用于开发，不会成为运行时依赖。它覆盖默认、严格、仅 JSON 围栏和
必须闭合围栏四类策略，并由 `.github/workflows/fuzz.yml` 定时执行有时限的运行；失败时会保留
可复现输入产物。
安装 `cargo-fuzz` 后，可在仓库根目录构建或运行同一目标：

```bash
rustup toolchain install nightly-2026-06-05 --profile minimal
cargo install cargo-fuzz --version 0.13.2 --locked
(cd fuzz && cargo +nightly-2026-06-05 fuzz build decoder)
(cd fuzz && cargo +nightly-2026-06-05 fuzz run decoder -- -max_len=4096)
```

## 测试

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 运行测试
cargo test --all-features

# 运行项目 CI 检查
./ci-check.sh

# 检查代码覆盖率
./coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-json](https://github.com/qubit-ltd/rs-json)
