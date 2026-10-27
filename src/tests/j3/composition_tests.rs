// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Cross-API composition and compatibility contract tests.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use serde_json::Value;
use serde_json::json;

use crate::decode::JsonDecodeNumberModel;
use crate::decode::JsonDecoder;
use crate::decode::JsonValidatedDecodeError;
use crate::decode::JsonValidatedDecodeOptions;
use crate::encode::JsonEncoder;
use crate::format::JsonPrettyOptions;
use crate::format::format_json_tokens;
use crate::value::JsonValueEncoder;
#[cfg(feature = "jcs")]
use crate::value::SafeJsonValue;
