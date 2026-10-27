// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Errors from budgeted JSON value projection.

use std::fmt;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;

use crate::encode::JsonCollectionKind;
use crate::encode::JsonIntegerSignedness;
use crate::encode::JsonMapKeyKind;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorCategory;
use crate::encode::JsonSerializationErrorKind;
use crate::encode::JsonSerializerStateError;

/// Failure while projecting a Serde value into a budgeted JSON tree.
#[non_exhaustive]
#[derive(Debug)]
pub enum JsonValueEncodeError<R, Q>
where
    Q: ResourceQuantity,
{
    /// The caller's resource transaction rejected a value.
    Budget(MeasuredBudgetError<R, Q>),
    /// The Serde value could not be represented as strict JSON.
    Serialization(JsonSerializationError),
}

impl<R, Q> JsonValueEncodeError<R, Q>
where
    Q: ResourceQuantity,
{
    /// Returns the underlying serialization error when Serde projection failed.
    pub const fn serialization_error(&self) -> Option<&JsonSerializationError> {
        match self {
            Self::Serialization(error) => Some(error),
            Self::Budget(_) => None,
        }
    }

    /// Returns the stable serialization failure kind, if applicable.
    pub const fn kind(&self) -> Option<JsonSerializationErrorKind> {
        match self {
            Self::Serialization(error) => Some(error.kind()),
            Self::Budget(_) => None,
        }
    }

    /// Returns the broad serialization category, if applicable.
    pub const fn category(&self) -> Option<JsonSerializationErrorCategory> {
        match self {
            Self::Serialization(error) => Some(error.category()),
            Self::Budget(_) => None,
        }
    }

    /// Reports whether the serialization failure concerns a JSON number.
    pub const fn is_number_error(&self) -> bool {
        matches!(self, Self::Serialization(error) if error.is_number_error())
    }

    /// Reports whether the serialization failure concerns an object key.
    pub const fn is_map_key_error(&self) -> bool {
        matches!(self, Self::Serialization(error) if error.is_map_key_error())
    }

    /// Reports whether the serialization failure concerns a raw JSON value.
    pub const fn is_raw_value_error(&self) -> bool {
        matches!(self, Self::Serialization(error) if error.is_raw_value_error())
    }

    /// Reports whether Serde violated a serializer protocol contract.
    pub const fn is_serializer_contract_error(&self) -> bool {
        matches!(self, Self::Serialization(error) if error.is_serializer_contract_error())
    }

    /// Returns the rejected integer signedness, if applicable.
    pub const fn integer_signedness(&self) -> Option<JsonIntegerSignedness> {
        match self {
            Self::Serialization(error) => error.integer_signedness(),
            Self::Budget(_) => None,
        }
    }

    /// Returns the unsupported map-key category, if applicable.
    pub const fn map_key_kind(&self) -> Option<JsonMapKeyKind> {
        match self {
            Self::Serialization(error) => error.map_key_kind(),
            Self::Budget(_) => None,
        }
    }

    /// Returns the collection whose length overflowed, if applicable.
    pub const fn collection_kind(&self) -> Option<JsonCollectionKind> {
        match self {
            Self::Serialization(error) => error.collection_kind(),
            Self::Budget(_) => None,
        }
    }

    /// Returns the serializer protocol state failure, if applicable.
    pub const fn serializer_state_error(&self) -> Option<JsonSerializerStateError> {
        match self {
            Self::Serialization(error) => error.serializer_state_error(),
            Self::Budget(_) => None,
        }
    }
}

impl<R, Q> fmt::Display for JsonValueEncodeError<R, Q>
where
    Q: ResourceQuantity,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(_) => f.write_str("JSON value budget rejected serialization"),
            Self::Serialization(error) => write!(f, "{error}"),
        }
    }
}

impl<R, Q> std::error::Error for JsonValueEncodeError<R, Q>
where
    R: fmt::Debug + 'static,
    Q: ResourceQuantity + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            Self::Serialization(error) => Some(error),
        }
    }
}
