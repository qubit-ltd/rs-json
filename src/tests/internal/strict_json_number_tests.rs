// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for strict JSON number admission and representation.

use crate::encode::JsonIntegerSignedness;
use crate::encode::JsonSerializationErrorKind;
use crate::internal::StrictJsonNumber;

#[test]
fn test_from_i128_accepts_signed_and_unsigned_64_bit_range() {
    assert_eq!(
        StrictJsonNumber::from_i128(i64::MIN as i128).unwrap().as_i64(),
        Some(i64::MIN)
    );
    assert_eq!(StrictJsonNumber::from_i128(-1).unwrap().as_i64(), Some(-1));
    assert_eq!(StrictJsonNumber::from_i128(0).unwrap().as_i64(), Some(0));
    assert_eq!(
        StrictJsonNumber::from_i128(u64::MAX as i128).unwrap().as_u64(),
        Some(u64::MAX)
    );
}

#[test]
fn test_from_i128_rejects_values_outside_integer_union() {
    for value in [i64::MIN as i128 - 1, u64::MAX as i128 + 1] {
        assert_eq!(
            StrictJsonNumber::from_i128(value).unwrap_err().kind(),
            JsonSerializationErrorKind::IntegerOutOfRange {
                signedness: JsonIntegerSignedness::Signed
            },
        );
    }
}

#[test]
fn test_from_u128_enforces_u64_range() {
    assert_eq!(
        StrictJsonNumber::from_u128(u64::MAX as u128).unwrap().as_u64(),
        Some(u64::MAX)
    );
    assert_eq!(
        StrictJsonNumber::from_u128(u128::MAX).unwrap_err().kind(),
        JsonSerializationErrorKind::IntegerOutOfRange {
            signedness: JsonIntegerSignedness::Unsigned
        },
    );
}

#[test]
fn test_parse_preserves_negative_zero_and_finite_fraction() {
    let negative_zero = StrictJsonNumber::parse("-0").unwrap();
    assert_eq!(negative_zero.as_f64().unwrap().to_bits(), (-0.0_f64).to_bits());
    assert_eq!(StrictJsonNumber::parse("1.5").unwrap().as_f64(), Some(1.5));
}

#[test]
fn test_parse_maps_numeric_range_errors() {
    for text in ["1e400", "1e-400"] {
        assert_eq!(
            StrictJsonNumber::parse(text).unwrap_err().kind(),
            JsonSerializationErrorKind::NonFiniteFloat
        );
    }
    assert_eq!(
        StrictJsonNumber::parse("18446744073709551615").unwrap().as_u64(),
        Some(u64::MAX)
    );
    assert_eq!(
        StrictJsonNumber::parse("18446744073709551616").unwrap_err().kind(),
        JsonSerializationErrorKind::IntegerOutOfRange {
            signedness: JsonIntegerSignedness::Unsigned
        },
    );
}

#[test]
fn test_parse_rejects_non_single_number_tokens() {
    for text in [" 1", "1 ", "1 2", "{}", "\"1\"", "+1", "01", ""] {
        assert_eq!(
            StrictJsonNumber::parse(text).unwrap_err().kind(),
            JsonSerializationErrorKind::InvalidNumberRepresentation
        );
    }
}
