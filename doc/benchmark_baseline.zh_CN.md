# `rs-json` 解码基准基线

## 记录范围

本记录保存 `decoder_bench` 在 `2026-07-23` 的一次完整执行结果中，
下游形态的严格字节解码与宽松类型解码的关键中位数。它用于复核
`rs-http` 的响应与 SSE 解码场景，不是跨机器或跨运行的性能阈值。

基准命令：

```bash
cargo +1.94.0 bench --bench decoder_bench
```

执行基线提交为 `081fb98f73ba7cd5dc7a930f9d0c98cf70a5bd92`。完整执行还覆盖
公开入口、控制字符规范化和代表性失败路径；Criterion 的原始统计保存在本地
`target/criterion/`，不纳入版本控制。

## 环境

- CPU：Intel(R) Core(TM) i5-9600K CPU @ 3.70GHz，6 核、每核 1 线程。
- 操作系统：Linux 6.17.0-35-generic x86_64。
- 工具链：rustc 1.94.0 (4a4ef493e 2026-03-02)，LLVM 21.1.8。

## 严格字节解码中位数

| 输入大小 | `serde_json::from_slice` | 复用严格 decoder | 按次构造严格 decoder |
| --- | ---: | ---: | ---: |
| 1 KiB | 231.56 ns | 239.74 ns | 241.28 ns |
| 64 KiB | 10.978 µs | 10.944 µs | 10.895 µs |
| 1 MiB | 184.94 µs | 185.18 µs | 186.52 µs |

“按次构造严格 decoder”在每个 Criterion 迭代内执行
`LenientJsonDecoder::new(JsonDecodeOptions::strict())` 后再调用 `decode_slice`，与当前
`rs-http` 的响应和 SSE 调用方式一致。三种严格路径的数值接近，比较时应以同机多次运行及
Criterion 的置信区间为准，而非将单次结果视为回归结论。

## 宽松类型解码中位数

| 输入大小 | 普通 JSON | Markdown fenced JSON | 每 1024 字节一个原始控制字符 |
| --- | ---: | ---: | ---: |
| 1 KiB | 311.00 ns | 355.11 ns | 2.4553 µs |
| 64 KiB | 13.337 µs | 13.656 µs | 241.17 µs |
| 1 MiB | 223.03 µs | 220.23 µs | 2.2784 ms |

该组通过 `LenientJsonDecoder::default().decode_object` 解码类型化记录，覆盖 `rs-http`
宽松 SSE 消息所依赖的普通、fenced 与控制字符规范化路径。控制字符密度会显著影响结果，
因此应单独与相同输入形态的历史运行比较。

## 2026-07-24 追加基线

本次在提交 `76e027c35639542c8df1bac846907cc85e37043d` 上完整重跑
`cargo +1.94.0 bench --bench decoder_bench`。环境与上次记录相同；下表使用
Criterion `median.point_estimate`，保留历史记录以便按输入形态比较。

### 严格字节解码中位数

| 输入大小 | `serde_json::from_slice` | 复用严格 decoder | 按次构造严格 decoder |
| --- | ---: | ---: | ---: |
| 1 KiB | 244.69 ns | 242.00 ns | 243.75 ns |
| 64 KiB | 10.959 µs | 11.400 µs | 11.040 µs |
| 1 MiB | 182.17 µs | 188.68 µs | 186.63 µs |

### 宽松类型解码中位数

| 输入大小 | 普通 JSON | 含 Unicode、无原始控制字符 | Markdown fenced JSON | Pretty JSON | 每 1024 字节一个原始控制字符 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 KiB | 580.23 ns | 581.39 ns | 615.90 ns | 2.3020 µs | 2.4164 µs |
| 64 KiB | 30.450 µs | 30.397 µs | 29.871 µs | 131.09 µs | 141.79 µs |
| 1 MiB | 506.53 µs | 491.52 µs | 495.86 µs | 2.1129 ms | 2.4197 ms |

新增的 Unicode、无原始控制字符输入用于单独观察规范化预扫描；它不应与含控制字符的
输入直接比较。由于本机当前 CPU 调频状态会影响单次结果，这些数值只作为同机、同输入形态
的后续回归基线，不构成跨机器性能阈值。
