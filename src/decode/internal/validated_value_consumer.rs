// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Lexical-event consumer that builds a validated safe JSON value.

use super::validated_consumer_error::ValidatedConsumerError;
use super::validated_container::ValidatedContainer;
use crate::decode::JsonDecodeContext;
use crate::decode::JsonNumberPolicy;
use crate::decode::JsonNumberToken;
use crate::decode::JsonPathSegment;
use crate::decode::JsonValidatedDecodeOptions;
use crate::decode::JsonValidatedLimit;
use crate::lexical::JsonLexicalConsumer;
use crate::lexical::JsonLexicalEvent;
use crate::lexical::JsonNumberModel;
use crate::value::SafeJsonValue;
use crate::value::internal::JsonValueMaterializationError;
use crate::value::internal::LexicalJsonValueBuilder;

/// Builds one validated value while tracking lexical traversal state.
///
/// The container stack records only unclosed containers. When path tracking is
/// disabled, callback paths are not built, while builder behavior, limits, and
/// container progression remain identical. Object key segments are pushed only
/// after key acceptance and are popped after the scalar or whole child
/// container ends. The builder protects partial materialization; this consumer
/// never commits the surrounding decode transaction.
pub(super) struct ValidatedValueConsumer<F, E> {
    /// Guarded builder for the value under construction.
    builder: LexicalJsonValueBuilder,
    /// Callback applied to each number before representation range checks.
    policy: F,
    /// Additional validated limits and number model.
    options: JsonValidatedDecodeOptions,
    /// Current callback path, maintained only when `track_path` is enabled.
    path: Vec<JsonPathSegment>,
    /// Whether to build callback paths; does not affect value or limit state.
    track_path: bool,
    /// Unclosed containers; array indices advance after each complete value.
    containers: Vec<ValidatedContainer>,
    /// Count of both values and object keys seen so far.
    nodes: usize,
    /// Policy consumed by the next matching number event; exact mode resets to
    /// preserve.
    next_number_policy: JsonNumberPolicy,
    /// Associates the policy error type with this consumer.
    marker: std::marker::PhantomData<E>,
}

impl<F, E> ValidatedValueConsumer<F, E> {
    /// Creates a consumer with the engine's validated decode configuration.
    pub(super) fn new(
        options: JsonValidatedDecodeOptions,
        policy: F,
        model: JsonNumberModel,
        track_path: bool,
    ) -> Self {
        Self {
            builder: LexicalJsonValueBuilder::new(model),
            policy,
            options,
            path: Vec::new(),
            track_path,
            containers: Vec::new(),
            nodes: 0,
            next_number_policy: JsonNumberPolicy::Preserve,
            marker: std::marker::PhantomData,
        }
    }

    /// Finishes guarded materialization and returns the completed safe value.
    ///
    /// The engine remains responsible for committing or rolling back its
    /// surrounding transaction.
    pub(super) fn finish(self) -> Result<SafeJsonValue, JsonValueMaterializationError> {
        self.builder.finish_guarded()
    }

    /// Checks raw encoded string content bytes, excluding surrounding quotes.
    fn check_encoded_limit(&self, token_len: usize, offset: usize) -> Result<(), ValidatedConsumerError<E>> {
        let Some(maximum) = self.options.max_encoded_string_bytes() else {
            return Ok(());
        };
        let observed = token_len.saturating_sub(2);
        if observed > maximum {
            return Err(ValidatedConsumerError::Limit {
                limit: JsonValidatedLimit::EncodedStringBytes,
                maximum,
                observed,
                offset,
            });
        }
        Ok(())
    }

    /// Completes one scalar or container and advances/pops its parent path
    /// state.
    fn complete_value(&mut self) {
        if let Some(parent) = self.containers.last_mut() {
            match parent {
                ValidatedContainer::Array { next_index } => {
                    if self.track_path {
                        self.path.pop();
                    }
                    *next_index = next_index.saturating_add(1);
                }
                ValidatedContainer::Object => {
                    if self.track_path {
                        self.path.pop();
                    }
                }
            }
        }
    }
}

