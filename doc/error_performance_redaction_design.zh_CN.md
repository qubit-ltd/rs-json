# JSON 错误、性能与下游安全边界设计

[English](error_performance_redaction_design.md)

本文记录 `qubit-json 0.8` 首次发布前这轮允许破坏性变更的整体设计决策。它覆盖
`rs-json` 的序列化错误与热路径，也覆盖直接依赖这些契约的 `rs-budget`、
`rs-value` 和 `rs-redact`。

## 设计原则

- 程序按结构化枚举处理错误，不能依赖字符串；公共诊断不得携带输入值、对象键或第三方
  `Error::custom` 文本。
- 严格数字、重复键、`RawValue` 验证、预算事务和增量 writer 的 partial-output 契约优先于
  吞吐量；优化只能删除不影响结果的工作。
- `unlimited()` 只适用于可信输入或已由其他层完成资源准入的数据。不可信输入必须在分配
  完整树之前同时受到输入字节和结构预算约束。
- 实验性实现只有在目标场景稳定提升至少 5%、主要既有场景不稳定退化超过 3% 时才保留。

## 序列化错误模型

文本编码与物化值编码共用 `JsonSerializationError`，该值对象只持有
`JsonSerializationErrorKind`。精确 kind 覆盖：

- 有符号或无符号整数越界、非有限浮点和无效数字表示；
- 不支持的 map-key Serde shape 和规范化后的重复对象键；
- 无效 `RawValue`；
- array/object 长度溢出；
- compound serializer 调用顺序错误或伪造 `RawValue` 协议；
- `collect_str` 格式化失败；
- 外部 serializer 的自定义错误。

粗粒度 category 为 `Number`、`ObjectKey`、`RawValue`、`Capacity`、
`SerializerContract` 和 `Custom`。下游既可穷举 `kind()`，也可按 `category()` 制定策略；
辅助访问器只返回不敏感的 signedness、key shape、collection kind 或 serializer-state reason。
这些枚举不使用 `non_exhaustive`，新增变体是明确的破坏性变更。

`JsonEncodeError` 继续区分操作层的预算、无效 `RawValue`、序列化和 writer 错误。
第三方 `Serialize::custom` 提供的任意文本统一映射为 `CustomSerialization`，不会进入公开的
display、debug 或错误来源链。

## 性能模型与实验边界

严格编码不可避免地执行 Serde 遍历、数字与键校验和输出生成；只在限制启用时才应支付资源
计量成本。基准矩阵必须区分 `serde_json`、strict-only、value-only、output-only、full、
owned/reused session、incremental writer，以及 numeric、string、object 和 `RawValue` 形态。

本轮实验结论：

1. 无 output limit 的 owned-buffer 路径直接写 `Vec<u8>`，保留 value accounting 和
   `RawValue` 错误传播；该优化达到门槛并保留。
2. E1 后 strict-only 与直接 `serde_json` 的差距已低于复制一套专用 serializer 所要求的
   5% 收益上限，因此不实施无 value limit 的第二套 serializer。
3. 只有 profiler 明确指向 `rs-budget::try_admit` 时才引入 operation-local admission plan。
   当前主机禁止 perf 采样，且 benchmark 没有提供足够证据，因此不增加该复杂度。
4. `RawValue` 必须先完整验证才能安全提交外部输出。把 scanner 与复制强耦合会增加状态机和
   partial-output 风险，现有数据不足以支持实施。
5. 有 output limit 的 owned buffer 缓存本次操作的剩余额度。成功写入不再借用共享
   `RefCell` 执行完整预算检查；失败时仍回退到累计长度检查，保持 quantity 与 budget
   错误语义不变。

完整数据和复现命令见 [JSON 性能证据日志](benchmark_baseline.zh_CN.md)。

## Tree 快路径

`JsonValueTransaction::has_limits()` 是不泄露内部状态的只读查询。reader 在事务无限制时只做
遍历和 visitor 回调；mutator 分别按 input/output transaction 是否有限决定是否执行变更前后
accounting。`tree_bench` 对 large array、large object 和 deep tree 分别报告 visitor floor、
unlimited、bounded，以及 mutator 的四种限制组合。行为回归必须证明快慢路径产生相同回调。

## 下游安全边界

`rs-redact` 的三处无限 decoder 按职责处理：独立 Serde 发布路径改用策略中的 input/value
limits，并以显式栈执行额外 domain-scope admission；已被两阶段 admission 替代的 HTTP NDJSON
旧函数直接删除；enabled JSON text 路径复用 admission 得到的 `Value`，disabled 路径只做
bounded copy，不再伪装成 parser。结果是这些路径不再包含 `JsonDecoder::unlimited()`，同时
每个非空 NDJSON 行仍只解析一次。

## 验证要求

所有可观察行为先有回归测试。最终验证包括各修改仓库的 `align-ci.sh`、`ci-check.sh`、
文档测试、Miri、fuzz、coverage、feature matrix、下游编译，以及固定 CPU 的完整 Criterion
编码与 tree 基准。benchmark 只描述同机事实，不承诺跨机器倍率。
