// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared public-API adapters for encoder tests.

use std::fmt::Debug;
use std::io::Write;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::encode::JsonEncodeError;
use qubit_json::encode::JsonEncoder;
use serde::Serialize;

/// Encodes one value through the stateful encoder API.
pub(crate) fn encode<T, R, Q>(
    value: &T,
    session: &mut JsonEncodeSession<'_, R, Q>,
) -> Result<Vec<u8>, JsonEncodeError<R, Q>>
where
    T: Serialize + ?Sized,
    R: Clone + Debug + 'static,
    Q: ResourceQuantity + 'static,
{
    let placeholder = JsonEncodeSession::owned(JsonEncodeLimits::<R, Q>::builder().build());
    let owned = std::mem::replace(session, placeholder);
    let mut encoder = JsonEncoder::new(owned);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| encoder.to_vec(value)));
    *session = encoder.into_session();
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Writes one buffered document through the stateful encoder API.
pub(crate) fn write_buffered<W, T, R, Q>(
    writer: W,
    value: &T,
    session: &mut JsonEncodeSession<'_, R, Q>,
) -> Result<(), JsonEncodeError<R, Q>>
where
    W: Write,
    T: Serialize + ?Sized,
    R: Clone + Debug + 'static,
    Q: ResourceQuantity + 'static,
{
    let placeholder = JsonEncodeSession::owned(JsonEncodeLimits::<R, Q>::builder().build());
    let owned = std::mem::replace(session, placeholder);
    let mut encoder = JsonEncoder::new(owned);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| encoder.write_buffered(writer, value)));
    *session = encoder.into_session();
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Streams one document through the stateful encoder API.
pub(crate) fn write_incremental<W, T, R, Q>(
    writer: W,
    value: &T,
    session: &mut JsonEncodeSession<'_, R, Q>,
) -> Result<(), JsonEncodeError<R, Q>>
where
    W: Write,
    T: Serialize + ?Sized,
    R: Clone + Debug + 'static,
    Q: ResourceQuantity + 'static,
{
    let placeholder = JsonEncodeSession::owned(JsonEncodeLimits::<R, Q>::builder().build());
    let owned = std::mem::replace(session, placeholder);
    let mut encoder = JsonEncoder::new(owned);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        encoder.write_incremental(writer, value)
    }));
    *session = encoder.into_session();
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
