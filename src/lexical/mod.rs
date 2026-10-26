// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared crate-private lexical infrastructure.

mod internal;
mod json_lexical_error;
mod json_lexical_error_reason;
mod json_lexical_event;
mod json_lexical_failure;
mod json_lexical_scanner;
mod json_number_model;

pub(crate) use internal::json_lexical_cursor::line_column_at;
pub(crate) use json_lexical_error::JsonLexicalError;
pub(crate) use json_lexical_error_reason::JsonLexicalErrorReason;
pub(crate) use json_lexical_event::JsonLexicalEvent;
pub(crate) use json_lexical_failure::JsonLexicalFailure;
pub(crate) use json_lexical_scanner::JsonLexicalScanner;
pub(crate) use json_number_model::JsonNumberModel;
