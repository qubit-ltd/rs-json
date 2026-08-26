// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines stable errors returned by strict JSON value encoding.

use std::fmt::Display;

use thiserror::Error;

/// Stable diagnostic emitted by Serde adapters for rejected non-finite values.
const NON_FINITE_FLOAT_MESSAGE: &str = "non-finite floating-point value";

/// Failure produced while projecting a serializable value into strict JSON.
///
/// The error intentionally exposes stable categories instead of retaining
/// third-party diagnostic text. This keeps callers independent from Serde and
/// serde_json wording changes.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum JsonValueEncodeError {
    /// A direct or nested floating-point value was not finite.
    #[error("non-finite float")]
    NonFiniteFloat,
    /// The Serde representation could not be expressed as a strict JSON value.
    #[error("JSON value serialization failed")]
    Serialization,
}

impl serde::ser::Error for JsonValueEncodeError {
    /// Classifies a custom Serde diagnostic into a stable public category.
    fn custom<T>(message: T) -> Self
    where
        T: Display,
    {
        if message.to_string() == NON_FINITE_FLOAT_MESSAGE {
            Self::NonFiniteFloat
        } else {
            Self::Serialization
        }
    }
}
