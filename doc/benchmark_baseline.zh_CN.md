# JSON 基准测试指南

[English](benchmark_baseline.md)

本文介绍当前 checkout 的基准测试目标与比较方法。测量结果取决于硬件、输入和启用的限制。

## 基准目标

| 目标 | 覆盖范围 |
| --- | --- |
| `decoder_bench` | 严格与规范化解码、UTF-8 入口、下游形态输入、控制字符和失败路径 |
| `budgeted_serde_json` | 与直接使用 `serde_json` 的编解码对照、多种输入规模及独立预算模式 |
| `tree_bench` | 大数组、大对象和深层树的迭代读取与修改 |

在仓库根目录运行：

```bash
cargo bench --bench decoder_bench
cargo bench --bench budgeted_serde_json -- --noplot
cargo bench --bench tree_bench -- --noplot
```

通过过滤器缩小测量范围：

```bash
cargo bench --bench budgeted_serde_json -- decode --noplot
cargo bench --bench budgeted_serde_json -- encode --noplot
cargo bench --bench budgeted_serde_json -- encode --quick
```

`--quick` 只用于初步筛选候选，正式结论需完整采样确认。Criterion 结果在本地生成，位于
`target/criterion/`。

## 比较矩阵

带预算的编解码基准使用约 1 KiB、64 KiB 和 1 MiB 的文档。解码区分直接使用 `serde_json`、
自有会话、借用会话、复用会话、值限制和规范化解码。编码包含：

| 模式 | 用途 |
| --- | --- |
| `strict-only` | 无预算限制的严格序列化 |
| `value-only` | 带值资源限制的严格序列化 |
| `output-only` | 带输出字节限制的严格序列化 |
| `full` | 同时启用值限制与输出限制 |
| `strict-owned` / `strict-only` | 比较新建和复用无限制编码器的成本 |
| `incremental-writer` / `incremental-output-only` | 比较无限制与有输出限制的增量写入 |
| `numeric`、`string`、`raw-value` | 不同数据形态的成本，均有相同形态的直接 `serde_json` 对照 |

tree 基准区分纯 visitor 遍历、无限制与有限制的 reader，以及 mutator 输入和输出限制的
四种组合。

## 测量步骤

1. 记录代码版本与未提交改动、Rust 工具链、依赖解析结果、硬件、操作系统、输入形态、
   限制配置和实际命令。
2. 在同一台机器上用相同 fixture 和 Criterion 参数比较修改前后，保持 CPU 亲和性与电源
   设置一致。
3. 确认成功场景实际准入成功。每次受限操作新建会话，或在复用前重置，避免累计用量改变
   被测路径。
4. 报告延迟、吞吐、置信区间与相同形态的直接对照。重新采样当前代码，不把旧的本地
   Criterion 结果视为当前实现基线。
5. 局部优化后复查有代表性的有限制和无限制场景。增加热路径设计复杂度前，先取得性能
   剖析证据。

Linux 安装了 `taskset` 时，可固定到一个可用 CPU，例如：

```bash
taskset -c 3 cargo bench --bench budgeted_serde_json -- --noplot
taskset -c 3 cargo bench --bench tree_bench -- --noplot
```

## 结果解释边界

严格编解码会执行直接 `serde_json` 不一定承担的检查。尤其是 `RawValue` 的直接对照信任
已经构造的值，而 `JsonEncoder` 会重新验证原始 JSON。差值用于测量契约成本，不能作为
删除验证的依据。

优化必须保留数字范围、重复键拒绝、输入与值记账、writer 错误顺序和部分输出行为。
数字检查应覆盖 `i64`/`u64` 边界、范围拒绝和 `NumberBytes` 拒绝。无限制结果仅适用于
可信数据或已由其他层准入的数据。详见[设计文档](json_design.zh_CN.md)和
[数字契约](number_contract.zh_CN.md)。
