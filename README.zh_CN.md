# Qubit JSON

[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![docs.rs](https://img.shields.io/docsrs/qubit-json?logo=docs.rs)](https://docs.rs/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

为 Rust 提供面向非完全可信文本输入的宽松 JSON 解码器。

## 概述

Qubit JSON 在 `serde_json` 之上提供了一层小而可预测的解码能力。它的
核心类型 `LenientJsonDecoder` 会先对输入做有限的规范化，再进行 JSON
解析和反序列化。

这个库适合处理这类来源的 JSON 文本：

- Markdown 包裹文本
- 使用反引号或波浪线围栏的 Markdown 代码块
- 复制粘贴的代码片段
- CLI 输出流
- 其他可能包裹了 JSON 的文本通道

这个库的边界是刻意收窄的。它不是通用 JSON 修复引擎，也不会去猜测缺失
的引号、逗号或花括号。

## 设计目标

- **宽松但可预测**：只处理少量、边界明确的输入问题
- **对象化 API**：通过可复用的 `LenientJsonDecoder` 实例暴露能力，而不是
  散落的工具函数
- **以 Serde 为核心**：真正的解析和反序列化仍然交给 `serde_json`
- **隐私感知错误**：默认提供稳定的脱敏诊断，仅在显式配置后保留完整 serde 明细
- **低额外开销**：在可以借用原始输入时尽量避免额外分配

## 特性

### `LenientJsonDecoder`

- 可复用的解码器对象，内部持有不可变配置
- `decode<T>()`：把任意 JSON 顶层值解码为 `T`
- `decode_value()`：解码为 `serde_json::Value`
- `decode_object<T>()`：要求顶层必须是 JSON 对象，并直接从规范化文本反序列化 `T`
- `decode_array<T>()`：要求顶层必须是 JSON 数组，并直接从规范化文本反序列化元素

### `JsonDecodeOptions`

- 每个选项都提供不可变 getter 和值式 builder
- 预设：`lenient()`、`strict()`、`json_code_fences_only()`
- `trim_whitespace`：裁剪首尾空白
- `strip_utf8_bom`：移除开头的 UTF-8 BOM
- `markdown_fence_policy`：统一表达禁用、任意语言或仅 JSON 围栏，以及可选或必须闭合
- `escape_control_chars_in_strings`：转义 JSON 字符串字面量里的 ASCII 控制字符
- `max_input_bytes`：规范化前的输入字节数上限（可选）
- `error_privacy_policy`：选择默认安全的脱敏错误，或显式启用完整 serde 诊断

默认配置不会设置 `max_input_bytes`，避免库替应用强加资源上限。来自不可信边界的输入应由
调用方按自身内存与延迟预算，通过 `with_max_input_bytes(Some(limit))` 显式限制。

### 显式错误模型

- `InputTooLarge`：原始输入大小超过配置上限
- `EmptyInput`：输入在规范化之后为空
- `InvalidJson`：规范化后的文本不是合法 JSON 语法
- `UnexpectedTopLevel`：JSON 顶层类型和调用的方法约束不一致
- `Deserialize`：JSON 语法合法，但无法反序列化为目标类型
- `JsonDecodeError` 通过不可变访问器提供失败种类、阶段、消息、顶层上下文、原始与
  规范化字节数及输入上限
- 解析行列访问器对应规范化后的 JSON 文本
- `privacy_policy()` 记录每个错误实际采用的隐私策略
- 默认 `Redacted` 策略不会在解析/反序列化消息中保留 serde 提供的输入片段，
  `Error::source()` 返回 `None`
- `Detailed` 会保留完整 serde 消息与 source，因此可能暴露输入值；仅应在受控诊断
  环境中显式启用

## 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
qubit-json = "0.4"
serde = { version = "1.0", features = ["derive"] }
```

只有在像下面第一个快速开始示例那样，为强类型解码派生 `Deserialize` 时，才需要
直接添加 `serde` 依赖。

## 快速开始

### 从 Markdown 代码块中解码 JSON 对象

```rust
use serde::Deserialize;
use qubit_json::LenientJsonDecoder;

#[derive(Debug, Deserialize)]
struct User {
    name: String,
    age: u8,
}

fn main() {
    let decoder = LenientJsonDecoder::default();
    let user: User = decoder
        .decode_object("```json\n{\"name\":\"Alice\",\"age\":30}\n```")
        .expect("decoder should extract and decode the fenced JSON object");

    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}
```

### 解码字符串中包含原始控制字符的 JSON

```rust
use qubit_json::LenientJsonDecoder;

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
use qubit_json::{LenientJsonDecoder, JsonDecodeOptions};

fn main() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::json_code_fences_only()
            .with_max_input_bytes(Some(1024)),
    );

    let value = decoder
        .decode_value("{\"ok\":true}")
        .expect("plain JSON should still decode with custom options");

    assert_eq!(value["ok"], true);
}
```

### 显式启用详细错误诊断

完整 serde 诊断可能包含输入值。只有当诊断存储及其读者均可信时才应启用。

```rust
use qubit_json::{
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

## 规范化规则

在对应选项启用时，解码器会按以下顺序处理输入：

1. 校验原始输入字节数是否超过可选上限
2. 校验输入非空
3. 裁剪首尾空白
4. 移除开头的 UTF-8 BOM
5. 再次裁剪首尾空白
6. 移除最外层反引号或波浪线 Markdown 代码块
7. 再次裁剪首尾空白
8. 转义 JSON 字符串字面量中的 ASCII 控制字符
9. 再次裁剪首尾空白

这个库不会做下面这些事情：

- 自动补引号
- 自动补逗号
- 自动补花括号或方括号
- 把任意畸形 JSON 猜测性地修复成合法 JSON

## 适用场景

Qubit JSON 适合这些情况：

- 你需要一个可复用、可配置的 JSON 解码对象
- 输入大体是合法 JSON，只是外层可能有包裹或轻度噪声
- 你希望在 `serde_json` 之外再得到一层稳定且默认安全的错误语义

它不适合这些情况：

- 你需要对严重损坏的 JSON 做激进修复
- 输入本身并不是 JSON
- 直接调用 `serde_json::from_str()` 已经足够

## 对齐说明

本文档与当前实现保持一致：

- `LenientJsonDecoder` 通过内部的 `LenientJsonNormalizer` 完成输入规范化。
- 对外公开能力为 `decode`、`decode_object`、`decode_array`、`decode_value`。
- 规范化与错误模型由 `src/internal/lenient_json_normalizer.rs`、`src/json_decode_error.rs` 实现，并有
  `tests/` 下对应测试覆盖。
- 需求与实现口径与
  `doc/json_prd.zh_CN.md` 和 `doc/json_design.zh_CN.md` 对齐。

## 开发验证

先运行 `./align-ci.sh`，再运行 `./ci-check.sh`。Criterion 基准可通过
`cargo bench --bench decoder_bench --no-run` 编译。

可选 fuzz 目标仅用于开发，不会成为运行时依赖。安装 `cargo-fuzz` 后，在仓库根目录执行：

```bash
cargo install cargo-fuzz
(cd fuzz && cargo fuzz build decoder)
(cd fuzz && cargo fuzz run decoder)
```

## 许可证

本项目基于 Apache 2.0 许可证发布。详见 [LICENSE](LICENSE)。
