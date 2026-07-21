# `rs-json` 解码基准基线

## 记录范围

本记录保存 `decoder_bench` 在 `2026-07-22` 的一次完整执行结果中，
`downstream-plain-bytes` 组的关键中位数。它用于复核严格字节解码的下游场景，
不是跨机器或跨运行的性能阈值。

基准命令：

```bash
cargo bench --bench decoder_bench
```

执行基线提交为 `52bcdbaa119a4d5ee15d89c098e0ce4262dfc04b`，并已应用本次新增的
“按次构造严格 decoder”基准。本次完整执行还覆盖公开入口、宽松类型解码、失败路径和
控制字符规范化；Criterion 的完整原始统计保存在本地 `target/criterion/`，不纳入版本控制。

## 环境

- CPU：Intel(R) Core(TM) i5-9600K CPU @ 3.70GHz，6 核、每核 1 线程。
- 操作系统：Linux 6.17.0-35-generic x86_64。
- 工具链：rustc 1.94.0 (4a4ef493e 2026-03-02)，LLVM 21.1.8。

## 严格字节解码中位数

| 输入大小 | `serde_json::from_slice` | 复用严格 decoder | 按次构造严格 decoder |
| --- | ---: | ---: | ---: |
| 1 KiB | 247.58 ns | 242.36 ns | 241.30 ns |
| 64 KiB | 10.686 µs | 10.917 µs | 10.693 µs |
| 1 MiB | 185.579 µs | 184.173 µs | 185.985 µs |

“按次构造严格 decoder”在每个 Criterion 迭代内执行
`LenientJsonDecoder::new(JsonDecodeOptions::strict())` 后再调用 `decode_slice`，与当前
`rs-http` 的响应和 SSE 调用方式一致。1 KiB 的数值处于置信区间重叠和环境噪声较敏感的区间；
比较时应以同机多次运行及 Criterion 的置信区间为准，而非将单次结果视为回归结论。
