// xpute-gpu/device.rs

//! These are the word offsets of the device's block (spec DEVICE). The runtime
//! writes the block where the core asks for it (`call::device`), and the core
//! reads it. The limits come first, one word for each name in LIMIT_NAME.

use crate::enums::{FEATURE_NAME, LIMIT_NAME};

/// The features take one bit for each name in FEATURE_NAME.
pub const FEATURES_AT: usize = LIMIT_NAME.len();
/// The timestamp period is in nanoseconds per tick, as f32 bits.
pub const PERIOD_AT: usize = FEATURES_AT + FEATURE_NAME.len().div_ceil(32);
/// The surface format the runtime prefers is an index into TEXTURE_FORMAT.
pub const FORMAT_AT: usize = PERIOD_AT + 1;
/// This counts the core's frames the GPU has not yet finished, and the runtime
/// keeps it up to date.
pub const UNFINISHED_AT: usize = FORMAT_AT + 1;
/// This is the device's STATUS, the block's last word. The runtime stores it
/// with release after the rest of the block, so a core that loads READY with
/// acquire sees the whole block.
pub const STATUS_AT: usize = UNFINISHED_AT + 1;
pub const WORDS: usize = STATUS_AT + 1;
