# JSON 依赖维护

`qubit-json` 精确锁定 `serde_json = 1.0.151` 和 `zmij = 1.0.23`。这不是常规的
最小版本约束：严格编码器需要识别 `serde_json` 的私有 Number/RawValue Serde
协议，并使用 `zmij` 计算与该版本 `serde_json` 一致的浮点 JSON lexeme 长度。

这两个上游实现细节不承诺具有 semver 稳定性。因此升级时必须在同一变更中评估并更新
两个版本，不能单独放宽或升级其中一个。

## 升级步骤

1. 在 `Cargo.toml` 同时更新 `serde_json` 与 `zmij`，并更新 lockfile。
2. 检查 `src/encode/serde_compat/` 中的私有 Number/RawValue token 及其对应的
   serde 形状。
3. 运行 private protocol、RawValue、arbitrary precision number 与 float lexeme
   length 测试：

   ```bash
   cargo test --test tests serde_json_compat
   cargo test --test tests json_lexeme_length
   cargo test --test tests json_text_encoder
   ```

4. 运行 `./align-ci.sh`、`./style-check.sh`、`./ci-check.sh`，并编译 benchmark、
   fuzz targets 和直接下游 crate。
5. 使用 `cargo bench --bench budgeted_serde_json` 对比编码结果；若 private protocol
   或浮点长度行为改变，先更新兼容实现和回归测试，再发布依赖升级。
