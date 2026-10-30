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
计时前会断言成功 fixture 确实解码成功，并在同一个 decoder 上完成全部八次 reused-session 预检；重复键预检会检查具体拒绝原因。
计时闭包只执行解码并消费结果，不检查成功状态。有限和无限配置都分别测量 fresh decoder；reused session 场景在同一 decoder 中连续解码
8 次，每个 Criterion batch 都重新创建 session。因此用量会在一个 batch 内累计，但不会跨 Criterion 样本耗尽。

唯一键目标使用三种自有 fixture：配置对象、字符串元数据 map，以及包含版本和记录列表的 wire envelope。
预算按实际 UTF-8 字节数和源节点数配置；每种形态都有约 1 KiB、64 KiB、1 MiB 的成功输入，并单独测量重复键拒绝。
Criterion 会比较唯一键动态包装器、类型化 decode、typed seed、普通 `JsonDecoder` 和直接 `serde_json`。
直接对照不提供全局重复键拒绝契约。有限预算覆盖输入与值资源；复用场景每个 batch 都新建 decoder，再连续成功解码
8 份文档，并为整批累计用量预留足够额度。

唯一键延迟目标会在注册测量前预检单次成功结果和完整 batch8 结果。重复键拒绝结果也会在计时前检查是否为 `DuplicateKey`，
计时闭包只执行解码并消费结果。本次测量边界调整不改变 fixture、预算、decoder 生命周期、结果析构位置或 throughput，也不改写历史样本。
新旧计时样本包含的校验工作不同，不能据此声称加速；本次没有重新采集性能基线。

延迟目标使用系统默认分配器。独立分配目标输出 `UNIQUE_DECODE_ALLOC` 记录，包含形态、字节数、路径、预算、模式、
分配次数、请求字节总量和 realloc 次数。realloc 请求计入请求字节总量并单独计数，不代表新增存活字节。
计数器测量操作期间成功的分配请求流量，不表示峰值存活内存、精确辅助工作内存或 RSS。fixture 准备、预热、结果检查和
格式化输出均在测量窗口之外。

唯一键基准还包含结构规模 fixture，与上面的字节规模 fixture 并列。结构样例覆盖 `wide_object` 和 `record_array`，分别使用
32、512、4096 个成员或记录；随着条目数增长，单个标量字符串仍保持短小。类型化路径会解码完整目标
（`BTreeMap<String, String>` 或 `Vec<StructuralRecord>`），动态路径会保留完整 `Value`。直接 `serde_json` 路径没有有限预算版本，
也不具备 decoder 的全局重复键拒绝契约。

可用以下命令筛选结构规模 Criterion 样例：

```bash
cargo bench --locked --bench unique_decode_bench -- unique_decode_structural --noplot
```

每种结构样例包含 fresh 操作和八次 batch。fresh 计时包含 decoder 创建和结果析构。batch 在 Criterion setup 中创建 decoder，
计时操作连续解码八次并析构结果。fixture 生成、完整结果预检和 batch decoder 创建都在计时窗口之外。有限预算 batch 的预检额度
按八份累计输入设置。

重复键拒绝样例使用宽对象，将同一个重复键分别放在较前位置或所有唯一成员之后。两类 fixture 都包含相同数量的唯一成员，
再附加一个重复成员；有限和无限预算下均检查错误原因为 `DuplicateKey`。

分配目标保留 `UNIQUE_DECODE_ALLOC`，并新增以下 schema 的记录：

```text
UNIQUE_DECODE_STRUCT_ALLOC,shape,items,bytes,path,budget,mode,allocation_count,allocated_request_bytes,reallocation_count
```

其中 `shape` 为 `wide_object`、`record_array`、`wide_object_reject_early` 或 `wide_object_reject_late`。batch8 报告八次解码的总量。
请求字节总量包含成功分配请求和重新分配请求，用于描述分配流量；它不等于峰值存活内存，也不等于实际搬移的字节数。

### 结构规模首次采样（2026-10-27）

首次结构规模采样对应 revision `5947f5d`，采样时没有未提交代码变更。执行命令：

```bash
cargo bench --locked --bench unique_decode_bench -- unique_decode_structural --noplot
```

Criterion 使用默认的 100 个样本、预热和测量设置。代表性的有限预算 `unique_typed` / `fresh` 场景如下：

| 形状 | 条目数 | 输入字节 | 延迟估计（95% 置信区间） | 分配次数 | 请求字节 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `wide_object` | 32 | 641 | 11.307 µs（11.232–11.396） | 77 | 6,904 |
| `wide_object` | 512 | 10,241 | 421.56 µs（392.66–448.78） | 1,195 | 108,448 |
| `wide_object` | 4096 | 81,921 | 2.6601 ms（2.6443–2.6773） | 9,557 | 877,016 |
| `record_array` | 32 | 929 | 17.047 µs（16.935–17.175） | 136 | 24,064 |
| `record_array` | 512 | 14,849 | 353.86 µs（337.10–375.93） | 2,060 | 381,184 |
| `record_array` | 4096 | 118,785 | 2.2605 ms（2.2441–2.2792） | 16,399 | 3,047,680 |

4096 个唯一成员时，有限预算 fresh 重复键拒绝场景的 early 延迟为 325.81 µs（323.73–328.74），late 为 2.3056 ms（2.2629–2.3583）。两者输入均为 81,943 字节；分配次数和请求字节分别为 11 / 1,204 与 8,881 / 497,510。

测量主机为 Intel Core i5-9600K、Ubuntu 24.04.4（Linux 7.0.0-38-generic）、Rust 1.94.0、Criterion 0.8.2、`serde_json` 1.0.154 和 `qubit-budget` 0.10.0。`Cargo.lock` SHA-256 为
`ae1dfa7427aef55f7c6cd96ea3661d274070d1c6a8de2051e84a4a369cb78a52`。使用系统分配器；CPU governor 为 `powersave`，可用 CPU 为 0–5，未设置 CPU 亲和性。采样期间主机上还有其他编译和审查任务。Criterion 提示部分场景采集 100 个样本需要超过 5 秒目标时间，随后仍完成了全部样本。该结果是共享主机上的描述性首次基线，不代表空闲机器结果，也不用于性能对比。

### 索引路径后续采样（2026-10-27）

修改前的路径样本采自 T1 检查点（`7dacc96`）的独立基线 worktree；修改后样本使用本 worktree 中已集成的对象键索引和路径调整。两次均使用 Rust 1.94.0、Criterion 0.8.2、相同 fixture、10 个样本，以及 0.1 秒预热和测量时间。修改后的延迟命令为：

```bash
cargo bench --locked --bench unique_decode_bench -- unique_decode_structural --sample-size 10 --warm-up-time 0.1 --measurement-time 0.1
```

下表列出 `wide_object` fresh、unlimited 路径的 Criterion 中心估计。长前缀键使用与修改前样本相同的条目数和数据。除特别标为 ms 外，单位均为微秒。

| 键样式 | 条目数 | 路径 | 修改前 | 修改后 |
| --- | ---: | --- | ---: | ---: |
| 短键 | 32 | `owned_value` | 11.54 µs | 19.27 µs |
| 短键 | 512 | `owned_value` | 576.93 µs | 1.146 ms |
| 短键 | 4096 | `owned_value` | 23.533 ms | 13.159 ms |
| 短键 | 32 | `validated_preserve` | 12.07 µs | 34.47 µs |
| 短键 | 512 | `validated_preserve` | 553.17 µs | 708.31 µs |
| 短键 | 4096 | `validated_preserve` | 24.625 ms | 27.072 ms |
| 短键 | 32 | `value_projection` | 8.10 µs | 14.69 µs |
| 短键 | 512 | `value_projection` | 628.08 µs | 608.39 µs |
| 短键 | 4096 | `value_projection` | 25.098 ms | 17.034 ms |
| 长公共前缀 | 32 | `owned_value` | 12.20 µs | 107.24 µs |
| 长公共前缀 | 512 | `owned_value` | 597.74 µs | 739.96 µs |
| 长公共前缀 | 4096 | `owned_value` | 26.273 ms | 13.131 ms |
| 长公共前缀 | 32 | `validated_preserve` | 12.24 µs | 105.42 µs |
| 长公共前缀 | 512 | `validated_preserve` | 587.80 µs | 379.51 µs |
| 长公共前缀 | 4096 | `validated_preserve` | 26.925 ms | 11.158 ms |
| 长公共前缀 | 32 | `value_projection` | 7.13 µs | 58.94 µs |
| 长公共前缀 | 512 | `value_projection` | 508.93 µs | 649.78 µs |
| 长公共前缀 | 4096 | `value_projection` | 26.510 ms | 6.339 ms |

