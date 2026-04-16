# Qubit JSON

[![CircleCI](https://circleci.com/gh/qubit-ltd/rs-json.svg?style=shield)](https://circleci.com/gh/qubit-ltd/rs-json)
[![Coverage Status](https://coveralls.io/repos/github/qubit-ltd/rs-json/badge.svg?branch=main)](https://coveralls.io/github/qubit-ltd/rs-json?branch=main)
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
- Markdown 代码块
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
- **错误语义清晰**：提供稳定的错误分类和必要的上下文信息
- **低额外开销**：在可以借用原始输入时尽量避免额外分配

## 特性

### `LenientJsonDecoder`

- 可复用的解码器对象，内部持有不可变配置
- `decode<T>()`：把任意 JSON 顶层值解码为 `T`
- `decode_value()`：解码为 `serde_json::Value`
- `decode_object<T>()`：要求顶层必须是 JSON 对象
- `decode_array<T>()`：要求顶层必须是 JSON 数组

### `JsonDecodeOptions`

- `trim_whitespace`：裁剪首尾空白
- `strip_utf8_bom`：移除开头的 UTF-8 BOM
- `strip_markdown_code_fence`：移除最外层 Markdown 代码块包裹
- `strip_markdown_code_fence_requires_closing`：仅在存在合法闭合 fence
  时才执行移除
- `strip_markdown_code_fence_json_only`：仅移除语言标签为空、`json` 或
  `jsonc` 的 fence
- `escape_control_chars_in_strings`：转义 JSON 字符串字面量里的 ASCII 控制字符
- `max_input_bytes`：规范化前的输入字节数上限（可选）

### 显式错误模型

- `InputTooLarge`：原始输入大小超过配置上限
- `EmptyInput`：输入在规范化之后为空
- `InvalidJson`：规范化后的文本不是合法 JSON 语法
- `UnexpectedTopLevel`：JSON 顶层类型和调用的方法约束不一致
- `Deserialize`：JSON 语法合法，但无法反序列化为目标类型
- `JsonDecodeError.stage`：标识失败阶段（`normalize`、`parse`、
  `top_level_check`、`deserialize`）
- `JsonDecodeError.input_bytes` / `max_input_bytes`：用于诊断的可选字节上下文

## 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
qubit-json = "0.2.0"
```

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
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_markdown_code_fence: false,
        ..JsonDecodeOptions::default()
    });

    let value = decoder
        .decode_value("{\"ok\":true}")
        .expect("plain JSON should still decode with custom options");

    assert_eq!(value["ok"], true);
}
```

## 规范化规则

在对应选项启用时，解码器会按以下顺序处理输入：

1. 校验输入非空
2. 裁剪首尾空白
3. 移除开头的 UTF-8 BOM
4. 移除最外层 Markdown 代码块
5. 转义 JSON 字符串字面量中的 ASCII 控制字符

这个库不会做下面这些事情：

- 自动补引号
- 自动补逗号
- 自动补花括号或方括号
- 把任意畸形 JSON 猜测性地修复成合法 JSON

## 适用场景

Qubit JSON 适合这些情况：

- 你需要一个可复用、可配置的 JSON 解码对象
- 输入大体是合法 JSON，只是外层可能有包裹或轻度噪声
- 你希望在 `serde_json` 之外再得到一层稳定的错误语义

它不适合这些情况：

- 你需要对严重损坏的 JSON 做激进修复
- 输入本身并不是 JSON
- 直接调用 `serde_json::from_str()` 已经足够

## 对齐说明

本文档与当前实现保持一致：

- `LenientJsonDecoder` 通过内部的 `LenientJsonNormalizer` 完成输入规范化。
- 对外公开能力为 `decode`、`decode_object`、`decode_array`、`decode_value`。
- 规范化与错误模型由 `src/lenient_json_normalizer.rs`、`src/json_decode_error.rs` 实现，并有
  `tests/` 下对应测试覆盖。
- 需求与实现口径与
  `doc/json_prd.zh_CN.md` 和 `doc/json_design.zh_CN.md` 对齐。

## 许可证

本项目基于 Apache 2.0 许可证发布。详见 [LICENSE](LICENSE)。
