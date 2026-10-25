// pxl-kernel/sys/load.rs

//! What the kernel loads, and how many: the shipped files, each by its
//! number below the asset root, the texture pack's among them, and the
//! Earth's sphere, each as a model, an image or both. A leaf: io loads
//! them, and the turn's edge sizes its tasks by them.

use pxl_spec::asset::ingested::Asset;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf = 0,
    Image = 1,
}

/// A file below the asset root, by its number, the texture pack's image
/// among them; the Earth's picture whole is the earth layer's, under its
/// version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    Asset(u32),
    Sphere,
}

/// The sphere's number, past every one below the asset root, which a u16
/// holds.
const SPHERE: u32 = 1 << 16;

impl Load {
    pub fn number(self) -> u32 {
        match self {
            Load::Asset(n) => n,
            Load::Sphere => SPHERE,
        }
    }

    pub fn of(number: u32) -> Option<Load> {
        match number {
            SPHERE => Some(Load::Sphere),
            n if n < SPHERE => Some(Load::Asset(n)),
            _ => None,
        }
    }
}

/// Every asset and the sphere.
pub const LOADS: usize = Asset::ALL.len() + 1;

/// The files that can be in flight at once: each load, as a model and as an
/// image.
pub const LOAD_FILES: usize = 2 * LOADS;
