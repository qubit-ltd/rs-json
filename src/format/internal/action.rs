// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
/// One prevalidated output operation that references source bytes or literals.
pub(in crate::format) enum Action {
    /// Copies the already validated half-open input byte range `[start, end)`.
    Span(usize, usize),
    /// Emits one literal byte.
    Byte(u8),
    /// Emits the requested number of ASCII spaces.
    Spaces(usize),
}