这组 10 样本数据波动较大：多个 4096 项测量提示 0.1 秒目标时间不足，32 项长前缀区间尤其宽。因此表格仅供探索，不足以确认稳定的加速或退化。对应的分配命令为 `cargo bench --locked --bench unique_decode_allocations`。短键 `wide_object` 的 `unique_value` fresh 场景在 32 项报告 184 次分配 / 14,120 请求字节，512 项为 2,825 / 224,896，4096 项为 22,539 / 1,812,680。这些数据是分配请求总量，不是索引存活大小。`BTreeSet` 查重会增加 O(键总字节数) 的辅助存储及树节点开销；该存储不计入 JSON 值精确预算。主机仍为上文所述共享 Intel Core i5-9600K / Ubuntu 24.04.4 系统；修改后的 `Cargo.lock` SHA-256 为
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`。

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

### SafeJsonValue 桥接重复键索引（2026-10-27）

`safe_value_bridge` 组比较了将重复对象扫描替换为 `BTreeSet` 查找前后，直接通过
`serde_json` 反序列化为 `SafeJsonValue` 的耗时。对应命令为：

```bash
cargo bench --locked --bench decoder_bench -- safe_value_bridge --save-baseline rj001-before --noplot
cargo bench --locked --bench decoder_bench -- safe_value_bridge --baseline rj001-before --noplot
```

两次运行均使用 Rust 1.94.0、Criterion 0.8.2、默认 features，在前文描述的共享
Intel Core i5-9600K / Ubuntu 24.04.4 主机上每个 case 采样 100 次。fixture 构造和
语义检查不在计时闭包内；计时操作包含 decoder 创建及结果释放。下表列出具有
4096 个成员的代表性直接 Serde 场景。数值为 Criterion 中心估计；原始运行也输出了置信
区间，但仓库 CI 清理 `target/` 时一并删除了完整 Criterion 结果。后续重跑与共享主机上的
其他编译任务重叠，出现大量离群值，因此不用于替代以下观测。

| 键形式 | 场景 | 项数 | 输入字节 | 修改前（95% CI） | 修改后估计值 |
| --- | --- | ---: | ---: | ---: | ---: |
| 短键 | 唯一键 | 4096 | 52,139 | 85.253 ms（78.471–92.320） | 12.838 ms |
| 短键 | 尾部重复 | 4096 | 52,152 | 28.743 ms（27.711–29.995） | 2.951 ms |
| 长公共前缀 | 唯一键 | 4096 | 121,771 | 48.878 ms（44.369–53.792） | 6.534 ms |
| 长公共前缀 | 尾部重复 | 4096 | 121,801 | 35.167 ms（32.867–37.696） | 4.433 ms |

首次修改后采样还显示小对象成本：32 项时，直接 Serde 唯一键场景比修改前约慢
77–82%。之后的高噪声重跑在 4096 项对照路径上同时出现表观提升和退化。这些观测支持
本机宽对象场景的趋势，不构成普遍延迟保证。`JsonDecoder` 场景是对照，并未经过本次修改的
桥接路径。索引为每个解码后的键保存一次并按字符串顺序比较；查找成本取决于公共前缀，
辅助内存随键总字节数及树节点开销增长。

### Token formatter 输出限额下的动作存储（2026-10-27）

`token_format_limit` 组测量数组分别有 32、512、4096 项（输入 65、1,025、8,193 字节）时，
无限制格式化成功和输出限额为 8 字节时的 `OutputLimit`。修改前后命令为：

```bash
cargo bench --locked --bench budgeted_serde_json -- token_format_limit --save-baseline rj002-before --noplot
cargo bench --locked --bench budgeted_serde_json -- token_format_limit --baseline rj002-before --noplot
```

Criterion 在同一共享主机、Rust 1.94.0、Criterion 0.8.2 和默认 features 下，每个场景采样
100 次。Criterion 报告的代表性中心估计如下：

| 项数 | 输入字节 | 无限制修改前 → 修改后均值（µs；95% CI；变化率、95% CI） | 限额 8 修改前 → 修改后均值（µs；95% CI；变化率、95% CI） |
| ---: | ---: | ---: | ---: |
| 32 | 65 | 2.6466（2.6431–2.6504）→ 6.1948 µs（5.7068–6.6971；+134.07%，+116.23% 至 +154.33%） | 2.4360（2.4094–2.4660）→ 4.2857 µs（3.9061–4.6961；+75.93%，+60.22% 至 +92.13%） |
| 512 | 1,025 | 37.3556（36.7373–38.0618）→ 50.8842 µs（47.6950–54.2535；+36.22%，+27.06% 至 +45.00%） | 44.8948（41.9409–48.0430）→ 27.3277 µs（27.0175–27.6681；−39.13%，−43.20% 至 −34.75%） |
| 4096 | 8,193 | 523.0789（484.2250–563.7111）→ 294.0991 µs（290.3670–298.2947；−43.78%，−47.92% 至 −39.19%） | 441.2246（413.5106–470.0707）→ 358.1130 µs（330.5324–387.6673；−18.84%，−26.23% 至 −9.28%） |

该修改以测得的小输入回退换取宽输入下较低的测量成本。私有容量回归确认 `Some(0)` 不保留动作，
且超出输出限额后，动作长度和容量不会随后续 token 增长。这仅限制 planner 保存的动作存储，
不代表 RSS 或总内存上限。为验证语法并维持尾部语法错误优先级，仍会扫描完整输入，因此扫描时间和
输入存储仍随输入大小增长。

### 有限 JCS 栈增长观测（2026-10-26）

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

| 深度 | 无限配置估计值（报告置信区间） | 有限配置估计值（报告置信区间） | 栈增长次数，无限 / 有限 | 有限配置累计请求栈字节数 |
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

### 回收对象表后的 JCS 栈增长（2026-10-29）

`src/encode/internal/jcs_working_memory.rs` 中的内部测试
`test_diagnoses_stack_growth_after_reclaiming_wide_table` 使用
`jcs-allocation-diagnostics` 执行真实的 reserve、push、release 及后续栈增长。对每个目标深度
（128、512、2048），测试先建立有剩余容量的栈，再预留一个宽成员表；该表请求字节数与栈原容量
同时存活时会超过预算。测试确认栈容量被替换/回收，释放成员表，快照诊断计数器，然后增长深数组
分支。无回收对照从相同栈容量和长度开始，并使用相同预算。预算为
`depth * size_of::<[u64; 4]>() + 1024` 字节；成员表宽度根据预算与原栈容量之差，使用
`size_of::<[u64; 3]>()` 推导。两种场景都能在预算内完成。

旧策略数据复现的是基线版本
`48c4be653c3e342342327f46eb9e60b620e5a264` 的精确增长行为。由于诊断 fixture 是本次变更新增的，
复现时使用本文件中的 fixture，并临时将回收后的高水位赋值设为 `usize::MAX`，与基线策略的
`stack_growth_is_exact = true` 等价；采集时仅禁用了几何增长断言，以便输出三个深度的数据。环境为
`rustc 1.94.0 (4a4ef493e 2026-03-02)` 和锁定的依赖集合。完整输出：

```text
JCS_STACK_RECLAIM depth=128 budget=5120 live_stack_bytes=256 peak_with_table=4888 peak_after_deep_branch=4888 reclaim_replacements=1 post_growths=120 post_replacements=0 post_requested_bytes=263040 control_growths=4 control_requested_bytes=8128
JCS_STACK_RECLAIM depth=512 budget=17408 live_stack_bytes=256 peak_with_table=17176 peak_after_deep_branch=17176 reclaim_replacements=1 post_growths=504 post_replacements=0 post_requested_bytes=4201344 control_growths=6 control_requested_bytes=32704
JCS_STACK_RECLAIM depth=2048 budget=66560 live_stack_bytes=256 peak_with_table=66328 peak_after_deep_branch=66328 reclaim_replacements=1 post_growths=2040 post_replacements=0 post_requested_bytes=67140480 control_growths=8 control_requested_bytes=131008
```

另一次启用几何增长断言的运行在深度 128 停止，观察到回收后增长 120 次，并因超过上限而失败。
两次采集均使用以下命令：

```bash
cargo test --locked --lib --features 'jcs jcs-allocation-diagnostics' encode::internal::jcs_working_memory::tests -- --nocapture
```

新策略从 8 个存活元素和有剩余容量的栈开始；无回收对照也使用同一起点。历史需求高水位规则
允许仅对超过历史请求深度的需求进行几何增长。下表峰值来自新策略运行。未运行 Criterion 或墙钟计时。

| 深度 | 预算（B） | 新策略宽表期间峰值（B） | 新策略深分支后峰值（B） | 回收 replacement 次数 | 回收后增长次数，旧 / 新 | 回收后请求字节，旧 / 新（B） | 新策略无回收增长次数 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 128 | 5,120 | 4,888 | 4,888 | 1 | 120 / 5 | 263,040 / 8,928 | 4 |
| 512 | 17,408 | 17,176 | 17,408 | 1 | 504 / 7 | 4,201,344 / 35,552 | 6 |
| 2048 | 66,560 | 66,328 | 66,560 | 1 | 2,040 / 9 | 67,140,480 / 140,000 | 8 |

旧策略在回收后观察到随新增深度成比例增长的次数。使用高水位规则后，这些深度的增长次数呈对数
趋势，并接近同起点无回收对照。请求字节总量是成功增长事件的容量请求之和；它不代表复制字节、
分配器流量、存活内存或耗时。峰值列是该内部 fixture 观测到的栈与成员表跟踪容量。有限深分支
峰值达到预算但未超过。新策略通过了既有的精确边界、有限增长、逐渐增宽兄弟及祖先表保留回归。

复现命令：

```bash
cargo test --locked --lib --features 'jcs jcs-allocation-diagnostics' encode::internal::jcs_working_memory::tests -- --nocapture
```

这个有界合成 fixture 证明了容量增长回归及其修复；它不估算端到端渲染延迟，也不能证明实际
物理复制量。

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

## 重复键长尾与惰性 JSON 值读取帧（2026-10-29）

测量使用 Rust 1.94.0、`serde_json 1.0.154`、Criterion 0.8.2、系统分配器，以及 Linux
7.0.0-38-generic 上的 Intel Core i5-9600K。主机为共享环境，未设置 CPU 亲和性。唯一键
基准每项使用 Criterion 默认的 100 个样本；采样期间有其他 crate 的构建任务并行运行，
因此这些延迟只是观测值，不能作为受控的加速结论。启用和未启用 JCS feature 时，Criterion
为 `unique_seed` 使用同一组 benchmark ID，因此其自动跨次比较可能选到另一个 feature 的
最近样本。下表记录匹配 feature 的原始估计值与报告区间。

重复键解码命令：

```bash
cargo bench --locked --bench unique_decode_bench -- early_duplicate_tail
cargo bench --locked --bench unique_decode_bench --features jcs -- early_duplicate_tail
```

| Feature | 操作 | 尾部 | 修改前估计值（报告区间） | 修改后估计值（报告区间） |
| --- | --- | ---: | ---: | ---: |
| 默认 | unique seed | 1 KiB | 1.9063 µs（1.8943–1.9187） | 1.5736 µs（1.5269–1.6295） |
| 默认 | unique seed | 64 KiB | 66.148 µs（65.802–66.563） | 42.870 µs（41.908–43.934） |
| 默认 | unique seed | 1 MiB | 1.4596 ms（1.4013–1.5223） | 533.18 µs（530.50–536.13） |
| `jcs` | unique seed | 1 KiB | 2.0086 µs（1.9379–2.0889） | 1.8600 µs（1.8432–1.8774） |
| `jcs` | unique seed | 64 KiB | 80.856 µs（77.954–83.903） | 72.543 µs（70.221–75.326） |
| `jcs` | unique seed | 1 MiB | 1.0482 ms（1.0435–1.0531） | 1.0292 ms（1.0234–1.0351） |
| `jcs` | JCS decode | 1 KiB | 1.7619 µs（1.7091–1.8288） | 1.8726 µs（1.8217–1.9334） |
| `jcs` | JCS decode | 64 KiB | 77.683 µs（75.496–80.351） | 63.781 µs（63.422–64.200） |
| `jcs` | JCS decode | 1 MiB | 1.0396 ms（1.0365–1.0428） | 1.0085 ms（1.0043–1.0132） |

fixture 在靠前位置包含重复键，后面跟随合法字符串数据。首遍完整准入仍保留，以维持语法和
预算错误优先级；第二遍构树会在 builder 首次物化失败时停止。这些样本没有墙钟验收阈值；
JCS 的 1 KiB 结果略慢，测量也受到共享主机负载影响。

值读取分配二进制在计量操作前创建 fixture；Debug 输出写入丢弃 sink，或在接受 64 字节后
失败的 sink。realloc 也计入请求字节总量。次数和字节统计分配请求流量，不代表峰值存活内存
或 RSS。

```bash
cargo bench --locked --bench value_read_allocations
cargo bench --locked --bench value_read_allocations --features serde_json/preserve_order
```

默认 feature 下 owner 与 view 结果一致。新增帧前比较和 Debug 的请求量随宽度增长；修改后
所有三档宽度（32、512、8192）均报告相同请求：

| 形状 / 操作 | 修改前宽度 32 | 修改前宽度 8192 | 修改后，各宽度 |
| --- | ---: | ---: | ---: |
| 数组比较（相等或不等） | 2 次 / 528 B | 2 次 / 131,088 B | 1 次 / 320 B |
| 对象比较（相等或不等） | 5 次 / 976 B | 13 次 / 262,096 B | 1 次 / 320 B |
| 数组 Debug（成功或 writer 提前失败） | 6 次 / 3,000 B | 14 次 / 786,360 B | 2 次 / 400 B |
| 对象 Debug（成功或 writer 提前失败） | 7 次 / 6,072 B | 15 次 / 1,572,792 B | 2 次 / 400 B |

启用 `serde_json/preserve_order` 后，相同宽度下的修改后结果也保持不变：两种形状及 owner/view
比较均为 1 次 / 128 请求字节，Debug 均为 2 次 / 120 字节。未采集匹配的修改前
preserve-order 分配数据。深度为 8、64、256 时，默认 feature 修改后的比较请求分别为
960、9,920、40,640 B，Debug 分别为 2,320、20,240、81,680 B。preserve-order 修改后
比较分别为 384、3,968、16,256 B，Debug 分别为 696、6,072、24,504 B。深度方向的增长
符合显式帧/action 栈的设计，也显示每层的分配成本。

定向计时目标使用 10 个样本、0.1 秒预热和测量窗口：

```bash
cargo bench --locked --bench tree_bench -- json-value-read --warm-up-time 0.1 --measurement-time 0.1 --sample-size 10
```

当前代码的代表性估计值：8192 元素数组相等比较 38.0 µs、8192 项对象相等比较 676 µs、
首个子节点不同为 24–46 ns、深度 256 的 Debug 丢弃输出为 3.85 µs。这些短样本存在
异常值，只用于描述当前实现；宽度扩展证据来自分配计数结果。没有设置计时断言。

### 类型化唯一键解码峰值存活分配采样（2026-10-29）

本次先测量类型化唯一键解码的中间存活分配峰值，再决定是否优化。分配基准的可选 live
计数器记录单次解码期间成功的分配器请求及释放。`peak_extra` 表示相对操作开始时存活字节的
峰值增量；`end_delta` 是操作结束时的存活字节变化。这些是分配器请求计量，不是 RSS，也不是
重复 `Value` 物化的精确计数。`protected_value` 对照保留受保护的动态值；路径相关的其他工作和
结果形态差异也会影响两者差值。实施 worktree 起始于 `cdcdf2c`，采样期间仅包含本任务的实现
变更。fixture 准备和计数器自检均在被测操作之外。直接 Serde 是独立且不等价的对照，因为它不提供
全局重复键拒绝或 decoder 的资源契约。

四种 feature 组合各运行三轮：

```bash
cargo bench --locked --bench unique_decode_allocations
cargo bench --locked --features "serde_json/arbitrary_precision" --bench unique_decode_allocations
cargo bench --locked --features "serde_json/preserve_order" --bench unique_decode_allocations
cargo bench --locked --features "serde_json/arbitrary_precision serde_json/preserve_order" --bench unique_decode_allocations
```

以上四条命令重复三轮。每次运行输出 390 条结构记录和一行 CSV 标题。所有记录均满足
`peak >= baseline` 且 `peak >= end`；三轮数据一致。下表为 fresh、unlimited 的
`unique_typed`，对比 `protected_value`；单位为 `peak_extra` 字节，括号内是对照值。

| Features | 形状 | 条目数 | 类型化峰值额外量（对照） | 类型化减对照 |
| --- | --- | ---: | ---: | ---: |
| default | `wide_object` | 512 | 131,616（90,432） | 41,184 |
| default | `wide_object` | 4096 | 1,065,048（726,776） | 338,272 |
| arbitrary precision | `wide_object` | 512 | 131,616（90,432） | 41,184 |
| arbitrary precision | `wide_object` | 4096 | 1,065,048（726,776） | 338,272 |
| preserve order | `wide_object` | 512 | 200,368（159,184） | 41,184 |
| preserve order | `wide_object` | 4096 | 1,608,496（1,270,224） | 338,272 |
| combined | `wide_object` | 512 | 200,368（159,184） | 41,184 |
| combined | `wide_object` | 4096 | 1,608,496（1,270,224） | 338,272 |
| default | `record_array` | 512 | 384,512（364,992） | 19,520 |
| default | `record_array` | 4096 | 3,076,096（2,916,800） | 159,296 |
| arbitrary precision | `record_array` | 512 | 385,024（365,504） | 19,520 |
| arbitrary precision | `record_array` | 4096 | 3,080,192（2,920,896） | 159,296 |
| preserve order | `record_array` | 512 | 288,256（268,736） | 19,520 |
| preserve order | `record_array` | 4096 | 2,306,048（2,146,752） | 159,296 |
| combined | `record_array` | 512 | 288,768（269,248） | 19,520 |
| combined | `record_array` | 4096 | 2,310,144（2,150,848） | 159,296 |

宽对象样例的差值超过 20% 初筛门槛，但这些数据没有指出哪些分配可以消除。record-array
差值更小，而且对照保留的是不同结果。因此测量不能证明存在可减少 20% 的重复物化成本，也不能
证明存在语义等价的安全解码路径。

延迟参考使用完整 Criterion 命令运行三轮，每轮默认 100 个样本并传入 `--noplot`：

```bash
cargo bench --locked --bench unique_decode_bench -- unique_decode_structural --noplot
```

以下为 unlimited、fresh、4096 项场景的中心估计及 95% 置信区间（毫秒）：

| 场景 | 第 1 轮 | 第 2 轮 | 第 3 轮 |
| --- | ---: | ---: | ---: |
| `wide_object` / `unique_typed` | 8.265（7.613–8.978） | 6.249（5.574–7.068） | 4.146（4.105–4.192） |
| `wide_object` / `unique_seed` | 7.471（6.876–8.132） | 30.671（28.019–33.431） | 4.253（4.199–4.310） |
| `record_array` / `unique_typed` | 5.356（5.060–5.686） | 11.688（10.707–12.724） | 3.546（3.486–3.607） |
| `record_array` / `unique_seed` | 5.438（5.182–5.711） | 6.498（5.980–7.084） | 3.496（3.448–3.545） |

采样期间 1 分钟 load average 在第 1 轮约为 10.78–19.65，第 2 轮约为 10–33，第 3 轮开始约为
26 后逐渐下降。运行还出现长样本警告和离群值，多项结果在不同轮次间变化明显。这些计时只作描述。
live 峰值和延迟采样都没有识别出可归因且可安全消除的重复物化成本，
因此本轮不授权优化生产解码路径。后续若提出方案，应先给出具体的消费式解码设计，并证明数字处理、
错误行为、对象顺序和失败路径有界析构与现有实现等价，再进入实现。

环境为 Ubuntu 24.04.4、Linux 7.0.0-38-generic、x86_64、Intel Core i5-9600K（6 核）、
Rust 1.94.0、Criterion 0.8.2、`serde_json` 1.0.154，使用系统分配器。`Cargo.lock` SHA-256 为
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`。分配计数器自检覆盖了
分配/释放、测量窗口外释放、realloc 扩大与缩小，以及请求字节累计。它是基准诊断机制，不是生产 API。

