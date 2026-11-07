// xpute-gpu/device.rs

//! The device's block (spec DEVICE) by word: where a runtime writes each part
//! of it and a core reads it. Its limits come first, a word each (LIMIT_NAME).

use crate::enums::{FEATURE_NAME, LIMIT_NAME};

/// Its features, a bit each (FEATURE_NAME).
pub const FEATURES_AT: usize = LIMIT_NAME.len();
/// Its timestamp period, f32 bits.
pub const PERIOD_AT: usize = FEATURES_AT + FEATURE_NAME.len().div_ceil(32);
/// The surface format it prefers (TEXTURE_FORMAT).
pub const FORMAT_AT: usize = PERIOD_AT + 1;
/// The frames unfinished.
pub const UNFINISHED_AT: usize = FORMAT_AT + 1;
/// Its STATUS, the block's last word.
pub const STATUS_AT: usize = UNFINISHED_AT + 1;
pub const WORDS: usize = STATUS_AT + 1;
