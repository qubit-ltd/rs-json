// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Names the source representation used behind JSON encoding errors.

use crate::encode::JsonEncodeErrorSource;

/// Crate-private name for the public owned-source representation.
pub(in crate::encode) type JsonEncodeFailure<R, Q> = JsonEncodeErrorSource<R, Q>;
