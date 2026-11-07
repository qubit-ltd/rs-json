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
    Span(
        /// Inclusive start offset into the original input.
        usize,
        /// Exclusive end offset into the original input.
        usize,
    ),
    /// Emits one literal byte.
    Byte(
        /// The byte to append to the formatted output.
        u8,
    ),
    /// Emits the requested number of ASCII spaces.
    Spaces(
        /// Number of ASCII space bytes to append.
        usize,
    ),
}