### SafeJsonValue 复制分配基线（2026-10-30）

本基线在任何生产复制实现变更前测量 `SafeJsonValue::copy_from` 和 `Clone`。fixture 在计量窗口
外构建；窗口内只创建副本，值相等检查、`black_box` 和副本析构均在窗口外。基准首先运行分配
计数器自检。对象 feature 标签通过运行时检查 `serde_json::Map` 的迭代顺序确定。

以下两条命令各顺序运行三次。每种配置的 24 行记录在三轮间完全一致；默认配置第 2 轮输出过
一次 Cargo package-cache 锁等待，其余五轮没有输出锁等待。每种形状与规模下，`copy_from` 和
`clone` 的输出也完全相同。

```bash
cargo bench --locked --bench value_clone_allocations
cargo bench --locked --bench value_clone_allocations --features serde_json/preserve_order
```

CSV 字段为
`VALUE_CLONE_ALLOC,shape,size,feature,operation,revision,allocation_count,allocated_request_bytes,reallocation_count,baseline_live_bytes,peak_live_bytes,end_live_bytes,peak_extra_bytes`。
表中 `operation=copy_from, clone` 表示两个操作分别输出了所列的相同数据。数组/对象的 `size`
表示宽度，混合链表示深度。`end_live_bytes` 包含仍存活的 fixture 和基准进程状态；用它减去
`baseline_live_bytes` 得到结果仍存活时的净存活字节变化。`peak_extra_bytes` 表示超过窗口开始
时基线的最大存活字节，因此同时包括副本和临时分配。`allocated_request_bytes` 包括 realloc 请求
大小。这些是分配器计量，不是 RSS，也不是耗时数据。

