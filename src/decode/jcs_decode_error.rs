// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Error returned by the JCS JSON text decoder.

use std::error::Error;
use std::fmt;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonResource;

use super::JsonDecodeError;

/// JCS decoding failure, with duplicate object keys represented explicitly.
#[derive(Debug, Clone)]
pub enum JcsDecodeError<R = JsonResource, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    /// A lexical, input, budget, or Serde decoding failure.
    Decode(JsonDecodeError<R, Q>),
    /// An object contains two keys equal after JSON escape decoding.
    DuplicateKey {
        /// One-based line reported by the JSON deserializer.
        line: usize,
        /// One-based column reported by the JSON deserializer.
        column: usize,
    },
}

impl<R, Q> fmt::Display for JcsDecodeError<R, Q>
where
    R: fmt::Debug,
    Q: ResourceQuantity,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(formatter),
            Self::DuplicateKey { line, column } => {
                write!(formatter, "duplicate JSON object key at line {line}, column {column}")
            }
        }
    }
}

impl<R, Q> Error for JcsDecodeError<R, Q>
where
    R: fmt::Debug + 'static,
    Q: ResourceQuantity + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::DuplicateKey { .. } => None,
        }
    }
}

impl<R, Q> From<JsonDecodeError<R, Q>> for JcsDecodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    fn from(error: JsonDecodeError<R, Q>) -> Self {
        Self::Decode(error)
    }
}
