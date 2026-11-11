// xpute-gpu/device.rs

//! This reads the device's block, laid out by spec/gpu/api.json's DEVICE
//! (`spec::DEVICE`). The runtime writes the block where the core asks for it
//! (`call::device`), and the core reads it. The limits come first, one word for
//! each name in LIMIT_NAME; the features take one bit for each name in
//! FEATURE_NAME; the timestamp period is in nanoseconds per tick, as f32 bits;
//! the surface format the runtime prefers is an index into TEXTURE_FORMAT; the
//! runtime keeps the counts of uncaptured errors and of the core's unfinished
//! frames up to date; and STATUS, the last, the runtime stores with release
//! after the rest of the block, so a core that loads READY with acquire sees
//! the whole block.

use crate::encode::load_acquire;
use crate::enums::{FeatureName, LimitName, TextureFormat};
use crate::spec::{DEVICE, STATUS};

/// The device as its block describes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Device {
    limits: [u32; DEVICE::FEATS_AT],
    feats: [u32; DEVICE::PERIOD_AT - DEVICE::FEATS_AT],
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
        if load_acquire(&block[DEVICE::STATUS_AT]) != STATUS::READY {
            return None;
        }
        let mut limits = [0; DEVICE::FEATS_AT];
        limits.copy_from_slice(&block[..DEVICE::FEATS_AT]);
        let mut feats = [0; DEVICE::PERIOD_AT - DEVICE::FEATS_AT];
        feats.copy_from_slice(&block[DEVICE::FEATS_AT..DEVICE::PERIOD_AT]);
        Some(Device {
            limits,
            feats,
            period_ns: f32::from_bits(block[DEVICE::PERIOD_AT]),
            format: *TextureFormat::ALL.get(block[DEVICE::FORMAT_AT] as usize)?,
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
