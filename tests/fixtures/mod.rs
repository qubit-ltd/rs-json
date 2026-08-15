// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared typed fixtures for public decoder tests.

mod byte_buffer;
mod counted_failure;
mod exact_integer;
mod internal;
mod json_test_limits;
mod message;
mod public_choice;
mod single_value;
mod user;

pub(crate) use byte_buffer::ByteBuffer;
pub(crate) use counted_failure::CountedFailure;
pub(crate) use counted_failure::deserialize_calls;
pub(crate) use counted_failure::reset_deserialize_calls;
pub(crate) use exact_integer::ExactInteger;
pub(crate) use json_test_limits::JsonTestLimits;
pub(crate) use message::Message;
pub(crate) use public_choice::PublicChoice;
pub(crate) use single_value::SingleValue;
pub(crate) use user::User;
