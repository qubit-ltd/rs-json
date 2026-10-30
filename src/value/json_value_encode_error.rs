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
///
/// The error distinguishes resource budget rejection, strict JSON
/// serialization failure, and private text payloads that exceed their
/// configured limit. Because this type is non-exhaustive, matches outside this
/// crate must include a fallback arm. After any failure, discard the entire
/// caller-owned resource transaction; do not treat its partial measurements as
/// a successful projection.
///
/// `None` from a serialization query does not indicate success. It means that
/// the error is not a serialization failure.
///
/// # Type Parameters
///
/// * `R` is the resource identifier type used by the budget.
/// * `Q` is the resource quantity type and must implement [`ResourceQuantity`].
///
/// # Examples
///
/// ```
/// use qubit_budget::json::JsonResource;
/// use qubit_budget::json::JsonValueBudget;
/// use qubit_budget::json::JsonValueLimits;
/// use qubit_json::value::JsonValueEncodeError;
/// use qubit_json::value::JsonValueEncoder;
/// use serde_json::value::RawValue;
///
/// let limits = JsonValueLimits::<JsonResource, usize>::new();
/// let mut budget = JsonValueBudget::new(limits);
/// let mut transaction = budget.transaction();
/// let raw = RawValue::from_string("null".to_owned()).unwrap();
/// let error = JsonValueEncoder::new()
///     .encode_in_transaction_with_private_text_limit(&raw, &mut transaction, 0)
///     .unwrap_err();
/// let classification = match &error {
///     JsonValueEncodeError::Budget(_) => "budget",
///     JsonValueEncodeError::Serialization(_) => "serialization",
///     JsonValueEncodeError::PrivateTextLimitExceeded { maximum } => {
///         assert_eq!(*maximum, 0);
///         "private text limit"
///     }
///     _ => "other",
/// };
/// assert_eq!(classification, "private text limit");
/// assert!(error.serialization_error().is_none());
/// assert!(error.kind().is_none());
/// assert!(error.category().is_none());
/// drop(transaction);
/// ```
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
    /// A private RawValue or Number payload exceeded its configured UTF-8 text
    /// limit.
    PrivateTextLimitExceeded {
        /// Maximum UTF-8 byte length allowed for one private text payload.
        maximum: usize,
    },
}

impl<R, Q> JsonValueEncodeError<R, Q>
where
    Q: ResourceQuantity,
{
    /// Returns the underlying serialization error when Serde projection failed.
    ///
    /// # Returns
    ///
    /// Returns `Some` for [`Self::Serialization`]. Returns `None` for
    /// [`Self::Budget`] and [`Self::PrivateTextLimitExceeded`]; `None` means
    /// the failure is not a serialization error, not that projection
    /// succeeded.
    pub const fn serialization_error(&self) -> Option<&JsonSerializationError> {
        match self {
            Self::Serialization(error) => Some(error),
            Self::Budget(_) => None,
            Self::PrivateTextLimitExceeded { .. } => None,
        }
    }

    /// Returns the stable serialization failure kind, if applicable.
    ///
    /// # Returns
    ///
    /// Returns `Some` for [`Self::Serialization`]. Returns `None` for
    /// [`Self::Budget`] and [`Self::PrivateTextLimitExceeded`]; `None` means
    /// the failure has no serialization kind, not that projection succeeded.
    pub const fn kind(&self) -> Option<JsonSerializationErrorKind> {
        match self {
            Self::Serialization(error) => Some(error.kind()),
            Self::Budget(_) => None,
            Self::PrivateTextLimitExceeded { .. } => None,
        }
    }

    /// Returns the broad serialization category, if applicable.
    ///
    /// # Returns
    ///
    /// Returns `Some` for [`Self::Serialization`]. Returns `None` for
    /// [`Self::Budget`] and [`Self::PrivateTextLimitExceeded`]; `None` means
    /// the failure has no serialization category, not that projection
    /// succeeded.
    pub const fn category(&self) -> Option<JsonSerializationErrorCategory> {
        match self {
            Self::Serialization(error) => Some(error.category()),
            Self::Budget(_) => None,
            Self::PrivateTextLimitExceeded { .. } => None,
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
            Self::PrivateTextLimitExceeded { .. } => None,
        }
    }

    /// Returns the unsupported map-key category, if applicable.
    pub const fn map_key_kind(&self) -> Option<JsonMapKeyKind> {
        match self {
            Self::Serialization(error) => error.map_key_kind(),
            Self::Budget(_) => None,
            Self::PrivateTextLimitExceeded { .. } => None,
        }
    }

    /// Returns the collection whose length overflowed, if applicable.
    pub const fn collection_kind(&self) -> Option<JsonCollectionKind> {
        match self {
            Self::Serialization(error) => error.collection_kind(),
            Self::Budget(_) => None,
            Self::PrivateTextLimitExceeded { .. } => None,
        }
    }

    /// Returns the serializer protocol state failure, if applicable.
    pub const fn serializer_state_error(&self) -> Option<JsonSerializerStateError> {
        match self {
            Self::Serialization(error) => error.serializer_state_error(),
            Self::Budget(_) => None,
            Self::PrivateTextLimitExceeded { .. } => None,
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
            Self::PrivateTextLimitExceeded { maximum } => {
                write!(f, "private JSON text exceeded the {maximum}-byte limit")
            }
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
            Self::PrivateTextLimitExceeded { .. } => None,
        }
    }
}
