// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

/// Selects the point at which a lexical token is admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum JsonLexicalAdmissionPolicy {
    /// Preserves the established delimiter-first validation order.
    #[default]
    Immediate,
    /// Validates a complete token before checking what follows it.
    TokenFirst,
}
