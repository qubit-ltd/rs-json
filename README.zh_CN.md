# Qubit JSON

[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

面向 Rust 的资源感知 JSON 基础设施。它保留 Serde 原生数据模型，同时将输入规范化、严格
文本编解码、value 构造和 tree 处理纳入明确的预算语义。

## 安装

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.4", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## 按领域选择能力

| 领域 | 适用场景 | 明确边界 |
| --- | --- | --- |
| `lenient` | Markdown 围栏和少量已定义的文本噪声 | 不猜测缺失的引号、逗号或括号 |
| `text` | 严格 JSON 字节流 | 使用调用方持有 session 的有状态 decoder/encoder 对象 |
| `value` | 由 Serde 事件构造 `serde_json::Value` | 对解码后的 value transaction 记账 |
| `tree` | 迭代读取或修改已物化的 value | 可变处理是增量式的，不提供事务回滚 |

`qubit-budget` 负责限制、资源标识、预算和 session；`qubit-json` 负责 JSON 规范化、词法
校验、文本编解码、value 构造和遍历。

## 宽松输入

`LenientJsonDecoder` 是持有不可变 `LenientJsonDecodeOptions` 的可复用对象。它只按已配置
规则移除噪声，然后直接反序列化为所需类型。

```rust
use qubit_json::lenient::{LenientJsonDecodeOptions, LenientJsonDecoder};

let decoder = LenientJsonDecoder::new(
    LenientJsonDecodeOptions::builder().max_input_bytes(Some(1024)).build(),
);
let value = decoder.decode_value("```json\n{\"ok\":true}\n```")?;
assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::lenient::LenientJsonDecodeError>(())
```

需要累计记账时使用 `decode_with_session`。原始输入和规范化输入的消耗会在一次尝试后保留；
只有完整的强类型解码成功，解码后 value 的暂存消耗才提交。错误默认脱敏；仅在输入诊断可
安全暴露的环境中启用 `ErrorPrivacyPolicy::Detailed`。

## 严格文本对象

严格 API 不修复文本。围绕调用方持有的 session 构造 decoder 或 encoder 对象，再对一个或
多个文档调用其方法。

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonDecodeSession};
use qubit_json::text::JsonTextDecoder;

let mut decode_session = JsonDecodeSession::owned(JsonDecodeLimits::<JsonResource, usize>::new());
let value: serde_json::Value = JsonTextDecoder::new(&mut decode_session)
    .decode(br#"{"ok":true}"#)?;
assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::text::JsonDecodeError<
#     qubit_budget::json::JsonResource,
# >>(())
```

```rust
use qubit_budget::json::{JsonEncodeLimits, JsonEncodeSession};
use qubit_json::text::JsonTextEncoder;

let value = serde_json::json!({"ok": true});
let mut encode_session = JsonEncodeSession::owned(JsonEncodeLimits::<JsonResource, usize>::new());
let mut encoder = JsonTextEncoder::new(&mut encode_session);
let bytes = encoder.to_vec(&value)?;
assert_eq!(bytes, br#"{"ok":true}"#);
# Ok::<(), qubit_json::text::JsonEncodeError<
#     qubit_budget::json::JsonResource,
# >>(())
```

`JsonTextEncoder::write_buffered` 只在完整输出已准备好写入时提交；`write_incremental`
在流式写入失败时保留已经接受的输出前缀。

## 错误与预算语义

五个公开 error 各自归属业务领域：

1. `lenient::LenientJsonDecodeError`：规范化和宽松强类型解码失败。
2. `text::JsonDecodeError`：严格预算、语法或强类型解码失败。
3. `text::JsonEncodeError`：严格预算、原始 JSON、序列化或 I/O 失败。
4. `text::JsonSyntaxError`：稳定的语法原因和位置元数据。
5. `tree::JsonTreeProcessError`：遍历预算或 visitor 失败。

带预算操作使用 transaction：解码后 value 或输出消耗在文档定义的成功边界提交。decode
session 中的输入消耗会在失败尝试后刻意保留。

## 高级 value 与 tree 用法

`JsonValueSeed` 在调用方 transaction 中构造已物化 value 并记账。`JsonTreeReader` 不使用
Rust 递归地访问每个已准入节点；`JsonTreeMutator` 原地应用 visitor 的变更，并可通过
`JsonTreeBudgetRejection` 跳过被拒绝的子树。`JsonTreeBudgetTracker` 适合重复执行完整 tree
记账。

这些能力只核算 JSON 资源限制，不替代应用语义校验。请为实际信任边界选择限制，并避免将
详细诊断写入不可信日志。

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
