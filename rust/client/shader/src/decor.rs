// pxl-shader/decor.rs

//! The decor compacted this frame as a reader beside its draws asks it (pxl-render
//! decor_compact.wgsl writes it): how many are placed and where each stands, so no reader names
//! its lanes (light.rs `light_mark_draws_cs`).

use pxl_content::generation::drawn;
use pxl_engine::{at, wrap_idx, LEAF_CELL};

/// The decor compacted this frame as its draws read it (pxl-render decor_compact.wgsl):
/// its drawing words, its regions and its instances. What a reader beside
/// the draws asks of it, how many and where each stands, decor answers
/// here, so none names its lanes (pxl-shader light.rs `light_mark_draws_cs`).
pub(crate) struct Instances<'a> {
    pub(crate) drawing: &'a [u32],
    pub(crate) regions: &'a [u32],
    pub(crate) instances: &'a [u32],
}

impl Instances<'_> {
    /// The slots the instances hold.
    fn cap(&self) -> u32 {
        self.instances.len() as u32 / (drawn::COLUMNS + 1)
    }

    /// How many are placed: the last region's end once placed, the regions
    /// laid in order from the first slot (pxl-render decor_args.wgsl), no more than the
    /// slots.
    pub(crate) fn cnt(&self) -> u32 {
        at(self.regions, 2 * drawn::REGIONS - 1).min(self.cap())
    }

    /// Where placed slot `k` stands, its base middle as drawn (pxl-render decor_compact.wgsl),
    /// units from the eye at Q3 `origin`.
    pub(crate) fn pos(&self, k: u32, origin: [i32; 3]) -> [f32; 3] {
        [
            self.anchor(0, origin[0]) + self.lane(0, k),
            self.anchor(1, origin[1]) + self.lane(1, k),
            self.anchor(2, origin[2]) + self.lane(2, k),
        ]
    }

    /// The anchor on `axis` from a Q3 `o`, in units.
    fn anchor(&self, axis: u32, o: i32) -> f32 {
        let w = pxl_engine::FRAC_W;
        let a = at(self.drawing, drawn::ANCHOR + axis) as i32;
        // The anchor and the eye in unit cells, which wrap round the world on X.
        let d = a.wrapping_sub(o >> w);
        let d = if axis == 0 { wrap_idx(LEAF_CELL, d) } else { d };
        d as f32 - (o & ((1 << w) - 1)) as f32 / (1 << w) as f32
    }

    /// Lane `l` of slot `k`, f32.
    fn lane(&self, l: u32, k: u32) -> f32 {
        f32::from_bits(at(self.instances, l * self.cap() + k))
    }
}
