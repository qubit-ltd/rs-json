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
`JcsEncoder::unlimited`，以及旧 `serde_json_canonicalizer` 路径。canonicalizer 对照测量其完整输出路径，
不提供新版的工作内存边界。报告结果时应记录 fixture 规模和这三项限制。

第一条命令只启用 `jcs` feature，Criterion 计时使用系统分配器，不包含分配计数器。第二条命令额外
启用 `jcs-allocation-diagnostics` 并设置 `JCS_ALLOCATION_DIAGNOSTICS`；它为每个 fixture 的有限 JCS、无限 JCS 和旧路径各打印一条
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

## JCS 解码定向采样记录

2026-10-22 在 0.12 之前的基线提交 `9d821142ed844f25196b52a609eda45eeacefd35` 对
`jcs_decode/numbers_1k/unlimited/fresh/decoder_per_decode` 完成一次定向采样：

- 估计值为 38.014 µs，Criterion 区间为 37.633–38.607 µs。
- 吞吐量为 25.714 MiB/s，区间为 25.320–25.975 MiB/s。
- Criterion 使用 1 秒预热、1 秒测量和 10 个样本；报告 1 个轻微高值离群点。
- fixture 是生成的 1,024 字节数组，包含 512 个整数。配置为
  `JcsDecoder::unlimited()`，每次操作新建 decoder。
- 环境：Intel Core i5-9600K（6 核，3.70 GHz）、x86_64 Linux
  7.0.0-38-generic、Rust/Cargo 1.94.0，使用锁定的依赖版本。未设置 CPU 亲和性，
  主机未隔离且启用了 CPU 频率调节。系统未安装 Gnuplot，Criterion 使用 Plotters 后端。

命令：

```bash
cargo bench --manifest-path /data/working/qubit/rust-common/.worktrees/rs-json/implementation-20261022/Cargo.toml --features jcs --bench decoder_bench -- jcs_decode/numbers_1k/unlimited/fresh --noplot --sample-size 10 --measurement-time 1 --warm-up-time 1
```

这是 0.12 之前基线提交的一次观测，不是 0.12 候选的测量，也没有修改前后对照。本轮没有运行 0.12 benchmark，不能据此得出性能结论；该采样也未测分配次数或峰值内存。
该 fixture 不代表转义字符串、大对象、有限预算、复用 session 或应用层工作负载。
## JCS 深链采样记录（2026-10-23）

`jcs-depth` 组测量单子节点数组链，叶子为 `null`，深度分别为 32、128、512、2,048 和 8,192。
fixture 在计时外迭代构建，与预期输出字节比较，并逐层迭代拆解。比较 `JcsEncoder::unlimited()` 和有限预算：
`max_depth = depth + 1`、`max_nodes = depth + 1`、`max_output_bytes = 2 * depth + 4`，辅助工作内存上限沿用 1 MiB。
输出长度分别为 68、260、1,028、4,100 和 16,388 字节。

环境：基线版本 `ae26c342c66025302ed27f8e7f6031fdc90994d1`，实施 worktree 有未提交改动；Rust/Cargo 1.94.0；
x86_64 Linux 7.0.0-38-generic；Intel Core i5-9600K，6 核、3.70 GHz。未固定 CPU 亲和性，CPU 频率调节开启，主机未隔离。
因未安装 Gnuplot，Criterion 使用 Plotters 后端。benchmark profile 启用优化、LTO 和单 codegen unit；计时使用系统分配器，
没有启用分配计数器。依赖版本锁定。

每批计时使用 20 个样本、1 秒预热和 1 秒测量。下表是 Criterion 输出的时间区间；三批分别以独立命令和 baseline 运行。
批次之间存在明显波动，反映主机未隔离带来的影响。

| 深度 | 路径 | 第 1 批（µs） | 第 2 批（µs） | 第 3 批（µs） | 成功 realloc 次数 | 累计 realloc 请求字节 | 输出字节 |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 32 | unlimited | 6.0173–7.6997 | 3.5546–4.6375 | 4.1912–5.9725 | 35 | 17,104 | 68 |
| 32 | finite | 7.1184–10.820 | 5.0491–5.8028 | 4.8082–5.6065 | 40 | 31,984 | 68 |
| 128 | unlimited | 8.9541–12.204 | 30.351–41.185 | 25.774–38.384 | 133 | 265,168 | 260 |
| 128 | finite | 27.198–39.262 | 22.775–35.555 | 22.287–31.982 | 140 | 326,128 | 260 |
| 512 | unlimited | 67.779–94.772 | 66.182–78.464 | 65.233–93.017 | 519 | 4,206,544 | 1,028 |
| 512 | finite | 62.206–91.545 | 60.150–93.850 | 99.432–150.00 | 528 | 4,451,824 | 1,028 |
| 2,048 | unlimited | 234.93–318.13 | 257.22–344.40 | 188.67–253.32 | 2,057 | 67,157,968 | 4,100 |
| 2,048 | finite | 412.47–502.65 | 385.67–483.98 | 167.52–268.07 | 2,068 | 68,140,528 | 4,100 |
| 8,192 | unlimited | 962.63–1,724.8 | 865.99–1,183.2 | 480.06–659.14 | 8,203 | 1,073,938,384 | 16,388 |
| 8,192 | finite | 858.12–1,172.3 | 1,764.8–2,698.7 | 1,080.8–1,532.2 | 8,216 | 1,077,870,064 | 16,388 |

计时命令分别将 `review-depth-1`、`review-depth-2`、`review-depth-3` 作为 `--save-baseline` 值；分配诊断单独运行：

```bash
cargo bench --bench jcs_bench --features jcs --locked -- jcs-depth --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline review-depth-1
JCS_ALLOCATION_DIAGNOSTICS=1 cargo bench --bench jcs_bench --features "jcs jcs-allocation-diagnostics" --locked -- jcs-depth
```

分配诊断与所有计时批次分开运行。统计成功的 `realloc` 次数，并累计其新请求大小；范围是整个编码操作，包含输出和准入状态分配。
这些数字不是实际搬迁字节、峰值存活内存、RSS，也不是 renderer 栈的独立测量。

realloc 次数大致随深度增长，而累计请求字节增长明显更快：深度 8,192 时，16,388 字节输出对应约 1.07 GB 的 realloc 请求总量。
该 fixture 中观察到超线性的分配请求流量，值得另行定位来源；但它不能证明实际搬迁了多少字节，也不能确认由哪个容器导致。
计时区间在不同批次间波动，无法隔离算法原因。本测量不构成修改生产扩容策略的依据；本轮未改生产扩容策略。

这是修改候选的一次测量，没有修改前对照。`jcs-depth` 基准和诊断仍可用于后续测量。

## 结果解释边界

严格编解码会执行直接 `serde_json` 不一定承担的检查。尤其是 `RawValue` 的直接对照信任
已经构造的值，而 `JsonEncoder` 会重新验证原始 JSON。差值用于测量契约成本，不能作为
删除验证的依据。

优化必须保留数字范围、重复键拒绝、输入与值记账、writer 错误顺序和部分输出行为。
数字检查应覆盖 `i64`/`u64` 边界、范围拒绝和 `NumberBytes` 拒绝。无限制结果仅适用于
可信数据或已由其他层准入的数据。详见[设计文档](json_design.zh_CN.md)和
[数字契约](number_contract.zh_CN.md)。
