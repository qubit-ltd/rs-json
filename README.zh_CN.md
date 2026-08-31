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

```toml
[dependencies]
qubit-json = "0.8"
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

## 规范化外部文本中的 JSON

当输入本应是 JSON，但可能带有输入契约明确允许的传输或展示痕迹时，应使用
`NormalizingJsonDecoder`。常见来源包括生成式文本、从 Markdown 复制的代码片段和文本配置
文件。它先按 `NormalizingJsonDecodePolicy` 规范化文本，再执行与 `JsonDecoder` 相同的严格
JSON 语法、数字范围和资源准入检查。

`NormalizingJsonDecodePolicy::lenient()` 提供标准规范化配置：

- 去除首尾空白；
- 去除一个开头的 UTF-8 BOM；
- 去除一层外围 JSON Markdown 代码围栏，结束围栏可以省略；
- 转义 JSON 字符串内部未经转义的 ASCII 控制字符；
- 对源自输入的诊断细节进行脱敏。

这里的“宽松”仅指执行上述受控规范化，并不是支持另一种 JSON 方言。注释、尾随逗号、未加
引号的键或缺失的 JSON 语法仍会被拒绝。

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;

let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_input_bytes(4096)
    .max_normalized_input_bytes(4096)
    .max_depth(32)
    .max_nodes(256)
    .max_sequence_items(64)
    .max_map_entries(64)
    .max_key_bytes(128)
    .max_string_bytes(2048)
    .max_number_bytes(20)
    .max_payload_bytes(4096)
    .build();
let mut decoder = NormalizingJsonDecoder::with_limits(
    NormalizingJsonDecodePolicy::lenient(),
    limits,
);
let value = decoder
    .decode_object_str::<serde_json::Value>("```json\n{\"ok\":true}\n```")
    .expect("当前策略应接受 Markdown JSON 代码围栏");
assert_eq!(value["ok"], true);
```

应根据输入契约选择解码入口：

| 输入契约 | API |
| --- | --- |
| 输入必须已经是完整、严格的 JSON | `JsonDecoder` |
| 严格解码前允许清理指定的展示痕迹 | 使用显式策略构造的 `NormalizingJsonDecoder` |
| 需要检查规范化结果、重复解码，或让结果借用规范化文本 | 先调用 `NormalizingJsonDecoder::prepare_str` / `prepare_utf8`，再通过 `NormalizedJsonDocument` 的解码方法处理 |

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

## 核心 API 一览

| API | 用途 |
| --- | --- |
| `decode::JsonDecoder` | 严格校验并解码完整的 JSON 字符串或 UTF-8 字节切片；支持限制顶层必须为对象或数组，并可复用累计记账状态 |
| `decode::NormalizingJsonDecoder` | 规范化外部文本中明确允许的展示痕迹，再执行与 `JsonDecoder` 相同的严格解码和资源准入 |
| `decode::NormalizingJsonDecodePolicy` / `NormalizingJsonDecodePolicyBuilder` | 选择允许执行的规范化操作，以及诊断信息采用脱敏还是详细模式；资源限制仍单独配置 |
| `decode::NormalizedJsonDocument` | 保存规范化后的文本，便于检查、借用式反序列化或重复解码，后续解码不会再次收取输入资源 |
| `decode::JsonDecodeError` 及诊断枚举 | 提供稳定的错误类别、处理阶段、顶层类型要求和语法原因，调用方无需解析错误消息 |
| `encode::JsonEncoder` | 将值序列化为严格、紧凑的 JSON 字节或写入 writer，同时限制输出和编码值所消耗的资源 |
| `value::JsonValueEncoder` | 不生成文本、不执行资源记账，直接把任意 `Serialize` 值投影为严格的 `serde_json::Value`；失败时提供粗粒度类别、精确原因和隐私安全的类型化细节 |
| `value::AccountingJsonValueSeed` | 从任意 Serde 反序列化器构造 `serde_json::Value`，并把解码值用量暂存到调用方持有的事务中 |
| `value::DuplicateKeyRejectingJsonValue` / `DuplicateKeyRejectingJsonValueSeed` | 构造 JSON 值时递归拒绝对象中的重复键 |
| `value::traverse::JsonTreeBudgetTracker` | 使用内部持有且可复用的值预算，对完整的已构造 JSON 树执行资源记账 |
| `value::traverse::JsonTreeReader` / `JsonTreeVisitor` | 以非递归、预算感知的方式只读遍历 JSON 树，并向访问器提供节点深度和位置上下文 |
| `value::traverse::JsonTreeMutator` / `JsonTreeMutVisitor` | 在独立的输入、输出事务之间非递归地原地修改 JSON 树；通过 `JsonTreeControl` 控制是否继续回调子节点 |

限制编码输出大小及值结构：

```rust
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncoder;

