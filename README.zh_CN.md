# Qubit JSON

[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

面向 Rust 服务、配置读取器和数据管道的资源感知 JSON 基础设施。它在保留 Serde 数据模型的
同时，为 `serde_json` 补充调用方可控的资源准入，防止不可信输入在解析、值构造或输出阶段
无限制地消耗资源。输入必须是严格 JSON 时使用 `JsonDecoder`；如果输入边界明确允许对外部
文本中的 JSON 做受控清理，则使用 `NormalizingJsonDecoder`。

## 安装

当前最新版 `0.8`：

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.4", features = ["json"] }
serde_json = "1.0"
```

本地 checkout：

```toml
[dependencies]
qubit-json = { version = "0.8", path = "../rs-json" }
qubit-budget = { version = "0.4", features = ["json"] }
serde_json = "1.0"
```

如果需要解码到通过 derive 生成的应用类型，再添加
`serde = { version = "1.0", features = ["derive"] }`。

## 快速开始：准入 HTTP 请求体

下面的示例先接收一个包含完整 `u64` 范围标识符的请求体，再验证超出输入上限的请求会在
解码结果提交前被拒绝：

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;

fn main() -> Result<(), JsonDecodeError<JsonResource>> {
    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .max_depth(32)
        .max_nodes(256)
        .max_sequence_items(64)
        .max_map_entries(64)
        .max_key_bytes(128)
        .max_string_bytes(2048)
        .max_number_bytes(20)
        .max_payload_bytes(4096)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let value: serde_json::Value =
        decoder.decode_utf8(br#"{"id":18446744073709551615,"ok":true}"#)?;
    assert_eq!(value["id"], serde_json::json!(u64::MAX));

    let small_limits = limits.into_builder().max_input_bytes(8).build();
    let mut small_decoder = JsonDecoder::with_limits(small_limits);
    let error = small_decoder
        .decode_utf8::<serde_json::Value>(br#"{"ok":true}"#)
        .expect_err("the request body must exceed eight bytes");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    Ok(())
}
```

与只调用 `serde_json::from_slice` 相比，这个边界会显式执行资源准入，并通过 `kind()` 返回稳定的
错误类别。JSON 准入成功后，再执行模式校验、权限检查和领域规则。

## 选择解码边界

选择入口时应以输入契约为准，而不是看某份数据能否碰巧解析成功：

| 输入契约 | API |
| --- | --- |
| 输入必须已经是完整、严格的 JSON | `JsonDecoder` |
| 严格解码前允许清理指定的展示痕迹 | 使用显式策略构造的 `NormalizingJsonDecoder` |
| 需要检查规范化结果、重复解码，或让结果借用规范化文本 | 先调用 `NormalizingJsonDecoder::prepare_str` / `prepare_utf8`，再通过 `NormalizedJsonDocument` 的解码方法处理 |

默认规范化策略可以清理首尾空白、一个 BOM、一层 JSON Markdown 围栏，以及字符串内未经转义
的 ASCII 控制字符。这只是受控规范化，不是另一种 JSON 方言；注释、尾随逗号、未加引号的键
和缺失的 JSON 语法仍会被拒绝。

## 为什么需要这个项目

语法正确的 JSON 仍可能过大、嵌套过深，或在构造内存对象时消耗过多资源。`qubit-json` 保留
JSON 语法和 Serde 兼容性，并允许调用方限制原始及规范化输入、嵌套深度、节点数、集合大小、
键、字符串、数字、有效载荷和编码输出。跨调用累计的用量及其提交边界由 `qubit-budget` 的会话
和事务明确表达。

## 核心能力

| 领域 | 适用场景 | 明确边界 |
| --- | --- | --- |
| `decode` | 严格准入 JSON，或按显式配置规范化文本 | 只执行配置允许的转换，不会补齐缺失的 JSON 语法 |
| `encode` | 生成受预算约束的严格 JSON | 完整序列化成功后才提交值资源用量，但 I/O 失败仍可能在外部写入器中留下部分字节 |
| `value` | 从 Serde 事件构造受预算约束的 `serde_json::Value` | Serde seed 看不到数字的原始文本，也不能执行文本级数值范围检查 |
| `value::traverse` | 迭代读取或原地修改已构造的值树 | 修改按步骤生效；访问器或输出预算失败不会回滚此前的改动 |

`qubit-budget` 负责限制、资源标识、预算和会话；`qubit-json` 负责 JSON 规范化、词法校验、
文本编解码、值构造和遍历。

## 明确边界

- 严格准入会检查 JSON 语法和文档规定的数字范围，但不会自动要求对象键唯一。若唯一性属于
  输入契约，应选择 `DuplicateKeyRejectingJsonValue` 等目标类型。
- 负整数须能装入 `i64`，非负整数须能装入 `u64`，小数或指数形式须能表示为有限 `f64`。
  更宽的整数以及不能接受二进制舍入的精确十进制值，应使用字符串或领域类型。
- 所有不可信边界都应设置有限资源上限。`unlimited()` 只应用于可信输入，或已由其他层完成
  准入的数据；外层输出上限不能替代解析前的输入与结构准入。
- 严格解码器和规范化解码器当前接收完整的 `&str` 或 `&[u8]`。输入字节限制只负责准入调用方
  已经提供的切片，不能限制 HTTP body 聚合器或其他传输层已经分配的内存。交给解码器之前，
  应先在外层传输边界设置有界读取或 body 聚合上限。
- 诊断信息默认脱敏。严格解码器通过 `JsonDecoder::with_diagnostic_policy` 配置，规范化
  解码器通过其规范化策略配置。只有在输入派生信息可以安全保留和记录时，才启用
  `DiagnosticPolicy::Detailed`。

## 延伸阅读

- [中文用户手册](doc/user_guide.zh_CN.md) ·
  [English user guide](doc/user_guide.md)
- [JSON 数字契约](doc/number_contract.zh_CN.md) ·
  [JSON number contract](doc/number_contract.md)
- [设计文档](doc/json_design.zh_CN.md) ·
  [Design documents](doc/json_design.md)
- [基准测试基线](doc/benchmark_baseline.zh_CN.md) ·
  [Benchmark baseline](doc/benchmark_baseline.md)
- [从 0.3 迁移到 0.8](doc/migration_0_3_to_0_8.zh_CN.md) ·
  [Migration from 0.3 to 0.8](doc/migration_0_3_to_0_8.md)
- [中文变更记录](CHANGELOG.zh_CN.md) · [Changelog](CHANGELOG.md)
- [已发布 API 文档](https://docs.rs/qubit-json)；当前分支 API 请运行
  `cargo doc --all-features --open`

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
