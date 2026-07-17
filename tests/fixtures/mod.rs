// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared typed fixtures for public decoder tests.

mod exact_integer;
mod message;
mod single_value;
mod user;

pub(crate) use exact_integer::ExactInteger;
pub(crate) use message::Message;
pub(crate) use single_value::SingleValue;
pub(crate) use user::User;
