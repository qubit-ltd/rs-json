use crate::decode::{JsonDecodeNumberModel, JsonNumberToken, JsonNumberTokenError};
use crate::encode::JsonSerializationErrorKind;
use crate::internal::StrictJsonNumber;

#[test]
fn test_exact_safe_integer_uses_decimal_arithmetic() {
    for (text, expected) in [
        ("90071992547409910e-1", 9_007_199_254_740_991_i64),
        ("1.00e2", 100),
        ("1.0e1", 10),
        ("-0", 0),
        ("-9007199254740991", -9_007_199_254_740_991),
    ] {
        assert_eq!(
            JsonNumberToken::from_validated(text, 7)
                .exact_safe_integer()
                .unwrap(),
            expected
        );
    }
    for text in ["9007199254740991001e-3", "9007199254740991.1", "0.1"] {
        assert!(matches!(
            JsonNumberToken::from_validated(text, 0).exact_safe_integer(),
            Err(JsonNumberTokenError::Fractional)
        ));
    }
    for text in ["9007199254740992", "-9007199254740992"] {
        assert!(matches!(
            JsonNumberToken::from_validated(text, 0).exact_safe_integer(),
            Err(JsonNumberTokenError::OutOfSafeIntegerRange)
        ));
    }
}

#[test]
fn test_exact_safe_integer_handles_extreme_exponents_in_constant_space() {
    let zero = format!("0e{}", "9".repeat(100_000));
    assert_eq!(
        JsonNumberToken::from_validated(&zero, 0)
            .exact_safe_integer()
            .unwrap(),
        0
    );
    let tiny = format!("1e-{}", "9".repeat(100_000));
    assert!(matches!(
        JsonNumberToken::from_validated(&tiny, 0).exact_safe_integer(),
        Err(JsonNumberTokenError::Fractional)
    ));
    let huge = format!("1e{}", "9".repeat(100_000));
    assert!(matches!(
        JsonNumberToken::from_validated(&huge, 0).exact_safe_integer(),
        Err(JsonNumberTokenError::OutOfSafeIntegerRange)
    ));
}

#[test]
fn test_number_model_fallback_only_covers_integer_range_overflow() {
    let strict = JsonDecodeNumberModel::Integer64;
    let fallback = JsonDecodeNumberModel::Integer64WithFiniteFallback;
    assert_eq!(
        StrictJsonNumber::from_admitted_lexeme_with_model("18446744073709551615", strict)
            .unwrap()
            .as_u64(),
        Some(u64::MAX)
    );
    assert!(matches!(
        StrictJsonNumber::from_admitted_lexeme_with_model("18446744073709551616", strict)
            .unwrap_err()
            .kind(),
        JsonSerializationErrorKind::IntegerOutOfRange { .. }
    ));
    let rounded =
        StrictJsonNumber::from_admitted_lexeme_with_model("18446744073709551616", fallback)
            .unwrap();
    assert!(rounded.is_f64());
    assert!(matches!(
        StrictJsonNumber::from_admitted_lexeme_with_model("1e400", fallback)
            .unwrap_err()
            .kind(),
        JsonSerializationErrorKind::NonFiniteFloat
    ));
    assert!(matches!(
        StrictJsonNumber::from_admitted_lexeme_with_model("1e400", strict)
            .unwrap_err()
            .kind(),
        JsonSerializationErrorKind::NonFiniteFloat
    ));
    assert!(
        StrictJsonNumber::from_admitted_lexeme_with_model("1e-400", fallback)
            .unwrap()
            .as_f64()
            .unwrap()
            == 0.0
    );
    let negative_zero = StrictJsonNumber::from_admitted_lexeme_with_model("-0", fallback)
        .unwrap()
        .as_f64()
        .unwrap();
    assert!(negative_zero.is_sign_negative());
    assert!(matches!(
        StrictJsonNumber::from_admitted_lexeme_with_model("0.1", fallback)
            .unwrap()
            .as_f64(),
        Some(0.1)
    ));
}

#[test]
fn test_number_token_preserves_borrowed_slice_and_offset() {
    let source = "prefix 1.00e2 suffix";
    let raw = &source[7..13];
    let token = JsonNumberToken::from_validated(raw, 7);
    assert_eq!(token.raw().as_ptr(), raw.as_ptr());
    assert_eq!(token.raw().len(), raw.len());
    assert_eq!(token.offset(), 7);
}
