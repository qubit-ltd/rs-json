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
/// The decoder borrows its accounting session for `'budget`; `R` selects the
/// resource kind and `Q` is the quantity representation used by that session.
/// Calls reuse the same cumulative session, so limits apply across all inputs
/// decoded through this value.
///
/// # Type Parameters
///
/// * `R` is the resource kind tracked by the session.
/// * `Q` is the quantity type used to represent resource limits and usage.
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
    ///
    /// The limits are moved into a new accounting session and remain shared by
    /// every decode performed through the returned decoder.
    ///
    /// # Parameters
    ///
    /// * `limits` contains the resource ceilings for this decoder's session.
    ///
    /// # Returns
    ///
    /// A decoder that owns a session initialized from `limits`.
    #[must_use]
    #[inline]
    pub fn with_limits(limits: JsonDecodeLimits<R, Q>) -> Self {
        Self::new(JsonDecodeSession::from_limits(limits))
    }
}

impl JcsDecoder<'static, JsonResource, usize> {
    /// Creates a decoder with no configured resource limits.
    ///
    /// The returned decoder still accounts for decoded resources, but no
    /// configured ceiling rejects input based on resource usage.
    ///
    /// # Returns
    ///
    /// A decoder backed by a fresh unlimited accounting session.
    #[must_use]
    #[inline]
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
    ///
    /// The session is moved into the decoder and borrowed for `'budget`; its
    /// cumulative usage and configured limits are retained across decode calls.
    ///
    /// # Parameters
    ///
    /// * `session` is the accounting state to use for subsequent decodes.
    ///
    /// # Returns
    ///
    /// A decoder using the supplied session and redacted diagnostics.
    #[must_use]
    #[inline]
    pub const fn new(session: JsonDecodeSession<'budget, R, Q>) -> Self {
        Self {
            diagnostic_policy: DiagnosticPolicy::Redacted,
            engine: JsonDecodeEngine::new(session),
        }
    }

    /// Selects how much input-derived error detail is retained.
    ///
    /// Detailed diagnostics may retain input text and parser sources; use them
    /// only when those values are safe to expose. This setting affects errors
    /// from later decode calls.
    ///
    /// # Parameters
    ///
    /// * `policy` determines whether decode errors redact input-derived data.
    ///
    /// # Returns
    ///
    /// This decoder with the selected diagnostic policy.
    #[must_use]
    #[inline]
    pub const fn with_diagnostic_policy(mut self, policy: DiagnosticPolicy) -> Self {
        self.diagnostic_policy = policy;
        self
    }

    /// Returns the active diagnostic policy.
    ///
    /// # Returns
    ///
    /// The policy applied to errors from subsequent decode calls.
    #[must_use]
    #[inline]
    pub const fn diagnostic_policy(&self) -> DiagnosticPolicy {
        self.diagnostic_policy
    }

    /// Returns the cumulative accounting session.
    ///
    /// The borrow is tied to this decoder and permits inspecting limits and
    /// usage without transferring ownership of the session.
    ///
    /// # Returns
    ///
    /// A shared reference to the session used by this decoder.
    #[must_use]
    #[inline]
    pub const fn session(&self) -> &JsonDecodeSession<'budget, R, Q> {
        self.engine.session()
    }

    /// Returns mutable access to the cumulative accounting session.
    ///
    /// The borrow is tied to this decoder and allows callers to inspect or
    /// adjust session accounting before another decode.
    ///
    /// # Returns
    ///
    /// A mutable reference to the session used by this decoder.
    #[must_use]
    #[inline]
    pub const fn session_mut(&mut self) -> &mut JsonDecodeSession<'budget, R, Q> {
        self.engine.session_mut()
    }

    /// Consumes this decoder and returns its accounting session.
    ///
    /// This transfers the accumulated resource state to the caller so it can
    /// be inspected or reused by another decoder.
    ///
    /// # Returns
    ///
    /// The session previously owned by this decoder.
    #[must_use]
    #[inline]
    pub fn into_session(self) -> JsonDecodeSession<'budget, R, Q> {
        self.engine.into_session()
    }

    /// Decodes one complete JSON string into an owned value.
    ///
    /// The string is interpreted as strict JSON with JCS binary64 number
    /// semantics. Resource usage is charged to this decoder's cumulative
    /// session.
    ///
    /// # Parameters
    ///
    /// * `input` is the complete UTF-8 JSON document to decode.
    ///
    /// # Returns
    ///
    /// The owned JSON value on success.
    ///
    /// # Errors
    ///
    /// Returns `JcsDecodeError` when syntax, JCS number constraints, or the
    /// configured resource limits reject the document.
    #[inline]
    pub fn decode_str(&mut self, input: &str) -> Result<serde_json::Value, JcsDecodeError<R, Q>> {
        self.decode_utf8(input.as_bytes())
    }

    /// Decodes one complete UTF-8 JSON document into an owned value.
    ///
    /// The input is parsed with strict JSON and JCS binary64 number semantics;
    /// accepted resource usage is charged to the cumulative session.
    ///
    /// # Parameters
    ///
    /// * `input` is the complete UTF-8 JSON document to decode.
    ///
    /// # Returns
    ///
    /// The owned JSON value on success.
    ///
    /// # Errors
    ///
    /// Returns `JcsDecodeError` for invalid JSON, unsupported JCS numbers, or
    /// resource-limit violations.
    pub fn decode_utf8(&mut self, input: &[u8]) -> Result<serde_json::Value, JcsDecodeError<R, Q>> {
        self.engine.decode_jcs_utf8(input, self.diagnostic_policy)
    }

    /// Reads a complete byte input through EOF and decodes it into an owned
    /// value.
    ///
    /// A finite input budget permits one uncharged probe byte to detect
    /// overflow; accepted input bytes remain charged when later decoding
    /// fails.
    ///
    /// # Type Parameters
    ///
    /// * `I` is the byte input implementation; it may be unsized.
    ///
    /// # Parameters
    ///
    /// * `input` supplies the document bytes and is read through EOF.
    ///
    /// # Returns
    ///
    /// The owned JSON value on success.
    ///
    /// # Errors
    ///
    /// Returns `JcsDecodeError` for input read failures, invalid JSON, JCS
    /// number violations, or resource-limit violations.
    pub fn decode_input<I>(&mut self, input: &mut I) -> Result<serde_json::Value, JcsDecodeError<R, Q>>
    where
        I: Input<Item = u8> + ?Sized,
    {
        self.engine.decode_jcs_input(input, self.diagnostic_policy)
    }
}