| Feature | 形状 | 规模 | 操作 | 分配次数 | 请求字节 | Realloc 次数 | 基线存活 B | 峰值存活 B | 结束存活 B | 峰值额外 B |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| default | array | 32 | copy_from, clone | 11 | 5,680 | 7 | 2,636 | 6,500 | 3,660 | 3,864 |
| default | array | 512 | copy_from, clone | 19 | 94,000 | 15 | 17,996 | 79,460 | 34,380 | 61,464 |
| default | array | 8192 | copy_from, clone | 27 | 1,507,120 | 23 | 263,756 | 1,246,820 | 525,900 | 983,064 |
| default | object_fixed_key | 32 | copy_from, clone | 45 | 8,072 | 4 | 5,380 | 12,532 | 8,924 | 7,152 |
| default | object_fixed_key | 512 | copy_from, clone | 608 | 132,400 | 8 | 63,948 | 180,068 | 122,700 | 116,120 |
| default | object_fixed_key | 8192 | copy_from, clone | 9,573 | 2,134,696 | 12 | 1,014,084 | 2,886,740 | 1,969,212 | 1,872,656 |
| default | object_escaped_non_ascii_key | 32 | copy_from, clone | 45 | 8,168 | 4 | 5,316 | 12,564 | 8,956 | 7,248 |
| default | object_escaped_non_ascii_key | 512 | copy_from, clone | 608 | 133,936 | 8 | 62,924 | 180,580 | 123,212 | 117,656 |
| default | object_escaped_non_ascii_key | 8192 | copy_from, clone | 9,573 | 2,159,272 | 12 | 997,700 | 2,894,932 | 1,977,404 | 1,897,232 |
| default | mixed_deep_chain | 8 | copy_from, clone | 29 | 4,528 | 3 | 4,292 | 7,996 | 7,356 | 3,704 |
| default | mixed_deep_chain | 64 | copy_from, clone | 200 | 35,832 | 6 | 23,052 | 50,892 | 47,564 | 27,840 |
| default | mixed_deep_chain | 256 | copy_from, clone | 778 | 143,160 | 8 | 87,372 | 197,964 | 185,420 | 110,592 |
| preserve_order | array | 32 | copy_from, clone | 11 | 11,760 | 7 | 3,916 | 11,620 | 6,220 | 7,704 |
| preserve_order | array | 512 | copy_from, clone | 19 | 196,080 | 15 | 38,476 | 161,380 | 75,340 | 122,904 |
| preserve_order | array | 8192 | copy_from, clone | 27 | 3,145,200 | 23 | 591,436 | 2,557,540 | 1,181,260 | 1,966,104 |
| preserve_order | object_fixed_key | 32 | copy_from, clone | 50 | 20,924 | 8 | 6,044 | 18,916 | 12,748 | 12,872 |
| preserve_order | object_fixed_key | 512 | copy_from, clone | 542 | 344,028 | 16 | 72,284 | 277,636 | 179,308 | 205,352 |
| preserve_order | object_fixed_key | 8192 | copy_from, clone | 8,234 | 5,512,732 | 24 | 1,132,124 | 4,417,156 | 2,844,268 | 3,285,032 |
| preserve_order | object_escaped_non_ascii_key | 32 | copy_from, clone | 50 | 21,020 | 8 | 5,980 | 18,948 | 12,780 | 12,968 |
| preserve_order | object_escaped_non_ascii_key | 512 | copy_from, clone | 542 | 345,564 | 16 | 71,260 | 278,148 | 179,820 | 206,888 |
| preserve_order | object_escaped_non_ascii_key | 8192 | copy_from, clone | 8,234 | 5,537,308 | 24 | 1,115,740 | 4,425,348 | 2,852,460 | 3,309,608 |
| preserve_order | mixed_deep_chain | 8 | copy_from, clone | 33 | 4,576 | 3 | 3,380 | 6,852 | 6,012 | 3,472 |
| preserve_order | mixed_deep_chain | 64 | copy_from, clone | 232 | 35,096 | 6 | 15,756 | 40,340 | 36,812 | 24,584 |
| preserve_order | mixed_deep_chain | 256 | copy_from, clone | 906 | 139,736 | 8 | 58,188 | 155,156 | 142,412 | 96,968 |

