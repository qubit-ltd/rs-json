# JSON 基准测试指南

[English](benchmark_baseline.md)

本文介绍当前 checkout 的基准测试目标与比较方法。测量结果取决于硬件、输入和启用的限制。

## 基准目标

| 目标 | 覆盖范围 |
| --- | --- |
| `decoder_bench` | 严格与规范化解码、UTF-8 入口、下游形态输入、控制字符和失败路径 |
| `budgeted_serde_json` | 与直接使用 `serde_json` 的编解码对照、多种输入规模及独立预算模式 |
| `tree_bench` | 大数组、大对象和深层树的迭代读取与修改 |
| `jcs_bench` | 大对象、深层对象和大数组的 JCS 编码，比较有限/无限配置及直接 canonicalizer 对照；`jcs-depth` 测量单子节点数组的多种深度 |
| `decoder_bench` / `jcs_decode` | JCS 文本解码：数字数组、嵌套对象、普通/转义键和重复键拒绝；区分有限/无限预算及 fresh/reused session |

在仓库根目录运行：

```bash
cargo bench --bench decoder_bench
cargo bench --bench budgeted_serde_json -- --noplot
cargo bench --bench tree_bench -- --noplot
cargo bench --features jcs --bench jcs_bench -- --noplot
cargo bench --features jcs --bench decoder_bench -- jcs_decode --noplot
JCS_ALLOCATION_DIAGNOSTICS=1 cargo bench --features "jcs jcs-allocation-diagnostics" --bench jcs_bench -- --noplot
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

`decoder_bench` 中的 `jcs_decode` 组覆盖 1 KiB 和 64 KiB 数字数组、64 层对象、普通/转义键写法以及重复键拒绝。
计时前会断言成功 fixture 确实解码成功。有限和无限配置都分别测量 fresh decoder；reused session 场景在同一 decoder 中连续成功解码
8 次，每个 Criterion batch 都重新创建 session。因此用量会在一个 batch 内累计，但不会跨 Criterion 样本耗尽。

JCS 编码目标覆盖三种 `Value` 形态：成员很多的对象（排序表开销）、深层对象（遍历栈开销）
以及大数组（值节点多但排序状态少）。它在相同 fixture 上比较有限的值、输出和工作内存限制，
`JcsEncoder::unlimited`，以及直接使用 `serde_json_canonicalizer` 的对照路径。canonicalizer 对照测量其完整输出路径，
不提供 JCS 编码器的工作内存边界。报告结果时应记录 fixture 规模和这三项限制。

第一条命令只启用 `jcs` feature，Criterion 计时使用系统分配器，不包含分配计数器。第二条命令额外
启用 `jcs-allocation-diagnostics` 并设置 `JCS_ALLOCATION_DIAGNOSTICS`；它为每个 fixture 的有限 JCS、无限 JCS 和 canonicalizer 对照路径各打印一条
`JCS_ALLOC` 诊断，并且不注册 Criterion 计时样本。诊断仅包围一次操作，统计成功的全局分配器调用
（`alloc`、`alloc_zeroed` 和 `realloc`）及其请求字节数总和。这些一次性数据包括编码器状态、排序表、
临时存储和输出缓冲，是总分配请求量，不是辅助工作内存、峰值存活内存或进程 RSS。预先构造的输入
`Value` 和诊断文本格式化均不计入操作测量。

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

定向采样应保留 Criterion 给出的估计值和置信区间，并记录机器、Rust 版本、依赖解析结果、fixture 大小、预算配置和完整命令。
如果没有同机、同 fixture 的修改前基线，当前单次采样只能描述本次环境，不能据此声称性能提升或退化。这些 fixture 不测分配次数或峰值内存。

## 结果解释边界

严格编解码会执行直接 `serde_json` 不一定承担的检查。尤其是 `RawValue` 的直接对照信任
已经构造的值，而 `JsonEncoder` 会重新验证原始 JSON。差值用于测量契约成本，不能作为
删除验证的依据。

优化必须保留数字范围、重复键拒绝、输入与值记账、writer 错误顺序和部分输出行为。
数字检查应覆盖 `i64`/`u64` 边界、范围拒绝和 `NumberBytes` 拒绝。无限制结果仅适用于
可信数据或已由其他层准入的数据。详见[设计文档](json_design.zh_CN.md)和
[数字契约](number_contract.zh_CN.md)。
