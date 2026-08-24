# JSON 依赖维护

`qubit-json` 使用 `serde_json = "1.0.151"` 的常规 caret 约束。`1.0.151` 是当前
兼容测试覆盖的最低版本，不是唯一允许版本。严格编码器需要识别 `serde_json` 的
RawValue Serde 私有协议，因此不能只依赖版本号推断兼容性。

有限浮点 JSON lexeme 长度通过 `serde_json::ser::CompactFormatter` 的公开接口
计算，因此会自动使用当前 serde_json 的实际格式化实现。`zmij` 只是 serde_json 的
传递依赖，`qubit-json` 不直接依赖或调用它。

## 升级步骤

1. 在 `Cargo.toml` 更新最低 `serde_json` 版本（如有必要），并更新 lockfile。
2. 检查 `src/encode/serde_compat/` 中的 RawValue token 及其对应 Serde 形状；确认依赖图
   没有启用 `serde_json/arbitrary_precision`。
3. 运行 RawValue、64 位数字边界与 float lexeme length 测试：

   ```bash
   cargo test --test tests serde_json_compat
   cargo test --test tests json_lexeme_length
   cargo test --test tests json_text_encoder
   ```

4. 使用 lockfile 当前解析版本运行 `./align-ci.sh`、`./style-check.sh`、
   `./ci-check.sh`，并编译 benchmark、fuzz targets 和直接下游 crate。升级验证失败时
   修复兼容层或提高最低版本，不能用精确锁定长期阻止兼容版本解析。
5. 使用 `cargo bench --bench budgeted_serde_json` 对比编码结果；若 RawValue protocol
   或 serde_json formatter 行为改变，先更新兼容实现和回归测试，再发布依赖升级。
