# Qubit JSON user guide

`qubit-json` is a resource-aware boundary around Serde and serde_json. It does
not replace serde_json's parser or define a new value tree. Use it when input,
decoded values, or output must be constrained by caller-owned budgets.

## Choose an entry point

- Use `JsonDecoder` for strict JSON text and complete syntax, number-range, and
  value-budget admission.
- Use `NormalizingJsonDecoder` only for explicitly allowed BOM, whitespace,
  Markdown-fence, or control-character normalization before the same strict
  admission path.
- Use `JsonEncoder` for budgeted compact JSON output.
- Use `JsonValueSeed` when another Serde deserializer already owns the input;
  it budgets decoded events but cannot validate original JSON lexemes.
- Use `JsonTreeReader`, `JsonTreeMutator`, or `JsonTreeBudgetTracker` for an
  existing `serde_json::Value`.

## Strict decode and encode

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_input_bytes(4096)
    .max_number_bytes(20)
    .build();
let value: serde_json::Value =
    JsonDecoder::owned(limits).decode_str(r#"{"id":18446744073709551615}"#)?;
assert_eq!(value["id"], serde_json::json!(u64::MAX));
# Ok::<(), qubit_json::decode::JsonDecodeError<JsonResource>>(())
```

Construct sessions explicitly when accounting must accumulate across calls.
Decode attempts retain input charges after failure, while staged decoded-value
charges commit only after complete typed success. Buffered encoding commits a
complete output; incremental encoding can leave an accepted writer prefix.

## Numeric limits

Negative integers fit `i64`; non-negative integers fit `u64`; fractions and
exponents must be finite `f64`. See the [number contract](number_contract.md)
for boundaries, browser `BigInt` requirements, exact decimal guidance, and the
difference between numeric range and `NumberBytes`.

## Error handling

Strict decode separates `Budget`, `Syntax`, and typed `Deserialize` failures.
Range failures are syntax-domain reasons `IntegerOutOfRange` and
`FloatOutOfRange`, not budget failures. Strict encode separates budget,
invalid raw JSON, Serde serialization, and writer failures. Detailed
normalizing diagnostics can contain input-derived data and should be enabled
only at trusted boundaries.

## Operational guidance

Choose finite limits for every untrusted boundary, especially input/output
bytes, depth, nodes, collection sizes, key/string bytes, and number bytes.
Treat application constraints—identifier policy, decimal scale, schemas, and
authorization—as separate validation after JSON admission.
