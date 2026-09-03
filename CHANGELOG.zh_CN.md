# 变更记录

[English](CHANGELOG.md)

本文记录 `qubit-json` 的重要变化。已发布版本遵循语义化版本规则。

## 0.8.0 - 2026

- 将 crate 重组为明确的 `decode`、`encode` 和 `value` 领域。
- 增加由资源记账 session 支撑的严格 decoder 与规范化 decoder。
- 增加有预算约束的严格编码、物化值构造、重复键拒绝和迭代读写遍历 API。
- 明确有符号 `i64`、无符号 `u64` 和有限 `f64` 数字契约。
- 增加结构化、隐私感知的编解码错误模型，包括所有权映射接口
  `JsonDecodeErrorSource` 和 `JsonEncodeErrorSource`。
- 增加 fuzz、Miri、文档示例、兼容性和 benchmark 测试套件。

参见 [0.3 到 0.8 迁移指南](doc/migration_0_3_to_0_8.zh_CN.md)。

## 0.7.0 - 2026

- 为原宽松 decoder 增加 UTF-8 入口、规范化输入限制和默认脱敏诊断。

## 0.6.0 - 2026

- 引入 builder 风格 decoder 配置和更完整的资源限制。

## 0.3.6 - 2026

- 原根级 `LenientJsonDecoder` API 的最后一个 0.3 补丁版本。
