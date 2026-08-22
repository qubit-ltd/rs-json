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
qubit-budget = { version = "0.3", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## 按领域选择能力

| 领域 | 适用场景 | 明确边界 |
| --- | --- | --- |
| `decode` | 规范化文本输入和严格 JSON 字节流 | 不猜测缺失的引号、逗号或括号 |
| `encode` | 严格 JSON 输出 | 使用调用方持有 session 的有状态 encoder 对象 |
| `value` | 由 Serde 事件构造 `serde_json::Value` | 对解码后的 value transaction 记账 |
| `value::traverse` | 迭代读取或修改已物化的 value | 可变处理是增量式的，不提供事务回滚 |

`qubit-budget` 负责限制、资源标识、预算和 session；`qubit-json` 负责 JSON 规范化、词法
校验、文本编解码、value 构造和遍历。

## 宽松输入

`NormalizingJsonDecoder` 是持有不可变 `NormalizingJsonDecodePolicy` 的可复用对象。它只按已配置
规则移除噪声，然后直接反序列化为所需类型。

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::{NormalizingJsonDecodePolicy, NormalizingJsonDecoder};

let mut decoder = NormalizingJsonDecoder::owned(
    NormalizingJsonDecodePolicy::builder().build(),
    JsonDecodeLimits::builder()
        .max_input_bytes(1024)
        .build(),
);
let value = decoder.decode_value("```json\n{\"ok\":true}\n```")?;
assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::decode::NormalizingJsonDecodeError>(())
```

需要累计记账时使用 `NormalizingJsonDecoder::from_session` 构造有状态 decoder。原始输入和规范化
输入的消耗会在一次尝试后保留；只有完整的强类型解码成功，解码后 value 的暂存消耗才提交。
错误默认脱敏；仅在输入诊断可安全暴露的环境中启用 `DiagnosticPolicy::Detailed`。
规范化 policy 不携带资源限制：`owned` 显式接收 `JsonDecodeLimits`，`from_session` 显式接收
`JsonDecodeSession`。只有确实需要无限预算时才传入 `JsonDecodeLimits::default()`。

## 严格文本对象

严格 API 不修复文本。围绕调用方持有的 session 构造 decoder 或 encoder 对象，再对一个或
多个文档调用其方法。codec 不实现 `Default`；通常应调用 `owned(limits)`，只有明确需要标准
无限预算 session 时才调用 `unlimited()`。

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let mut decoder = JsonDecoder::owned(
    JsonDecodeLimits::<JsonResource, usize>::new(),
);
let value: serde_json::Value = decoder.decode_utf8(br#"{"ok":true}"#)?;
assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::decode::JsonDecodeError<
#     qubit_budget::json::JsonResource,
# >>(())
```

```rust
use qubit_budget::json::{JsonEncodeLimits, JsonResource};
use qubit_json::encode::JsonEncoder;

let value = serde_json::json!({"ok": true});
let mut encoder = JsonEncoder::owned(
    JsonEncodeLimits::<JsonResource, usize>::new(),
);
let bytes = encoder.to_vec(&value)?;
assert_eq!(bytes, br#"{"ok":true}"#);
# Ok::<(), qubit_json::encode::JsonEncodeError<
#     qubit_budget::json::JsonResource,
# >>(())
```

`JsonEncoder::write_buffered` 只在完整输出已准备好写入时提交；`write_incremental`
在流式写入失败时保留已经接受的输出前缀。

## 错误与预算语义

五个公开 error 各自归属业务领域：

1. `decode::NormalizingJsonDecodeError`：规范化和宽松强类型解码失败。
2. `decode::JsonDecodeError`：严格预算、语法或强类型解码失败。
3. `encode::JsonEncodeError`：严格预算、原始 JSON、序列化或 I/O 失败。
4. `decode::JsonSyntaxError`：稳定的语法原因和位置元数据。
5. `value::traverse::JsonTreeProcessError`：遍历预算或 visitor 失败。

带预算操作使用 transaction：解码后 value 或输出消耗在文档定义的成功边界提交。decode
session 中的输入消耗会在失败尝试后刻意保留。

## 高级 value 与 tree 用法

`JsonValueSeed` 在调用方 transaction 中构造已物化 value 并记账。`JsonTreeReader` 不使用
Rust 递归地访问每个已准入节点；其 `account` 方法在调用方已有 transaction 中暂存整棵树的
消耗，不调用 visitor，也不提交 transaction。`JsonTreeMutator` 原地应用 visitor 的变更，并可通过
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
