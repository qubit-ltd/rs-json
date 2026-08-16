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
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    JsonEncoder::new(session).to_vec(value)
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
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    JsonEncoder::new(session).write_buffered(writer, value)
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
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    JsonEncoder::new(session).write_incremental(writer, value)
}
