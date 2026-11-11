// xpute-gpu/device.rs

//! These are the word offsets of the device's block (spec DEVICE). The runtime
//! writes the block where the core asks for it (`call::device`), and the core
//! reads it. The limits come first, one word for each name in LIMIT_NAME.

use crate::encode::load_acquire;
use crate::enums::{FeatureName, LimitName, TextureFormat, FEATURE_NAME, LIMIT_NAME};
use crate::spec::STATUS;

/// The features take one bit for each name in FEATURE_NAME.
pub const FEATS_AT: usize = LIMIT_NAME.len();
/// The timestamp period is in nanoseconds per tick, as f32 bits.
pub const PERIOD_AT: usize = FEATS_AT + FEATURE_NAME.len().div_ceil(32);
/// The surface format the runtime prefers is an index into TEXTURE_FORMAT.
pub const FORMAT_AT: usize = PERIOD_AT + 1;
/// This counts the uncaptured errors the device raised, and the runtime keeps
/// it up to date.
pub const ERRORS_AT: usize = FORMAT_AT + 1;
/// This counts the core's frames the GPU has not yet finished, and the runtime
/// keeps it up to date.
pub const UNFINISHED_AT: usize = ERRORS_AT + 1;
/// This is the device's STATUS, the block's last word. The runtime stores it
/// with release after the rest of the block, so a core that loads READY with
/// acquire sees the whole block.
pub const STATUS_AT: usize = UNFINISHED_AT + 1;
pub const WORDS: usize = STATUS_AT + 1;

/// The device as its block describes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Device {
    limits: [u32; LIMIT_NAME.len()],
    feats: [u32; FEATURE_NAME.len().div_ceil(32)],
    /// The timestamp period, in nanoseconds per tick.
    pub period_ns: f32,
    /// The surface format the runtime prefers.
    pub format: TextureFormat,
}

impl Device {
    /// Reads the device's `block`, once the runtime has stored its STATUS as
    /// READY, which this loads with acquire so the whole block is seen; none
    /// before then, and none where its format names no texture format.
    pub fn read(block: &[u32]) -> Option<Device> {
        if load_acquire(&block[STATUS_AT]) != STATUS::READY {
            return None;
        }
        let mut limits = [0; LIMIT_NAME.len()];
        limits.copy_from_slice(&block[..FEATS_AT]);
        let mut feats = [0; FEATURE_NAME.len().div_ceil(32)];
        feats.copy_from_slice(&block[FEATS_AT..PERIOD_AT]);
        Some(Device {
            limits,
            feats,
            period_ns: f32::from_bits(block[PERIOD_AT]),
            format: *TextureFormat::ALL.get(block[FORMAT_AT] as usize)?,
        })
    }

    pub fn limit(&self, name: LimitName) -> u32 {
        self.limits[name as usize]
    }

    pub fn has(&self, feat: FeatureName) -> bool {
        let k = feat as usize;
        self.feats[k / 32] >> (k % 32) & 1 != 0
    }
}
