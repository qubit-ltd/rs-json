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

本 checkout 对应的 `0.12` 版本：

```toml
[dependencies]
qubit-json = "0.12"
qubit-budget = { version = "0.9.1", features = ["json"] }
serde_json = "1.0"
```

本地 checkout：

```toml
[dependencies]
qubit-json = { version = "0.12", path = "../rs-json" }
qubit-budget = { version = "0.9.1", features = ["json"] }
serde_json = "1.0"
```

如果需要解码到通过 derive 生成的应用类型，再添加
`serde = { version = "1.0", features = ["derive"] }`。

要生成符合 RFC 8785 的规范化字节，请显式开启 `jcs` feature：

```toml
qubit-json = { version = "0.12", features = ["jcs"] }
```

## 快速开始：解码已聚合且受大小限制的 HTTP body

下面的示例处理的是传输层已设置大小上限并聚合完成的 HTTP body 字节。它接收一个包含完整
`u64` 范围标识符的 JSON，再验证超出 JSON 输入上限的内容会在解码结果提交前被拒绝：

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
错误类别。上面的切片产生之前，传输层必须先限制 body 大小再进行聚合。应用调用顺序是：传输层
有界聚合 → `JsonDecoder` 准入 JSON → 模式、权限和领域校验。

`qubit-json` 不提供 HTTP body 聚合 API。需要查看下游 HTTP 接入方式时，可参考
[`rs-web` controller 示例](https://github.com/qubit-ltd/rs-web/blob/main/examples/controller_crud.rs)；该链接是可能随下游仓库变化的参考示例。更多输入方式见[用户手册的 `decode_input` 章节](doc/user_guide.zh_CN.md#进阶用法)。

## 选择解码边界

选择入口时应以输入契约为准，而不是看某份数据能否碰巧解析成功：

| 输入契约 | API |
| --- | --- |
| 输入必须已经是完整、严格的 JSON | `JsonDecoder` |
| 从字节输入源读取到 EOF，并生成拥有所有权的结果 | `JsonDecoder::decode_input` |
| 严格解码前允许清理指定的展示痕迹 | 使用显式策略构造的 `NormalizingJsonDecoder` |
| 需要检查规范化结果、重复解码，或让结果借用规范化文本 | 先调用 `NormalizingJsonDecoder::prepare_str` / `prepare_utf8`，再由 `NormalizingJsonDecoder::decode_precharged_document`、`decode_precharged_object_document` 或 `decode_precharged_array_document` 解码 |

默认规范化策略可以清理首尾空白、一个 BOM、一层 JSON Markdown 围栏，以及字符串内未经转义
的 ASCII 控制字符。这只是受控规范化，不是另一种 JSON 方言；注释、尾随逗号、未加引号的键
和缺失的 JSON 语法仍会被拒绝。

## 为签名生成规范化字节

如果应用需要为哈希或签名生成稳定字节，应开启 `jcs`，并把已经构造好的
`serde_json::Value` 交给 `JcsEncoder`：

```rust
use qubit_budget::json::{JsonEncodeLimits, JsonResource};
use qubit_json::encode::JcsEncoder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(4096)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    let value = serde_json::json!({"b": 2, "a": 1.0});
    assert_eq!(encoder.to_vec(&value)?, br#"{"a":1,"b":2}"#);
    Ok(())
}
```

`jcs` 默认关闭。规范化对象是 `Value` 数据模型，不接受任意 `Serialize` 值或原始 JSON 文本。
用普通解码器处理原始文本时，若契约要求拒绝重复对象键，必须在构造 `Value` 前完成检查；一旦键已合并，
就无法再找回重复项。`JcsDecoder` 会在物化期间执行这项检查。
JCS 数值遵循 IEEE 754 双精度语义，因此 `9007199254740993` 可能输出为
`9007199254740992`。若协议必须保留整数的每一位十进制数字，应将整数编码为 JSON 字符串。
普通 `JsonDecoder` 也不接受所有 JCS 数字词法形式作为原始文本。

需要从原始文本开始时，可在 `jcs` feature 下使用 `JcsDecoder`。它返回自有
`serde_json::Value`，把所有数字舍入为有限 binary64，并拒绝反转义后重复的对象键。有限预算示例
以及舍入、Unicode、深度和输入缓冲边界见[用户手册](doc/user_guide.zh_CN.md#按-jcs-规则解码原始文本)。
`JcsDecoder` 解析输入但不验证输入字节是否已经规范化；规范输出仍由 `JcsEncoder` 生成。

## 0.12 迁移指南

0.12 严格处理 `serde_json::RawValue` 与 `serde_json::Number` 使用的私有 Serde 协议。
[迁移指南](doc/migration_0_12.zh_CN.md)说明新增的 `InvalidNumberProtocol` 状态错误、预算类型身份变化及下游版本调整。

## 为什么需要这个项目

语法正确的 JSON 仍可能过大、嵌套过深，或在构造内存对象时消耗过多资源。`qubit-json` 保留
JSON 语法和 Serde 兼容性，并允许调用方限制原始及规范化输入、嵌套深度、节点数、集合大小、
键、字符串、数字、有效载荷和编码输出。跨调用累计的用量及其提交边界由 `qubit-budget` 的会话
和事务明确表达。

## 核心能力

| 领域 | 适用场景 | 明确边界 |
| --- | --- | --- |
| `decode` | 严格准入 JSON、按策略规范化文本，或用 `JcsDecoder` 解码 JCS 数字 | 普通解码保留整数范围；JCS 解码采用 binary64 并拒绝重复键 |
| `encode` | 生成受预算约束的严格 JSON | 完整序列化成功后才提交值资源用量，但 I/O 失败仍可能在外部写入器中留下部分字节 |
| `value` | 从 Serde 事件构造受预算约束的 `serde_json::Value` | Serde seed 看不到数字的原始文本，也不能执行文本级数值范围检查 |
| `value::traverse` | 迭代读取或原地修改已构造的值树 | 修改按步骤生效；新增后代的访问器要自行限制工作量，访问器或输出预算失败会保留此前的改动 |

`qubit-budget` 负责限制、资源标识、预算和会话；`qubit-json` 负责 JSON 规范化、词法校验、
文本编解码、值构造和遍历。

JCS 编码中，`Budget` 表示值或输出限制拒绝了操作；`WorkingMemory` 表示单独配置的辅助容量上限不足，
该上限通过 `JcsEncoder::with_limits(limits, max_working_bytes)` 设置。
值准入发生在渲染之前，准入遍历栈不受这项工作内存限制约束。需要限制这段遍历时，应同时配置
`max_depth` 和 `max_nodes`。工作内存限制约束的是 renderer 集合容量，不是精确的堆内存上限。

## 明确边界

- 普通严格准入会检查 JSON 语法和数字范围，但不会自动要求对象键唯一。若唯一性属于输入契约，
  可使用 `DuplicateKeyRejectingJsonValue`；JCS 原始文本路径则使用 `JcsDecoder`。
- 普通文本解码和编码要求负整数装入 `i64`、非负整数装入 `u64`；`JcsDecoder` 则将所有数字舍入为
  有限 `f64`。例如 `9007199254740993` 会变成 `9007199254740992`。要求精确保留各位数字时，使用字符串或领域类型。
- `JsonValueEncoder` 只识别 `serde_json::Number` 对应的精确私有 Serde struct 协议；普通 map 即使字段同名也仍是对象。格式错误的 `RawValue` 与 `Number` 协议分别返回 `JsonSerializerStateError::InvalidRawValueProtocol` 和 `InvalidNumberProtocol`。seed 只看到原生数字事件，无法从普通 map 猜出私有协议来源。
- 固定的 127 层容器保护同时适用于 JCS 解码和 `JsonValueEncoder` 对真实 `RawValue` 的投影；它独立于可配置、包含根值的 `max_depth`。`InvalidRawValue` 表示严格物化契约拒绝输入，原因不只限于语法错误。普通 newtype 保持透明；需要按原始 JSON 物化时应使用 `serde_json::value::RawValue`。
- 树遍历按现有 `serde_json::Number` 的紧凑显示长度计量。启用 `arbitrary_precision` 时，树读取器可计量已保存的宽数字（例如 `1e400`）；普通和 JCS 编码器仍按各自规则拒绝超出有限范围的数字。
- 所有不可信边界都应设置有限资源上限。`unlimited()` 会同时取消值、输出和工作内存限制，
  只应用于可信输入或已由其他层完成准入的数据；外层输出上限不能替代解析前的输入与结构准入。
- 严格解码和规范化解码的切片入口都会处理传入的完整 `&str` 或 `&[u8]`；输入字节限制只准入该切片，
  无法追溯限制传输层或 body 聚合器已经分配的内存。`JsonDecoder` 的切片方法可以返回借用输入的结果。
  一次性 `NormalizingJsonDecoder` 方法要求目标实现 `DeserializeOwned`，因为规范化文本可能是临时数据。
  若需在规范化后借用文本，应先准备 `NormalizedJsonDocument` 再解码，借用结果的生命周期不能超过该文档。
  `JsonDecoder::decode_input` 仍会从 `Input<Item = u8>` 读取到 EOF，并返回拥有数据的 `DeserializeOwned` 结果；
  它会暂存整份文档，未设置 `max_input_bytes` 时这部分临时内存没有配置上限。解码前仍应在传输层限制读取或
  body 聚合大小。这个严格 JSON 入口不能用于原始文本的 JCS 规范化。
- `JcsDecoder::decode_input` 也会读取并缓冲完整输入；有限输入预算下可能多读取一个不计费且无法放回输入源的探测字节。
  词法预算不是堆内存上限，结果容器容量、分配器元数据和输入缓冲区容量不受精确约束。JCS Value 构造另有 127 层容器上限，
  即使无限预算也生效；第 128 层返回 `MaterializationDepthLimit`。它与包含根值的 `max_depth` 不同。重复键按反转义后的字符串比较，
  不做 Unicode 规范化；该 API 不验证规范字节。
- `JcsEncoder::write_buffered` 和 `write_incremental` 在后续写入或 flush 失败时，可能在外部目标留下已经接受的字节，
  这些字节会计入输出预算。增量模式直接写入，并在成功时 flush；值预算只在完整操作成功后提交。
  `max_working_bytes` 限制遍历栈和对象排序表的辅助容量，不限制输入 `Value`、分配器元数据或输出缓冲区容量。
- 诊断信息默认脱敏。严格解码器通过 `JsonDecoder::with_diagnostic_policy` 配置，规范化
  解码器通过其规范化策略配置。脱敏错误只保留稳定分类和源坐标，不保留意外字节、token、
  键/值文本或 parser/Serde 来源。只有在输入派生来源可以安全保留和记录时，才启用
  `DiagnosticPolicy::Detailed`。

## 延伸阅读

- [中文用户手册](doc/user_guide.zh_CN.md) ·
  [English user guide](doc/user_guide.md)
- [JSON 数字契约](doc/number_contract.zh_CN.md) ·
  [JSON number contract](doc/number_contract.md)
- [设计文档](doc/json_design.zh_CN.md) ·
  [Design documents](doc/json_design.md)
- [0.12 迁移指南](doc/migration_0_12.zh_CN.md) · [English migration guide](doc/migration_0_12.md)
- [基准测试指南](doc/benchmark_baseline.zh_CN.md) ·
  [Benchmark guide](doc/benchmark_baseline.md)
- [已发布 API 文档](https://docs.rs/qubit-json)；当前分支 API 请运行
  `cargo doc --all-features --open`

## 测试

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 运行测试
cargo test --all-features

# 运行独立 fixture 中的 Markdown 文档示例
cargo test --manifest-path tests/fixtures/documentation_examples/Cargo.toml --doc --locked

# 运行项目 CI 检查
./.infra/bin/ci-check.sh

# 检查代码覆盖率
./.infra/bin/coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./.infra/bin/align-ci.sh`格式化代码，运行`./.infra/bin/ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-json](https://github.com/qubit-ltd/rs-json)
