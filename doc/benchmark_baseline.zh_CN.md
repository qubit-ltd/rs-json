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
| `unique_decode_bench` | 配置对象、元数据字符串 map 和 wire envelope 的唯一键解码；规模约 1 KiB、64 KiB、1 MiB；比较动态值、类型化目标、typed seed、普通 decoder 和直接 Serde |
| `unique_decode_allocations` | 对相同路径及重复键拒绝单独统计分配请求，不注册 Criterion 计时样本 |

在仓库根目录运行：

```bash
cargo bench --bench decoder_bench
cargo bench --bench budgeted_serde_json -- --noplot
cargo bench --bench tree_bench -- --noplot
cargo bench --features jcs --bench jcs_bench -- --noplot
cargo bench --features jcs --bench decoder_bench -- jcs_decode --noplot
JCS_ALLOCATION_DIAGNOSTICS=1 cargo bench --features "jcs jcs-allocation-diagnostics" --bench jcs_bench -- --noplot
cargo bench --locked --bench unique_decode_bench -- --noplot
cargo bench --locked --bench unique_decode_allocations
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

唯一键目标使用三种自有 fixture：配置对象、字符串元数据 map，以及包含版本和记录列表的 wire envelope。
预算按实际 UTF-8 字节数和源节点数配置；每种形态都有约 1 KiB、64 KiB、1 MiB 的成功输入，并单独测量重复键拒绝。
Criterion 会比较唯一键动态包装器、类型化 decode、typed seed、普通 `JsonDecoder` 和直接 `serde_json`。
直接对照不提供全局重复键拒绝契约。有限预算覆盖输入与值资源；复用场景每个 batch 都新建 decoder，再连续成功解码
8 份文档，并为整批累计用量预留足够额度。

延迟目标使用系统默认分配器。独立分配目标输出 `UNIQUE_DECODE_ALLOC` 记录，包含形态、字节数、路径、预算、模式、
分配次数、请求字节总量和 realloc 次数。realloc 请求计入请求字节总量并单独计数，不代表新增存活字节。
计数器测量操作期间成功的分配请求流量，不表示峰值存活内存、精确辅助工作内存或 RSS。fixture 准备、预热、结果检查和
格式化输出均在测量窗口之外。

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

+### 有限 JCS 栈增长观测（2026-10-26）

使用现有单子数组深度 fixture，在 revision
`44b70043ef6ded7de68bd6fd6e22581e1b720fef` 上测量；环境为 Rust 1.94.0、Intel Core
i5-9600K、Ubuntu 24.04.4（Linux 7.0.0-38-generic）及 Rust 的 `System` 分配器。CPU
调频策略为 `powersave`；可用 CPU 为 0–5，未设置 CPU 亲和性。锁定依赖包括
`serde_json 1.0.154`、`serde_json_canonicalizer 0.4.1`、`qubit-budget 0.10.0` 和
`criterion 0.8.2`；`Cargo.lock` 的 SHA-256 为
`ae1dfa7427aef55f7c6cd96ea3661d274070d1c6a8de2051e84a4a369cb78a52`。有限 fixture 使用
1 MiB 工作内存上限，并将输出字节、深度和节点数限制为精确值。Criterion 每个场景使用默认的
100 个样本。执行命令：

```bash
cargo bench --locked --features jcs --bench jcs_bench -- jcs-depth --noplot --save-baseline rj002-before
JCS_RENDERER_ALLOCATION_DIAGNOSTICS=1 cargo bench --locked --features 'jcs jcs-allocation-diagnostics' --bench jcs_bench -- --noplot
cargo bench --locked --features jcs --bench jcs_bench -- jcs-encode --noplot --save-baseline rj002-before
```

| 深度 | 无限配置估计值（区间） | 有限配置估计值（区间） | 栈增长次数，无限 / 有限 | 有限配置累计请求栈字节数 |
| ---: | ---: | ---: | ---: | ---: |
| 32 | 1.4781 µs（1.3486–1.6258） | 3.4467 µs（3.3876–3.5111） | 5 / 32 | 16,896 |
| 128 | 3.0330 µs（3.0160–3.0521） | 11.908 µs（11.728–12.163） | 7 / 128 | 264,192 |
| 512 | 11.420 µs（11.176–11.721） | 93.461 µs（80.569–107.09） | 9 / 512 | 4,202,496 |
| 2048 | 140.08 µs（120.90–165.33） | 449.09 µs（398.87–501.24） | 11 / 2048 | 67,141,632 |
| 8192 | 209.08 µs（197.12–221.99） | 848.90 µs（769.90–928.19） | 13 / 8192 | 1,073,872,896 |

在 8192 层场景中，两种路径观测到的最大实际栈容量均为 262,144 字节。有限配置的累计
请求字节数是多次增长请求的总和；它不代表峰值/存活内存，也不代表实际搬移字节数。有限
与无限配置的端到端计时还包含不同的预算准入工作。代表性有限/无限估计值分别为：大对象
1.4002/1.0836 ms、深对象 83.323/48.029 µs、大数组 1.9309/1.4695 ms。这些是描述性
观测，不是前后版本对比，也不能证明延迟差异仅由栈增长造成。由于预留栈容量可能占用稍后
对象成员表所需的预算，回收余量还需要与 renderer 准入逻辑协调，因此保留了有限配置的
精确容量策略。

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

唯一键测量遵循相同边界：缺少匹配的修改前结果时，只报告当前 revision 的观测，不从不同 fixture 或机器的对比推断性能提升或退化。

## 结果解释边界

严格编解码会执行直接 `serde_json` 不一定承担的检查。尤其是 `RawValue` 的直接对照信任
已经构造的值，而 `JsonEncoder` 会重新验证原始 JSON。差值用于测量契约成本，不能作为
删除验证的依据。

优化必须保留数字范围、重复键拒绝、输入与值记账、writer 错误顺序和部分输出行为。
数字检查应覆盖 `i64`/`u64` 边界、范围拒绝和 `NumberBytes` 拒绝。无限制结果仅适用于
可信数据或已由其他层准入的数据。详见[设计文档](json_design.zh_CN.md)和
[数字契约](number_contract.zh_CN.md)。