采样 revision 为 `db0a09232b634a26b87393628e20f83c4a47e709`；`Cargo.lock` SHA-256 为
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`。`rustc -Vv` 为 Rust
1.94.0，host `x86_64-unknown-linux-gnu`，LLVM 21.1.8。平台为 Ubuntu 24.04.4、Linux
`7.0.0-38-generic`、x86_64。六轮采样期间工作树均有改动：`Cargo.toml`、`fuzz/Cargo.toml`、
`fuzz/src/json_unique_decode_invariants.rs`、`fuzz/src/lib.rs`、
`fuzz/tests/json_extended_invariants_tests.rs`、`src/encode/json_encoder.rs`，以及未跟踪的
`benches/value_clone_allocations.rs`、`fuzz/src/json_syntax_reference.rs`、
`fuzz/tests/json_syntax_reference_tests.rs`。这些是并行任务 R01、R03 和基准自身的改动；
`src/value/safe_json_value.rs` 未改动。

在宽度 8192 时，测量结束时副本仍存活。default 下数组的净结束存活字节 / 峰值额外字节为
262,144 B / 983,064 B，固定键对象为 955,128 B / 1,872,656 B，转义及非 ASCII 键对象为
979,704 B / 1,897,232 B。`preserve_order` 下相应结果为数组 589,824 B / 1,966,104 B、固定键
对象 1,712,144 B / 3,285,032 B、转义及非 ASCII 键对象 1,736,720 B / 3,309,608 B。两种
map 配置中，最大宽度的峰值存活字节都显著高于最终结果的净存活变化。因此值得进入 T4 做候选
实验；这还不能证明候选能达到前后至少 10% 的门槛。本基线没有修改生产复制实现，也没有设置
耗时断言或将分配器数据称作 RSS。

### SafeJsonValue 复制候选后测（2026-10-30）

基线后，候选实现将批量 Task/output 栈换为深度优先迭代器帧。最终帧存放 iterator、至多一个
对象键，以及一个布尔值来表示是否持有 builder 栈项。活动 `SafeValueBuilder` 保存在独立的
LIFO 向量中。只有当前容器在首个子节点后还有兄弟节点时才创建 builder，因此单子节点链不分配
或持有 builder 项。回溯时，已完成子树由 `SafeJsonValue` 持有，部分结果由 builder 保护，嵌套
builder 按帧的后进先出顺序弹出。没有新增 unsafe。

最终候选上，以下两条命令各顺序运行三次。每种 feature 的 24 行记录在三轮间完全一致；
`copy_from` 和 `clone` 每行数据也完全相同。default 三轮没有 Cargo 锁等待；preserve_order 第
一轮编译前出现过一次 package-cache 锁等待，后两轮没有。下表沿用基线表中的 CSV 字段定义，
操作列列出两个相同结果的操作。

| Feature | 形状 | 规模 | 操作 | 分配次数 | 请求字节 | Realloc 次数 | 基线存活 B | 峰值存活 B | 结束存活 B | 峰值额外 B |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| default | array | 32 | copy_from, clone | 6 | 2,400 | 3 | 2,636 | 4,140 | 3,660 | 1,504 |
| default | array | 512 | copy_from, clone | 10 | 33,120 | 7 | 17,996 | 34,860 | 34,380 | 16,864 |
| default | array | 8192 | copy_from, clone | 14 | 524,640 | 11 | 263,756 | 526,380 | 525,900 | 262,624 |
| default | object_fixed_key | 32 | copy_from, clone | 39 | 4,024 | 0 | 5,380 | 9,404 | 8,924 | 4,024 |
| default | object_fixed_key | 512 | copy_from, clone | 598 | 59,232 | 0 | 63,948 | 123,180 | 122,700 | 59,232 |
| default | object_fixed_key | 8192 | copy_from, clone | 9,559 | 955,608 | 0 | 1,014,084 | 1,969,692 | 1,969,212 | 955,608 |
| default | object_escaped_non_ascii_key | 32 | copy_from, clone | 39 | 4,120 | 0 | 5,316 | 9,436 | 8,956 | 4,120 |
| default | object_escaped_non_ascii_key | 512 | copy_from, clone | 598 | 60,768 | 0 | 62,924 | 123,692 | 123,212 | 60,768 |
| default | object_escaped_non_ascii_key | 8192 | copy_from, clone | 9,559 | 980,184 | 0 | 997,700 | 1,977,884 | 1,977,404 | 980,184 |
| default | mixed_deep_chain | 8 | copy_from, clone | 14 | 4,120 | 1 | 4,292 | 8,060 | 7,356 | 3,768 |
| default | mixed_deep_chain | 64 | copy_from, clone | 101 | 35,424 | 4 | 23,052 | 53,196 | 47,564 | 30,144 |
| default | mixed_deep_chain | 256 | copy_from, clone | 391 | 142,752 | 6 | 87,372 | 207,948 | 185,420 | 120,576 |
| preserve_order | array | 32 | copy_from, clone | 6 | 4,736 | 3 | 3,916 | 6,636 | 6,220 | 2,720 |
| preserve_order | array | 512 | copy_from, clone | 10 | 73,856 | 7 | 38,476 | 75,756 | 75,340 | 37,280 |
| preserve_order | array | 8192 | copy_from, clone | 14 | 1,179,776 | 11 | 591,436 | 1,181,676 | 1,181,260 | 590,240 |
| preserve_order | object_fixed_key | 32 | copy_from, clone | 44 | 13,132 | 4 | 6,044 | 13,164 | 12,748 | 7,120 |
| preserve_order | object_fixed_key | 512 | copy_from, clone | 532 | 209,516 | 8 | 72,284 | 179,724 | 179,308 | 107,440 |
| preserve_order | object_fixed_key | 8192 | copy_from, clone | 8,220 | 3,350,700 | 12 | 1,132,124 | 2,844,684 | 2,844,268 | 1,712,560 |
| preserve_order | object_escaped_non_ascii_key | 32 | copy_from, clone | 44 | 13,228 | 4 | 5,980 | 13,196 | 12,780 | 7,216 |
| preserve_order | object_escaped_non_ascii_key | 512 | copy_from, clone | 532 | 211,052 | 8 | 71,260 | 180,236 | 179,820 | 108,976 |
| preserve_order | object_escaped_non_ascii_key | 8192 | copy_from, clone | 8,220 | 3,375,276 | 12 | 1,115,740 | 2,852,876 | 2,852,460 | 1,737,136 |
| preserve_order | mixed_deep_chain | 8 | copy_from, clone | 18 | 3,016 | 1 | 3,380 | 6,268 | 6,012 | 2,888 |
| preserve_order | mixed_deep_chain | 64 | copy_from, clone | 133 | 25,024 | 4 | 15,756 | 38,860 | 36,812 | 23,104 |
| preserve_order | mixed_deep_chain | 256 | copy_from, clone | 519 | 100,480 | 6 | 58,188 | 150,604 | 142,412 | 92,416 |

后测 revision 仍为 `db0a09232b634a26b87393628e20f83c4a47e709`，`Cargo.lock` SHA-256 仍为
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`，Rust/platform 信息与前一节
基线相同。后测期间的 dirty worktree 包含 `Cargo.toml`、两份 benchmark 基线文档、R01 的 fuzz
源文件及 `fuzz/Cargo.toml`、`src/encode/json_encoder.rs`、`src/tests/j3/owner_tests.rs`、
`src/value/internal/mod.rs`、`src/value/safe_json_value.rs`、
`tests/value/safe_json_value_tests.rs`，以及未跟踪的 `benches/value_clone_allocations.rs`、
`src/value/internal/json_value_copy_frame.rs` 和新的 R01 fuzz module/test。采样未修改
`Cargo.lock`。

宽度 8192 时，`peak_extra_bytes` 基线到最终候选的变化为：default 数组 983,064→262,624 B
（降低 73.3%）、固定键对象 1,872,656→955,608 B（降低 49.0%）、转义/非 ASCII 键对象
1,897,232→980,184 B（降低 48.3%）。`preserve_order` 下分别为数组 1,966,104→590,240 B
（降低 70.0%）、固定键对象 3,285,032→1,712,560 B（降低 47.9%）、转义/非 ASCII 键对象
3,309,608→1,737,136 B（降低 47.5%）。因此，8192 项数组和两类对象在两种 map 配置下均达到
设计要求的至少 10% 降幅。

最终 mixed-chain 结果如下：

| Feature | 深度 | 基线峰值额外 B | 候选峰值额外 B | 变化 |
| --- | ---: | ---: | ---: | ---: |
| default | 8 | 3,704 | 3,768 | +1.7% |
| default | 64 | 27,840 | 30,144 | +8.3% |
| default | 256 | 110,592 | 120,576 | +9.0%（+9,984 B） |
| preserve_order | 8 | 3,472 | 2,888 | -16.8% |
| preserve_order | 64 | 24,584 | 23,104 | -6.0% |
| preserve_order | 256 | 96,968 | 92,416 | -4.7% |

第一版帧在每层内联保存 builder，使深度 256 的 `peak_extra` 增加 default 14.6%、
`preserve_order` 12.2%，所以没有保留。最终紧凑帧去掉 8-byte builder 索引，改用布尔标志；
独立 builder 栈仅为还有未访问兄弟节点的帧持有 builder。基准中的单子节点链不会分配该栈。
最终 default 深度 256 的峰值仍比基线高 9.0%（9,984 B），但请求字节总量是 142,752 B，
低于基线的 143,160 B；preserve_order 深链峰值均低于基线。这里如实记录 default 深链峰值差异，
不把它描述成优化。考虑其规模、较低的总请求流量、无 unsafe/语义变更，以及两种配置下宽形状
47.5–73.3% 的峰值下降，按设计条件保留候选。测试逐值比较副本、比较源和副本的键迭代顺序、
覆盖深度 8/64/256，并验证注入 unwind 时数组和对象部分结果的清理。没有墙钟阈值或 RSS 声明。


## RJ-01/RJ-02 生产候选修改前的分配基线