let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
    .max_output_bytes(64)
    .max_depth(4)
    .max_nodes(8)
    .build();
let mut encoder = JsonEncoder::with_limits(limits);
let bytes = encoder
    .to_vec(&serde_json::json!({"ok": true}))
    .expect("该值应满足配置的资源限制");
assert_eq!(bytes, br#"{"ok":true}"#);
```

处理值构造失败时，无需解析展示文本：

```rust
use qubit_json::value::JsonIntegerSignedness;
use qubit_json::value::JsonValueEncodeErrorCategory;
use qubit_json::value::JsonValueEncodeErrorKind;
use qubit_json::value::JsonValueEncoder;

let error = JsonValueEncoder::new()
    .encode(&u128::MAX)
    .expect_err("u128::MAX 超出严格 JSON 整数范围");
assert_eq!(error.category(), JsonValueEncodeErrorCategory::Number);
assert_eq!(
    error.kind(),
    JsonValueEncodeErrorKind::IntegerOutOfRange {
        signedness: JsonIntegerSignedness::Unsigned,
    },
);
assert!(error.is_number_error());
```

拒绝有歧义的重复键，并对已经构造的 JSON 树记账：

```rust
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use qubit_json::value::traverse::JsonTreeBudgetTracker;

let duplicate = serde_json::from_str::<DuplicateKeyRejectingJsonValue>(
    r#"{"role":"reader","role":"admin"}"#,
);
assert!(duplicate.is_err());

let mut tracker = JsonTreeBudgetTracker::new(
    JsonValueLimits::<JsonResource, usize>::builder()
        .max_depth(4)
        .max_nodes(8)
        .build(),
);
tracker
    .account(&serde_json::json!({"role": "reader"}))
    .expect("完整 JSON 树应满足配置的资源限制");
```

## 性能模型

只有资源检查可能改变结果时才支付其成本。未设置输出限制时，编码器直接写入自有字节向量，
同时保留值资源记账；值事务完全无限制时，树遍历跳过准入工作；有限事务仍执行原有检查并保持
错误语义。Criterion 基准分别覆盖这些路径：

```bash
cargo bench --bench budgeted_serde_json
cargo bench --bench tree_bench
```

`tree_bench` 会分别报告 unlimited 与 bounded 的只读和修改场景，使后续快路径改动可以同时核对
无保护路径的收益和受保护路径的成本，而不会被单个汇总结果掩盖。

## 明确边界

- 严格准入会检查 JSON 语法和文档规定的数字范围，但不会自动要求对象键唯一。若唯一性属于
  输入契约，应选择 `DuplicateKeyRejectingJsonValue` 等目标类型。
- 负整数须能装入 `i64`，非负整数须能装入 `u64`，小数或指数形式须能表示为有限 `f64`。
  更宽的整数以及不能接受二进制舍入的精确十进制值，应使用字符串或领域类型。
- 所有不可信边界都应设置有限资源上限。`unlimited()` 只应用于可信输入，或已由其他层完成
  准入的数据；外层输出上限不能替代解析前的输入与结构准入。
- 诊断信息默认脱敏。严格解码器通过 `JsonDecoder::with_diagnostic_policy` 配置，规范化
  解码器通过其规范化策略配置。只有在输入派生信息可以安全保留和记录时，才启用
  `DiagnosticPolicy::Detailed`。

## 延伸阅读

- [中文用户手册](doc/user_guide.zh_CN.md) ·
  [English user guide](doc/user_guide.md)
- [JSON 数字契约](doc/number_contract.zh_CN.md) ·
  [JSON number contract](doc/number_contract.md)
- [API 文档](https://docs.rs/qubit-json/0.8.0/qubit_json/)

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
