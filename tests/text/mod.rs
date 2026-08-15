// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
mod json_decode_error_tests;
mod json_decode_session_tests;
mod json_encode_accounting_tests;
mod json_encode_error_tests;
mod json_encode_session_tests;
mod json_lexical_admission_tests;
mod json_output_tests;
mod json_syntax_error_reason_tests;
mod json_syntax_error_tests;
mod json_text_decoder_budget_tests;
mod json_text_decoder_tests;
mod json_text_encoder_tests;
mod serde_json_compat_tests;
mod support;

mod internal;

pub(crate) use support::json_encode_test_support;
