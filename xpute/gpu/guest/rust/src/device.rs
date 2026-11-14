// xpute-gpu-guest/device.rs

//! This reads the device's block, laid out by gpu/link/spec/api.json's DEVICE
//! (`spec::device`). The host writes the block where the guest asks for it
//! (`call::device`), and the guest reads it. The limits come first, one word
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
    /// Reads the device's `block`, once the host has stored its STATUS as
    /// READY, which this loads with acquire so the whole block is seen; none
    /// before then, and none where its format names no texture format.
    pub fn read(block: &[u32]) -> Option<Device> {
        if load_acquire(&block[device::STATUS_AT]) != status::READY {
            return None;
        }
        let mut limits = [0; device::FEATS_AT];
        limits.copy_from_slice(&block[..device::FEATS_AT]);
        let mut feats = [0; device::PERIOD_AT - device::FEATS_AT];
        feats.copy_from_slice(&block[device::FEATS_AT..device::PERIOD_AT]);
        Some(Device {
            limits,
            feats,
            period_ns: f32::from_bits(block[device::PERIOD_AT]),
            format: *TextureFormat::ALL.get(block[device::FORMAT_AT] as usize)?,
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