- HEAD：`8aa4be27bb8babde6e10fc1284a0d83eee66a5da`（T1 期间未变）。
- Rust：`rustc 1.94.0 (4a4ef493e 2026-03-02)`；目标平台 `x86_64-unknown-linux-gnu`；Cargo `1.94.0`。
- 主机硬件：Intel Core i5-9600K @ 3.70 GHz，6 CPU；Linux 内核 `7.0.0-38-generic`。
- `Cargo.lock` 的 SHA-256：`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`。
- 源码 SHA-256（按顺序）：Cargo.toml `1800a3b371d6b3626d739cbfde4d609ebe5bdbcb2131dd60313d1af5bc71bc17`；bench `a3d6d1bf3ec9c04a83bef207f791a193fe37fe326c482d9d6bb0ea11e605b2d7`；safe_value_builder `b043ec5e1e69b5be7da3eac64a0200a756e9fe42068750b7f6813855442cd9de`；json_value_serializer `4afc5f1af7f6ac4b9c704408ea48eb5d4f93f3083fc7f8ed0af2aaf233ad3cac`；json_value_compound `55dd5c054d5b4cc007b8b9fa0e829f0b9e558add4893402ba2b2173237e13d5e`；value_visitor `96fba144e005b1d1fe1d09194fc0f4537bbccbbb235be0c4cfcecf8877354fc1`。
- 采样时记录的 dirty 文件：`Cargo.toml`、`benches/value_projection_allocations.rs`、`fuzz/src/json_unique_decode_invariants.rs`、`fuzz/tests/json_unique_decode_invariants_tests.rs`、`src/decode/json_decoder.rs`、`src/value/safe_json_value.rs`、`tests/encode/serde_compat/serde_json_compat_tests/json_private_tests.rs`。T1 未修改上列 RJ-01/RJ-02 生产源码；同一共享 worktree 中其他并行 T4/T5 工作已修改其余 dirty 文件。
- Fixture：宽度 32/512/8192；普通序列与显式 `serialize_bytes`；短键为 `kNNNNN`；长键以 128 个 ASCII `p` 字节加唯一数字后缀构成；array 链深度 8/64/256。project 测量 `JsonValueEncoder::encode_in_transaction`；bridge 测量直接将 JSON 反序列化为 `SafeJsonValue`。fixture/input、期望值和 project transaction 均在量测前准备。受保护结果在量测期间存活；相等性校验和 transaction commit 在量测后执行，且两者均不计入量测。仅深度 256 的控制组因 `from_str` 默认递归上限无法解析，使用 `Deserializer::disable_recursion_limit`；它仍是直接 Serde 反序列化，并在窗口外校验。
- allocation 命令每种配置连续运行三次：`cargo bench --locked --bench value_projection_allocations`；`cargo bench --locked --bench value_projection_allocations --features serde_json/preserve_order`。每次完整运行输出 24 行。首次探索运行因 serde_json 默认递归上限在 chain 深度 256 失败，未纳入；修正后该控制项改用显式无上限 deserializer。一次编译尝试还发现并修正了仅限 benchmark 的相等比较类型错误；没有因此修改生产文件。
- Allocation 数值表示成功分配请求数/字节、realloc 请求和实时/峰值请求字节；它们是分配器请求诊断，不是 RSS。每种 feature 的三轮中，每条数据完全一致。


两个 feature 配置下的 project 与 bridge 操作使用相同 fixture 定义。以下六次完整运行均各输出 24 条记录。

### default allocation samples

| Round | Shape | Size | Operation | Allocations | Requested B | Reallocations | Baseline live B | Peak live B | End live B | Peak extra B |
| ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | sequence | 32 | project | 5 | 2944 | 3 | 3054 | 5102 | 4078 | 2048 |
| 1 | bytes | 32 | project | 5 | 2944 | 3 | 4110 | 6158 | 5134 | 2048 |
| 1 | object_short | 32 | project | 75 | 6928 | 0 | 6990 | 13918 | 10438 | 6928 |
| 1 | object_short | 32 | bridge | 78 | 8496 | 3 | 7502 | 14430 | 10950 | 6928 |
| 1 | object_long_prefix | 32 | project | 75 | 15120 | 0 | 18318 | 33438 | 25862 | 15120 |
| 1 | object_long_prefix | 32 | bridge | 78 | 16688 | 3 | 26510 | 41630 | 34054 | 15120 |
| 1 | sequence | 512 | project | 9 | 49024 | 7 | 18414 | 51182 | 34798 | 32768 |
| 1 | bytes | 512 | project | 9 | 49024 | 7 | 35310 | 68078 | 51694 | 32768 |
| 1 | object_short | 512 | project | 1193 | 113536 | 0 | 70222 | 183758 | 127438 | 113536 |
| 1 | object_short | 512 | bridge | 1200 | 141984 | 7 | 78414 | 191950 | 135630 | 113536 |
| 1 | object_long_prefix | 512 | project | 1193 | 244608 | 0 | 240302 | 484910 | 363054 | 244608 |
| 1 | object_long_prefix | 512 | bridge | 1200 | 273056 | 7 | 371374 | 615982 | 494126 | 244608 |
| 1 | sequence | 8192 | project | 16 | 1015680 | 14 | 264174 | 788462 | 526318 | 524288 |
| 1 | bytes | 8192 | project | 16 | 1015680 | 14 | 534510 | 1058798 | 796654 | 524288 |
| 1 | object_short | 8192 | project | 19118 | 2240784 | 3 | 1077566 | 2916942 | 2008118 | 1839376 |
| 1 | object_short | 8192 | bridge | 19126 | 2297904 | 11 | 1208638 | 3048014 | 2139190 | 1839376 |
| 1 | object_long_prefix | 8192 | project | 19118 | 4337936 | 3 | 3783310 | 7719838 | 5762438 | 3936528 |
| 1 | object_long_prefix | 8192 | bridge | 19126 | 4395056 | 11 | 5880462 | 9816990 | 7859590 | 3936528 |
| 1 | chain | 8 | project | 16 | 1280 | 0 | 2286 | 3342 | 3310 | 1056 |
| 1 | chain | 8 | bridge | 8 | 1024 | 0 | 2414 | 3438 | 3438 | 1024 |
| 1 | chain | 64 | project | 128 | 10240 | 0 | 4078 | 12302 | 12270 | 8224 |
| 1 | chain | 64 | bridge | 64 | 8192 | 0 | 4334 | 12526 | 12526 | 8192 |
| 1 | chain | 256 | project | 512 | 40960 | 0 | 10222 | 43022 | 42990 | 32800 |
| 1 | chain | 256 | bridge | 256 | 32768 | 0 | 11246 | 44014 | 44014 | 32768 |
| 2 | sequence | 32 | project | 5 | 2944 | 3 | 3054 | 5102 | 4078 | 2048 |
| 2 | bytes | 32 | project | 5 | 2944 | 3 | 4110 | 6158 | 5134 | 2048 |
| 2 | object_short | 32 | project | 75 | 6928 | 0 | 6990 | 13918 | 10438 | 6928 |
| 2 | object_short | 32 | bridge | 78 | 8496 | 3 | 7502 | 14430 | 10950 | 6928 |
| 2 | object_long_prefix | 32 | project | 75 | 15120 | 0 | 18318 | 33438 | 25862 | 15120 |
| 2 | object_long_prefix | 32 | bridge | 78 | 16688 | 3 | 26510 | 41630 | 34054 | 15120 |
| 2 | sequence | 512 | project | 9 | 49024 | 7 | 18414 | 51182 | 34798 | 32768 |
| 2 | bytes | 512 | project | 9 | 49024 | 7 | 35310 | 68078 | 51694 | 32768 |
| 2 | object_short | 512 | project | 1193 | 113536 | 0 | 70222 | 183758 | 127438 | 113536 |
| 2 | object_short | 512 | bridge | 1200 | 141984 | 7 | 78414 | 191950 | 135630 | 113536 |
| 2 | object_long_prefix | 512 | project | 1193 | 244608 | 0 | 240302 | 484910 | 363054 | 244608 |
| 2 | object_long_prefix | 512 | bridge | 1200 | 273056 | 7 | 371374 | 615982 | 494126 | 244608 |
| 2 | sequence | 8192 | project | 16 | 1015680 | 14 | 264174 | 788462 | 526318 | 524288 |
| 2 | bytes | 8192 | project | 16 | 1015680 | 14 | 534510 | 1058798 | 796654 | 524288 |
| 2 | object_short | 8192 | project | 19118 | 2240784 | 3 | 1077566 | 2916942 | 2008118 | 1839376 |
| 2 | object_short | 8192 | bridge | 19126 | 2297904 | 11 | 1208638 | 3048014 | 2139190 | 1839376 |
| 2 | object_long_prefix | 8192 | project | 19118 | 4337936 | 3 | 3783310 | 7719838 | 5762438 | 3936528 |
| 2 | object_long_prefix | 8192 | bridge | 19126 | 4395056 | 11 | 5880462 | 9816990 | 7859590 | 3936528 |
| 2 | chain | 8 | project | 16 | 1280 | 0 | 2286 | 3342 | 3310 | 1056 |
| 2 | chain | 8 | bridge | 8 | 1024 | 0 | 2414 | 3438 | 3438 | 1024 |
| 2 | chain | 64 | project | 128 | 10240 | 0 | 4078 | 12302 | 12270 | 8224 |
| 2 | chain | 64 | bridge | 64 | 8192 | 0 | 4334 | 12526 | 12526 | 8192 |
| 2 | chain | 256 | project | 512 | 40960 | 0 | 10222 | 43022 | 42990 | 32800 |
| 2 | chain | 256 | bridge | 256 | 32768 | 0 | 11246 | 44014 | 44014 | 32768 |
| 3 | sequence | 32 | project | 5 | 2944 | 3 | 3054 | 5102 | 4078 | 2048 |
| 3 | bytes | 32 | project | 5 | 2944 | 3 | 4110 | 6158 | 5134 | 2048 |
| 3 | object_short | 32 | project | 75 | 6928 | 0 | 6990 | 13918 | 10438 | 6928 |
| 3 | object_short | 32 | bridge | 78 | 8496 | 3 | 7502 | 14430 | 10950 | 6928 |
| 3 | object_long_prefix | 32 | project | 75 | 15120 | 0 | 18318 | 33438 | 25862 | 15120 |
| 3 | object_long_prefix | 32 | bridge | 78 | 16688 | 3 | 26510 | 41630 | 34054 | 15120 |
| 3 | sequence | 512 | project | 9 | 49024 | 7 | 18414 | 51182 | 34798 | 32768 |
| 3 | bytes | 512 | project | 9 | 49024 | 7 | 35310 | 68078 | 51694 | 32768 |
| 3 | object_short | 512 | project | 1193 | 113536 | 0 | 70222 | 183758 | 127438 | 113536 |
| 3 | object_short | 512 | bridge | 1200 | 141984 | 7 | 78414 | 191950 | 135630 | 113536 |
| 3 | object_long_prefix | 512 | project | 1193 | 244608 | 0 | 240302 | 484910 | 363054 | 244608 |
| 3 | object_long_prefix | 512 | bridge | 1200 | 273056 | 7 | 371374 | 615982 | 494126 | 244608 |
| 3 | sequence | 8192 | project | 16 | 1015680 | 14 | 264174 | 788462 | 526318 | 524288 |
| 3 | bytes | 8192 | project | 16 | 1015680 | 14 | 534510 | 1058798 | 796654 | 524288 |
| 3 | object_short | 8192 | project | 19118 | 2240784 | 3 | 1077566 | 2916942 | 2008118 | 1839376 |
| 3 | object_short | 8192 | bridge | 19126 | 2297904 | 11 | 1208638 | 3048014 | 2139190 | 1839376 |
| 3 | object_long_prefix | 8192 | project | 19118 | 4337936 | 3 | 3783310 | 7719838 | 5762438 | 3936528 |
| 3 | object_long_prefix | 8192 | bridge | 19126 | 4395056 | 11 | 5880462 | 9816990 | 7859590 | 3936528 |
| 3 | chain | 8 | project | 16 | 1280 | 0 | 2286 | 3342 | 3310 | 1056 |
| 3 | chain | 8 | bridge | 8 | 1024 | 0 | 2414 | 3438 | 3438 | 1024 |
| 3 | chain | 64 | project | 128 | 10240 | 0 | 4078 | 12302 | 12270 | 8224 |
| 3 | chain | 64 | bridge | 64 | 8192 | 0 | 4334 | 12526 | 12526 | 8192 |
| 3 | chain | 256 | project | 512 | 40960 | 0 | 10222 | 43022 | 42990 | 32800 |
| 3 | chain | 256 | bridge | 256 | 32768 | 0 | 11246 | 44014 | 44014 | 32768 |

