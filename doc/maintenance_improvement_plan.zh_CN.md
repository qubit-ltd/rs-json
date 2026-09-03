# 维护改进实施计划

[English](maintenance_improvement_plan.md)

## 执行顺序

1. 先为 `rs-ci` 可选项目钩子增加契约测试，再在本地 CI 和可复用工作流中实现，
   并补充契约文档。
2. 提交并同步 `rs-ci`，最终回到 `dev-starfish`；随后通过
   `update-submodule.sh` 更新 `rs-json/.rs-ci`。
3. 先增加 `JsonEncodeErrorSource` 失败测试，再实现并记录枚举和
   `into_source`，然后迁移 `rs-config`、`rs-metadata`、`rs-value` 的错误映射。
4. 增加唯一的 crate-private `RawValue` 兼容标记，并迁移两个内部使用点。
5. 增加十进制宽度、结构化诊断、fuzz 语料和可变树不变量覆盖。
6. 以回归测试为基础，用 `JsonTreeReader::account` 替换 `rs-datatype` 中无行为的
   accounting visitor。
7. 增加可变遍历安全说明并补齐 Rustdoc。
8. 对齐中英文 README、用户指南、设计文档、benchmark 证据、changelog 和迁移指南。
9. 对每个有改动的 crate/仓库运行 `align-ci.sh` 和 `ci-check.sh`。
10. 逐项对照设计和本计划检查最终文件；发现缺口后继续修复并重跑检查，直至无遗漏。
11. 将每个修改仓库的全部当前变更按主题拆成英文提交，推送 `dev-starfish`，将其
    fast-forward 到 `dev`、`main` 并推送，最终停留在干净的 `dev-starfish`。

## 验证矩阵

| 范围 | 权威证据 |
| --- | --- |
| CI 钩子 | `rs-ci` 单元测试、本地脚本文本、可复用工作流文本 |
| 公共错误 API | 编译测试、变体测试、下游穷尽匹配 |
| RawValue 兼容 | 唯一标记定义及两个导入点 |
| 数值边界 | 对 9、10、99、100、255 的明确字节/value 预算断言 |
| Fuzz 不变量 | 已提交语料目录及 fuzz workspace 测试/构建 |
| 可变遍历 | 安全文档及已配置 Miri 结果 |
| 文档 | 双语互链和语义对照 |
| 逐 crate CI | 最新一次 `align-ci.sh`、`ci-check.sh` 成功输出 |
| Git 交付 | 三分支提交一致、推送成功、最终工作树干净 |

