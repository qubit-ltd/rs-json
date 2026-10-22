// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stateful strict JSON decoding with JCS binary64 number semantics.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_io::Input;

use super::DiagnosticPolicy;
use super::JcsDecodeError;
use super::internal::JsonDecodeEngine;

/// Decodes strict JSON text into an owned value using finite binary64 numbers.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::JcsDecoder;
///
/// let value = JcsDecoder::unlimited().decode_str("1.5").unwrap();
/// assert_eq!(value.as_f64(), Some(1.5));
/// ```
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
        self.engine.decode_jcs_utf8(input, self.diagnostic_policy)
    }

    /// Reads a complete byte input through EOF and decodes it into an owned
    /// value.
    ///
    /// A finite input budget permits one uncharged probe byte to detect
    /// overflow; accepted input bytes remain charged when later decoding
    /// fails.
    pub fn decode_input<I>(
        &mut self,
        input: &mut I,
    ) -> Result<serde_json::Value, JcsDecodeError<R, Q>>
    where
        I: Input<Item = u8> + ?Sized,
    {
        self.engine.decode_jcs_input(input, self.diagnostic_policy)
    }
}