impl<'input, F, E> JsonLexicalConsumer<'input> for ValidatedValueConsumer<F, E>
where
    F: for<'call> FnMut(&'call [JsonPathSegment], JsonNumberToken<'call>) -> Result<JsonNumberPolicy, E>,
{
    type Error = ValidatedConsumerError<E>;

    /// Counts a value and prepares its array path before consuming its event.
    fn before_value(&mut self, _depth: usize, offset: usize) -> Result<(), Self::Error> {
        if self.track_path
            && let Some(ValidatedContainer::Array { next_index }) = self.containers.last()
        {
            self.path.push(JsonPathSegment::Index(*next_index));
        }
        self.nodes = self.nodes.saturating_add(1);
        if let Some(maximum) = self.options.max_value_and_key_nodes()
            && self.nodes > maximum
        {
            return Err(ValidatedConsumerError::Limit {
                limit: JsonValidatedLimit::ValueAndKeyNodes,
                maximum,
                observed: self.nodes,
                offset,
            });
        }
        Ok(())
    }

    /// Counts an object key toward the shared value-and-key node limit.
    fn before_key(&mut self, offset: usize) -> Result<(), Self::Error> {
        self.nodes = self.nodes.saturating_add(1);
        if let Some(maximum) = self.options.max_value_and_key_nodes()
            && self.nodes > maximum
        {
            return Err(ValidatedConsumerError::Limit {
                limit: JsonValidatedLimit::ValueAndKeyNodes,
                maximum,
                observed: self.nodes,
                offset,
            });
        }
        Ok(())
    }

    /// Runs policy before the following number event performs representation
    /// checks.
    fn before_number(&mut self, token: &'input str, offset: usize) -> Result<(), Self::Error> {
        self.next_number_policy = (self.policy)(&self.path, JsonNumberToken::from_validated(token, offset))
            .map_err(|source| ValidatedConsumerError::Policy { source, offset })?;
        Ok(())
    }

    /// Applies one scanner event, completing path state only after its whole
    /// value.
    fn event(&mut self, event: JsonLexicalEvent<'input>) -> Result<(), Self::Error> {
        match event {
            JsonLexicalEvent::Number { token, offset }
                if self.next_number_policy == JsonNumberPolicy::ExactSafeInteger =>
            {
                let text = std::str::from_utf8(token).expect("scanner number token is UTF-8");
                let value = JsonNumberToken::from_validated(text, offset)
                    .exact_safe_integer()
                    .map_err(|source| ValidatedConsumerError::Exact { source, offset })?;
                self.builder.try_accept_number(value.into(), offset).map_err(|source| {
                    ValidatedConsumerError::Materialization {
                        source,
                        context: JsonDecodeContext::Number,
                    }
                })?;
                self.next_number_policy = JsonNumberPolicy::Preserve;
                self.complete_value();
            }
            JsonLexicalEvent::Key { token, offset } => {
                self.check_encoded_limit(token.len(), offset)?;
                let key = serde_json::from_slice::<String>(token).expect("scanner admitted valid key token");
                let path_key = self.track_path.then(|| key.clone());
                self.builder.try_accept_decoded_key(key, offset).map_err(|source| {
                    ValidatedConsumerError::Materialization {
                        source,
                        context: JsonDecodeContext::ObjectKey,
                    }
                })?;
                if let Some(key) = path_key {
                    self.path.push(JsonPathSegment::Key(key));
                }
            }
            JsonLexicalEvent::String { token, offset } => {
                self.check_encoded_limit(token.len(), offset)?;
                self.builder
                    .try_accept(JsonLexicalEvent::String { token, offset })
                    .map_err(|source| ValidatedConsumerError::Materialization {
                        source,
                        context: JsonDecodeContext::String,
                    })?;
                self.complete_value();
            }
            JsonLexicalEvent::BeginArray { .. } => {
                self.builder
                    .try_accept(event)
                    .map_err(|source| ValidatedConsumerError::Materialization {
                        source,
                        context: JsonDecodeContext::Value,
                    })?;
                self.containers.push(ValidatedContainer::Array { next_index: 0 });
            }
            JsonLexicalEvent::BeginObject { .. } => {
                self.builder
                    .try_accept(event)
                    .map_err(|source| ValidatedConsumerError::Materialization {
                        source,
                        context: JsonDecodeContext::Value,
                    })?;
                self.containers.push(ValidatedContainer::Object);
            }
            JsonLexicalEvent::EndArray { .. } | JsonLexicalEvent::EndObject { .. } => {
                self.builder
                    .try_accept(event)
                    .map_err(|source| ValidatedConsumerError::Materialization {
                        source,
                        context: JsonDecodeContext::Value,
                    })?;
                self.containers.pop();
                self.complete_value();
            }
            other => {
                self.builder
                    .try_accept(other)
                    .map_err(|source| ValidatedConsumerError::Materialization {
                        source,
                        context: JsonDecodeContext::Value,
                    })?;
                self.complete_value();
            }
        }
        Ok(())
    }
}
