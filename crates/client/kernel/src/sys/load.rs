// pxl-kernel/sys/load.rs

//! What the kernel loads, and how many: every ingested asset, the texture
//! pack and the Earth's sphere, each as a model, an image or both. A leaf:
//! io loads them, and the turn's edge sizes its tasks by them.

use pxl_spec::asset::ingested::Asset;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf = 0,
    Image = 1,
}

/// The texture pack's image is written beside the asset table, not as a row of
/// it; the Earth's picture whole is the earth layer's, under its version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    Asset(Asset),
    Pack,
    Sphere,
}

impl Load {
    pub fn number(self) -> u32 {
        match self {
            Load::Asset(a) => a as u32,
            Load::Pack => Asset::ALL.len() as u32,
            Load::Sphere => Asset::ALL.len() as u32 + 1,
        }
    }

    pub fn of(number: u32) -> Option<Load> {
        match number.checked_sub(Asset::ALL.len() as u32) {
            Some(0) => Some(Load::Pack),
            Some(1) => Some(Load::Sphere),
            Some(_) => None,
            None => u16::try_from(number).ok().and_then(Asset::of).map(Load::Asset),
        }
    }
}

/// Every asset, the pack and the sphere.
pub const LOADS: usize = Asset::ALL.len() + 2;

/// The files that can be in flight at once: each load, as a model and as an
/// image.
pub const LOAD_FILES: usize = 2 * LOADS;
