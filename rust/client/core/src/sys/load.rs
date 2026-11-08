// pxl-core/sys/load.rs

//! What the core loads, and how many: the catalog, the files it names, each
//! by its number below the asset root, and the Earth's sphere, each read as
//! the catalog, a model or an image. sys/io loads them, a task a file in
//! flight (`LOAD_FILES`).

use pxl_spec::asset::catalog::SLOTS;
use pxl_spec::bake::publication::TILE_SPHERE;

use crate::sys::io::file::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf = 0,
    Image = 1,
    Catalog = 2,
}

/// A file below the asset root by its id, the catalog's (file.json
/// `CATALOG`) or one it names; or the Earth's picture whole, the earth
/// layer's, under its version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    File(u32),
    Sphere,
}

/// The sphere's id, past every one below the asset root, which a u16 holds.
const SPHERE: u32 = 1 << 16;

/// The files that can be in flight at once: a slot's each, wanted as one
/// kind, the catalog and the sphere.
pub const LOAD_FILES: usize = SLOTS + 2;

impl LoadKind {
    /// By number.
    pub const ALL: [LoadKind; 3] = [LoadKind::Gltf, LoadKind::Image, LoadKind::Catalog];
}

impl Load {
    /// Its path below its root.
    pub fn path(self) -> Path {
        match self {
            Load::File(n) => Path::of_id(u64::from(n)),
            Load::Sphere => Path::of_name(TILE_SPHERE),
        }
    }

    pub fn id(self) -> u32 {
        match self {
            Load::File(n) => n,
            Load::Sphere => SPHERE,
        }
    }

    pub fn of(id: u32) -> Option<Load> {
        match id {
            SPHERE => Some(Load::Sphere),
            n if n < SPHERE => Some(Load::File(n)),
            _ => None,
        }
    }
}
