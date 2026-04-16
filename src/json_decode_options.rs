/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Defines the option type used to configure the lenient JSON decoder.
//!
//! Author: Haixing Hu

/// Configuration switches for [`crate::LenientJsonDecoder`].
///
/// Each flag controls one normalization rule applied before parsing JSON.
/// Defaults are intentionally conservative and cover the most common
/// non-fully-trusted text inputs without attempting aggressive repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonDecodeOptions {
    /// Controls whether leading and trailing whitespace is removed before any
    /// other normalization step is applied.
    pub trim_whitespace: bool,
    /// Controls whether a leading UTF-8 byte order mark (`U+FEFF`) is removed
    /// before parsing.
    pub strip_utf8_bom: bool,
    /// Controls whether one outer Markdown code fence is removed.
    ///
    /// Typical examples include `````json ... ````` and bare fenced blocks
    /// starting with ````` followed by a newline.
    pub strip_markdown_code_fence: bool,
    /// Controls whether raw ASCII control characters inside JSON string
    /// literals are converted into valid JSON escape sequences.
    pub escape_control_chars_in_strings: bool,
    /// Caps the accepted raw input size in bytes before normalization.
    ///
    /// When set to `Some(limit)`, any input whose byte length is greater than
    /// `limit` is rejected before further processing. When set to `None`,
    /// no size limit is enforced.
    pub max_input_bytes: Option<usize>,
}

impl Default for JsonDecodeOptions {
    fn default() -> Self {
        Self {
            trim_whitespace: true,
            strip_utf8_bom: true,
            strip_markdown_code_fence: true,
            escape_control_chars_in_strings: true,
            max_input_bytes: None,
        }
    }
}