### preserve_order allocation samples

| Round | Shape | Size | Operation | Allocations | Requested B | Reallocations | Baseline live B | Peak live B | End live B | Peak extra B |
| ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | sequence | 32 | project | 5 | 6624 | 3 | 4334 | 8942 | 6638 | 4608 |
| 1 | bytes | 32 | project | 5 | 6624 | 3 | 6670 | 11278 | 8974 | 4608 |
| 1 | object_short | 32 | project | 80 | 17380 | 4 | 10846 | 22214 | 17454 | 11368 |
| 1 | object_short | 32 | bridge | 83 | 20068 | 7 | 11358 | 22726 | 17966 | 11368 |
| 1 | object_long_prefix | 32 | project | 80 | 25572 | 4 | 23470 | 43030 | 34174 | 19560 |
| 1 | object_long_prefix | 32 | bridge | 83 | 28260 | 7 | 31662 | 51222 | 42366 | 19560 |
| 1 | sequence | 512 | project | 9 | 110304 | 7 | 38894 | 112622 | 75758 | 73728 |
| 1 | bytes | 512 | project | 9 | 110304 | 7 | 76270 | 149998 | 113134 | 73728 |
| 1 | object_short | 512 | project | 1127 | 284364 | 8 | 142846 | 325134 | 248334 | 182288 |
| 1 | object_short | 512 | bridge | 1134 | 333132 | 15 | 151038 | 333326 | 256526 | 182288 |
| 1 | object_long_prefix | 512 | project | 1127 | 415436 | 8 | 344590 | 657950 | 515614 | 313360 |
| 1 | object_long_prefix | 512 | bridge | 1134 | 464204 | 15 | 475662 | 789022 | 646686 | 313360 |
| 1 | sequence | 8192 | project | 16 | 2285280 | 14 | 591854 | 1771502 | 1181678 | 1179648 |
| 1 | bytes | 8192 | project | 16 | 2285280 | 14 | 1189870 | 2369518 | 1779694 | 1179648 |
| 1 | object_short | 8192 | project | 17779 | 5250340 | 15 | 2254846 | 5178918 | 3942414 | 2924072 |
| 1 | object_short | 8192 | bridge | 17787 | 5348260 | 23 | 2385918 | 5309990 | 4073486 | 2924072 |
| 1 | object_long_prefix | 8192 | project | 17779 | 7347492 | 15 | 5482510 | 10503734 | 8218654 | 5021224 |
| 1 | object_long_prefix | 8192 | bridge | 17787 | 7445412 | 23 | 7579662 | 12600886 | 10315806 | 5021224 |
| 1 | chain | 8 | project | 16 | 2880 | 0 | 2606 | 4982 | 4910 | 2376 |
| 1 | chain | 8 | bridge | 8 | 2304 | 0 | 2734 | 5038 | 5038 | 2304 |
| 1 | chain | 64 | project | 128 | 23040 | 0 | 6638 | 25142 | 25070 | 18504 |
| 1 | chain | 64 | bridge | 64 | 18432 | 0 | 6894 | 25326 | 25326 | 18432 |
| 1 | chain | 256 | project | 512 | 92160 | 0 | 20462 | 94262 | 94190 | 73800 |
| 1 | chain | 256 | bridge | 256 | 73728 | 0 | 21486 | 95214 | 95214 | 73728 |
| 2 | sequence | 32 | project | 5 | 6624 | 3 | 4334 | 8942 | 6638 | 4608 |
| 2 | bytes | 32 | project | 5 | 6624 | 3 | 6670 | 11278 | 8974 | 4608 |
| 2 | object_short | 32 | project | 80 | 17380 | 4 | 10846 | 22214 | 17454 | 11368 |
| 2 | object_short | 32 | bridge | 83 | 20068 | 7 | 11358 | 22726 | 17966 | 11368 |
| 2 | object_long_prefix | 32 | project | 80 | 25572 | 4 | 23470 | 43030 | 34174 | 19560 |
| 2 | object_long_prefix | 32 | bridge | 83 | 28260 | 7 | 31662 | 51222 | 42366 | 19560 |
| 2 | sequence | 512 | project | 9 | 110304 | 7 | 38894 | 112622 | 75758 | 73728 |
| 2 | bytes | 512 | project | 9 | 110304 | 7 | 76270 | 149998 | 113134 | 73728 |
| 2 | object_short | 512 | project | 1127 | 284364 | 8 | 142846 | 325134 | 248334 | 182288 |
| 2 | object_short | 512 | bridge | 1134 | 333132 | 15 | 151038 | 333326 | 256526 | 182288 |
| 2 | object_long_prefix | 512 | project | 1127 | 415436 | 8 | 344590 | 657950 | 515614 | 313360 |
| 2 | object_long_prefix | 512 | bridge | 1134 | 464204 | 15 | 475662 | 789022 | 646686 | 313360 |
| 2 | sequence | 8192 | project | 16 | 2285280 | 14 | 591854 | 1771502 | 1181678 | 1179648 |
| 2 | bytes | 8192 | project | 16 | 2285280 | 14 | 1189870 | 2369518 | 1779694 | 1179648 |
| 2 | object_short | 8192 | project | 17779 | 5250340 | 15 | 2254846 | 5178918 | 3942414 | 2924072 |
| 2 | object_short | 8192 | bridge | 17787 | 5348260 | 23 | 2385918 | 5309990 | 4073486 | 2924072 |
| 2 | object_long_prefix | 8192 | project | 17779 | 7347492 | 15 | 5482510 | 10503734 | 8218654 | 5021224 |
| 2 | object_long_prefix | 8192 | bridge | 17787 | 7445412 | 23 | 7579662 | 12600886 | 10315806 | 5021224 |
| 2 | chain | 8 | project | 16 | 2880 | 0 | 2606 | 4982 | 4910 | 2376 |
| 2 | chain | 8 | bridge | 8 | 2304 | 0 | 2734 | 5038 | 5038 | 2304 |
| 2 | chain | 64 | project | 128 | 23040 | 0 | 6638 | 25142 | 25070 | 18504 |
| 2 | chain | 64 | bridge | 64 | 18432 | 0 | 6894 | 25326 | 25326 | 18432 |
| 2 | chain | 256 | project | 512 | 92160 | 0 | 20462 | 94262 | 94190 | 73800 |
| 2 | chain | 256 | bridge | 256 | 73728 | 0 | 21486 | 95214 | 95214 | 73728 |
| 3 | sequence | 32 | project | 5 | 6624 | 3 | 4334 | 8942 | 6638 | 4608 |
| 3 | bytes | 32 | project | 5 | 6624 | 3 | 6670 | 11278 | 8974 | 4608 |
| 3 | object_short | 32 | project | 80 | 17380 | 4 | 10846 | 22214 | 17454 | 11368 |
| 3 | object_short | 32 | bridge | 83 | 20068 | 7 | 11358 | 22726 | 17966 | 11368 |
| 3 | object_long_prefix | 32 | project | 80 | 25572 | 4 | 23470 | 43030 | 34174 | 19560 |
| 3 | object_long_prefix | 32 | bridge | 83 | 28260 | 7 | 31662 | 51222 | 42366 | 19560 |
| 3 | sequence | 512 | project | 9 | 110304 | 7 | 38894 | 112622 | 75758 | 73728 |
| 3 | bytes | 512 | project | 9 | 110304 | 7 | 76270 | 149998 | 113134 | 73728 |
| 3 | object_short | 512 | project | 1127 | 284364 | 8 | 142846 | 325134 | 248334 | 182288 |
| 3 | object_short | 512 | bridge | 1134 | 333132 | 15 | 151038 | 333326 | 256526 | 182288 |
| 3 | object_long_prefix | 512 | project | 1127 | 415436 | 8 | 344590 | 657950 | 515614 | 313360 |
| 3 | object_long_prefix | 512 | bridge | 1134 | 464204 | 15 | 475662 | 789022 | 646686 | 313360 |
| 3 | sequence | 8192 | project | 16 | 2285280 | 14 | 591854 | 1771502 | 1181678 | 1179648 |
| 3 | bytes | 8192 | project | 16 | 2285280 | 14 | 1189870 | 2369518 | 1779694 | 1179648 |
| 3 | object_short | 8192 | project | 17779 | 5250340 | 15 | 2254846 | 5178918 | 3942414 | 2924072 |
| 3 | object_short | 8192 | bridge | 17787 | 5348260 | 23 | 2385918 | 5309990 | 4073486 | 2924072 |
| 3 | object_long_prefix | 8192 | project | 17779 | 7347492 | 15 | 5482510 | 10503734 | 8218654 | 5021224 |
| 3 | object_long_prefix | 8192 | bridge | 17787 | 7445412 | 23 | 7579662 | 12600886 | 10315806 | 5021224 |
| 3 | chain | 8 | project | 16 | 2880 | 0 | 2606 | 4982 | 4910 | 2376 |
| 3 | chain | 8 | bridge | 8 | 2304 | 0 | 2734 | 5038 | 5038 | 2304 |
| 3 | chain | 64 | project | 128 | 23040 | 0 | 6638 | 25142 | 25070 | 18504 |
| 3 | chain | 64 | bridge | 64 | 18432 | 0 | 6894 | 25326 | 25326 | 18432 |
| 3 | chain | 256 | project | 512 | 92160 | 0 | 20462 | 94262 | 94190 | 73800 |
| 3 | chain | 256 | bridge | 256 | 73728 | 0 | 21486 | 95214 | 95214 | 73728 |

