# 维护改进设计

[English](maintenance_improvement_design.md)

## 范围

本轮维护不新增产品功能，覆盖 `rs-json` 的所有权编码错误接口、私有
`serde_json::RawValue` 兼容边界、CI 扩展、回归与 fuzz 测试、可变树安全文档、
Rustdoc 和中英文项目文档，并适配直接受影响的下游 crate。

修改范围包括 `rs-json`、`rs-ci`、`rs-datatype`、`rs-config`、
`rs-metadata` 和 `rs-value`。除非兼容验证证明必须修改源码，`rs-http` 与
`rs-redact` 只作为兼容性消费者验证。

## 所有权编码错误接口

`JsonEncodeErrorSource<R, Q>` 暴露 `JsonEncodeError<R, Q>` 的四类所有权来源：

- `Budget(MeasuredBudgetError<R, Q>)`
- `InvalidRawJson(JsonSyntaxError)`
- `Serialize(JsonSerializationError)`
- `Write(std::io::Error)`

`JsonEncodeError::into_source` 消耗错误并返回该枚举。现有 `kind` 和 `into_*`
方法继续保留以兼容已有调用方。下游通过穷尽匹配新枚举完成错误映射，不再把
`kind()`、可失败的提取方法和 `expect` 断言组合使用。

## `RawValue` 兼容边界

`serde_json::RawValue` 使用的私有标记由一个 crate-private 兼容模块统一维护。
通用编码兼容层和 `JsonValue` 序列化器都从该模块导入标记，使对上游私有约定的
依赖保持显式，并为未来升级 `serde_json` 提供唯一审计入口。

## 项目专属 CI 检查

`rs-ci` 提供可选的项目根目录 `project-ci-check.sh` 钩子。钩子不存在时不改变
现有行为；存在时必须可执行，否则 CI 给出可操作的失败信息。本地完整检查脚本
和可复用 GitHub Actions 工作流分别从项目根目录执行一次钩子，并继承已解析的
工具链环境。

`rs-json` 用该钩子执行文档示例测试和 `fuzz/Cargo.toml` 中的普通测试，包括
JSON 数字契约测试。根目录 `ci-check.sh` 仅委托 `.rs-ci/ci-check.sh`，避免重复
执行钩子。

## 测试与安全文档

回归覆盖包括十进制宽度边界、编码错误的全部所有权变体、下游错误映射、结构化
无效 JSON 诊断和 `rs-datatype` accounting 路径。每个 fuzz target 都提供小型、
可读的版本化种子；可变树 fuzz 成功路径还证明所有 `secret` 字段均已删除。

保留现有可变遍历实现。在模块级安全说明中记录 cursor、frame、栈和节点身份
不变量，并继续使用已有 Miri 配置验证这些边界。

## 文档

英文和简体中文文档保持语义一致。README 区分 crates.io 已发布版本与未发布的
0.8 开发状态，并说明 Git/path 安装。用户指南准确区分严格空输入错误与规范化
结果。设计文档一致说明 transport aggregation，数字契约文档互链，两种语言的
benchmark 证据均完整。Rustdoc 明确 `Option` 语义、返回行为、失败条件和当前
decoder 术语。

新增双语 changelog 和 0.3 到 0.8 迁移指南。不新增 `SECURITY.md`。

## 下游边界

`rs-datatype` 直接调用 `JsonTreeReader::account` 并删除无行为的 accounting
visitor。`rs-config`、`rs-metadata` 和 `rs-value` 使用
`JsonEncodeErrorSource`。不包含无关的下游重构。

