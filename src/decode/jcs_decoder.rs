// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stateful strict JSON decoding with JCS binary64 number semantics.

use std::cell::Cell;
use std::fmt;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_io::Input;

use super::DiagnosticPolicy;
use super::JcsDecodeError;
use super::JsonDecodeError;
use super::JsonDecodeErrorSource;
use super::internal::JcsValueSeed;
use super::internal::JsonDecodeEngine;
use crate::lexical::JsonNumberModel;

/// Decodes strict JSON text into an owned value using finite binary64 numbers.
#[derive(Debug)]
pub struct JcsDecoder<'budget, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    diagnostic_policy: DiagnosticPolicy,
    engine: JsonDecodeEngine<'budget, R, Q>,
}

impl<R, Q> JcsDecoder<'static, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder with the supplied cumulative resource limits.
    pub fn with_limits(limits: JsonDecodeLimits<R, Q>) -> Self {
        Self::new(JsonDecodeSession::from_limits(limits))
    }
}

impl JcsDecoder<'static, JsonResource, usize> {
    /// Creates a decoder with no configured resource limits.
    pub fn unlimited() -> Self {
        Self::with_limits(JsonDecodeLimits::new())
    }
}

impl<'budget, R, Q> JcsDecoder<'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder around a reusable accounting session.
    pub const fn new(session: JsonDecodeSession<'budget, R, Q>) -> Self {
        Self {
            diagnostic_policy: DiagnosticPolicy::Redacted,
            engine: JsonDecodeEngine::new(session),
        }
    }

    /// Selects how much input-derived error detail is retained.
    pub const fn with_diagnostic_policy(mut self, policy: DiagnosticPolicy) -> Self {
        self.diagnostic_policy = policy;
        self
    }

    /// Returns the active diagnostic policy.
    pub const fn diagnostic_policy(&self) -> DiagnosticPolicy {
        self.diagnostic_policy
    }

    /// Returns the cumulative accounting session.
    pub const fn session(&self) -> &JsonDecodeSession<'budget, R, Q> {
        self.engine.session()
    }

    /// Returns mutable access to the cumulative accounting session.
    pub const fn session_mut(&mut self) -> &mut JsonDecodeSession<'budget, R, Q> {
        self.engine.session_mut()
    }

    /// Consumes this decoder and returns its accounting session.
    pub fn into_session(self) -> JsonDecodeSession<'budget, R, Q> {
        self.engine.into_session()
    }

    /// Decodes one complete JSON string into an owned value.
    pub fn decode_str(&mut self, input: &str) -> Result<serde_json::Value, JcsDecodeError<R, Q>> {
        self.decode_utf8(input.as_bytes())
    }

    /// Decodes one complete UTF-8 JSON document into an owned value.
    pub fn decode_utf8(&mut self, input: &[u8]) -> Result<serde_json::Value, JcsDecodeError<R, Q>> {
        let duplicate_key = Cell::new(false);
        let seed = JcsValueSeed {
            duplicate_key: &duplicate_key,
        };
        self.engine
            .decode_seed_utf8_with_number_model(seed, input, self.diagnostic_policy, JsonNumberModel::Binary64)
            .map_err(|error| map_jcs_error(error, duplicate_key.get()))
    }

    /// Reads a complete byte input through EOF and decodes it into an owned
    /// value.
    ///
    /// A finite input budget permits one uncharged probe byte to detect
    /// overflow; accepted input bytes remain charged when later decoding
    /// fails.
    pub fn decode_input<I>(&mut self, input: &mut I) -> Result<serde_json::Value, JcsDecodeError<R, Q>>
    where
        I: Input<Item = u8> + ?Sized,
    {
        let duplicate_key = Cell::new(false);
        let seed = JcsValueSeed {
            duplicate_key: &duplicate_key,
        };
        self.engine
            .decode_input_seed(seed, input, self.diagnostic_policy, JsonNumberModel::Binary64)
            .map_err(|error| map_jcs_error(error, duplicate_key.get()))
    }
}

fn map_jcs_error<R, Q>(error: JsonDecodeError<R, Q>, duplicate_key: bool) -> JcsDecodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    if !duplicate_key {
        return JcsDecodeError::Decode(error);
    }
    match error.into_source() {
        JsonDecodeErrorSource::Deserialize { line, column, .. } => JcsDecodeError::DuplicateKey { line, column },
        _ => unreachable!("duplicate-key marker requires a materialization error"),
    }
}