### RJ-02 延迟前测（仅 default 配置，尝试结果）

命令：`cargo bench --locked --bench decoder_bench -- safe_value_bridge`（Criterion 全量采样，每个用例 100 个样本，未使用 `--quick`）。命令完成 36 个 benchmark 用例。下表记录 Criterion 输出的中心估计、95% 置信区间及离群样本数：

| Benchmark | Lower (95% CI) | Estimate | Upper (95% CI) | Outliers |
| --- | ---: | ---: | ---: | ---: |
| safe_value_bridge/serde/short/unique/32 | 28.403 µs | 34.276 µs | 40.419 µs | 1/100 |
| safe_value_bridge/json_decoder/short/unique/32 | 105.62 µs | 113.63 µs | 121.94 µs | 3/100 |
| safe_value_bridge/serde/short/duplicate_early/32 | 2.0403 µs | 2.3053 µs | 2.5988 µs | 3/100 |
| safe_value_bridge/json_decoder/short/duplicate_early/32 | 2.8726 µs | 3.3246 µs | 3.8275 µs | 4/100 |
| safe_value_bridge/serde/short/duplicate_tail/32 | 34.316 µs | 39.754 µs | 45.415 µs | 4/100 |
| safe_value_bridge/json_decoder/short/duplicate_tail/32 | 92.754 µs | 107.33 µs | 122.21 µs | 3/100 |
| safe_value_bridge/serde/long_prefix/unique/32 | 62.565 µs | 68.827 µs | 74.981 µs | 4/100 |
| safe_value_bridge/json_decoder/long_prefix/unique/32 | 42.971 µs | 48.378 µs | 54.588 µs | 4/100 |
| safe_value_bridge/serde/long_prefix/duplicate_early/32 | 1.0768 µs | 1.4664 µs | 2.0863 µs | 6/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_early/32 | 2.3808 µs | 3.1282 µs | 4.0385 µs | 8/100 |
| safe_value_bridge/serde/long_prefix/duplicate_tail/32 | 20.688 µs | 31.342 µs | 42.731 µs | 13/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_tail/32 | 102.19 µs | 113.38 µs | 125.07 µs | 3/100 |
| safe_value_bridge/serde/short/unique/512 | 589.77 µs | 707.75 µs | 840.34 µs | 5/100 |
| safe_value_bridge/json_decoder/short/unique/512 | 1.6295 ms | 1.8767 ms | 2.1436 ms | 1/100 |
| safe_value_bridge/serde/short/duplicate_early/512 | 1.1712 µs | 1.4728 µs | 1.8089 µs | 9/100 |
| safe_value_bridge/json_decoder/short/duplicate_early/512 | 2.1705 µs | 2.5048 µs | 2.8987 µs | 2/100 |
| safe_value_bridge/serde/short/duplicate_tail/512 | 424.90 µs | 487.92 µs | 560.31 µs | 2/100 |
| safe_value_bridge/json_decoder/short/duplicate_tail/512 | 1.0293 ms | 1.2437 ms | 1.4786 ms | 3/100 |
| safe_value_bridge/serde/long_prefix/unique/512 | 525.09 µs | 636.48 µs | 766.18 µs | 5/100 |
| safe_value_bridge/json_decoder/long_prefix/unique/512 | 1.3005 ms | 1.4953 ms | 1.7058 ms | 4/100 |
| safe_value_bridge/serde/long_prefix/duplicate_early/512 | 1.5845 µs | 1.8823 µs | 2.1821 µs | 1/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_early/512 | 6.7938 µs | 7.6372 µs | 8.4387 µs | 2/100 |
| safe_value_bridge/serde/long_prefix/duplicate_tail/512 | 371.75 µs | 428.25 µs | 487.73 µs | 1/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_tail/512 | 1.4121 ms | 1.6670 ms | 1.9290 ms | 5/100 |
| safe_value_bridge/serde/short/unique/4096 | 7.9057 ms | 8.6422 ms | 9.4354 ms | 1/100 |
| safe_value_bridge/json_decoder/short/unique/4096 | 10.839 ms | 11.765 ms | 12.691 ms | 4/100 |
| safe_value_bridge/serde/short/duplicate_early/4096 | 1.0311 µs | 1.1389 µs | 1.2632 µs | 4/100 |
| safe_value_bridge/json_decoder/short/duplicate_early/4096 | 5.3698 µs | 6.0881 µs | 6.9337 µs | 2/100 |
| safe_value_bridge/serde/short/duplicate_tail/4096 | 6.5620 ms | 7.1397 ms | 7.7525 ms | 2/100 |
| safe_value_bridge/json_decoder/short/duplicate_tail/4096 | 5.8590 ms | 6.4975 ms | 7.1895 ms | 2/100 |
| safe_value_bridge/serde/long_prefix/unique/4096 | 5.5580 ms | 6.4765 ms | 7.4909 ms | 6/100 |
| safe_value_bridge/json_decoder/long_prefix/unique/4096 | 8.9081 ms | 10.509 ms | 12.302 ms | 11/100 |
| safe_value_bridge/serde/long_prefix/duplicate_early/4096 | 1.0592 µs | 1.1890 µs | 1.3269 µs | 1/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_early/4096 | 26.874 µs | 32.260 µs | 39.656 µs | 5/100 |
| safe_value_bridge/serde/long_prefix/duplicate_tail/4096 | 7.3674 ms | 8.4231 ms | 9.6464 ms | 2/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_tail/4096 | 6.9676 ms | 8.4150 ms | 9.9817 ms | 7/100 |

环境：HEAD、工具链、锁文件与上文相同。运行期间共享主机有 23 个登录用户，负载峰值记录为 `47.13, 41.14, 30.70`；运行刚结束时为 `28.71, 34.10, 30.86`。同时存在其他 worktree 的 CI、Rust 编译和 Miri 进程，且部分用例报告 high-severe 离群值。因此保留这些原始观察值，但不视为可靠的 RJ-02 延迟基线。由于主机负载仍高，未运行要求的 `preserve_order` Criterion 命令。RJ-02 延迟仍未验证；不得据此决定是否采纳。任务期间完整输出保存在 `/tmp/rj-t1-criterion-default.log`。
