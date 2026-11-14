// xpute-gpu-guest/device.rs

//! This reads the device's block, laid out by gpu/link/spec/api.json's DEVICE
//! (`spec::device`). The host writes the block the guest lends it
//! (gpu/wit/gpu.wit `device.lost`), and the guest reads it. The limits come first, one word
//! for each name in LIMIT_NAME; the features take one bit for each name in
//! FEATURE_NAME; the timestamp period is in nanoseconds per tick, as f32 bits;
//! the surface format the host prefers is an index into TEXTURE_FORMAT; the
//! host keeps the counts of uncaptured errors and of the guest's unfinished
//! frames up to date; and STATUS, the last, the host stores with release
//! after the rest of the block, so a guest that loads READY with acquire sees
//! the whole block.

use crate::encode::load_acquire;
use xpute_gpu_link::enums::{FeatureName, LimitName, TextureFormat};
use xpute_gpu_link::spec::{device, status};

/// The device as its block describes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Device {
    limits: [u32; device::FEATS_AT],
    feats: [u32; device::PERIOD_AT - device::FEATS_AT],
    /// The timestamp period, in nanoseconds per tick.
    pub period_ns: f32,
    /// The surface format the host prefers.
    pub format: TextureFormat,
}

impl Device {
    /// Reads the device's block, whose first word is `block`, once the host
    /// has stored its STATUS as READY, which this loads with acquire so the
    /// whole block is seen; none before then, and none where its format names
    /// no texture format. The host writes the limits, the features, the period
    /// and the format before READY and none of them after.
    ///
    /// # Safety
    /// `block` is the first of `device::WORDS` live, aligned words, whose
    /// STATUS the host stores into only atomically.
    pub unsafe fn read(block: *const u32) -> Option<Device> {
        // SAFETY: the caller's; a word before STATUS is read only once STATUS
        // is READY, after which the host writes none of those read.
        let word = |k: usize| unsafe { block.add(k).read() };
        // SAFETY: the caller's.
        if unsafe { load_acquire(block.add(device::STATUS_AT)) } != status::READY {
            return None;
        }
        Some(Device {
            limits: core::array::from_fn(word),
            feats: core::array::from_fn(|k| word(device::FEATS_AT + k)),
            period_ns: f32::from_bits(word(device::PERIOD_AT)),
            format: *TextureFormat::ALL.get(word(device::FORMAT_AT) as usize)?,
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
