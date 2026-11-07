// pxl-kernel/sys/load.rs

//! What the kernel loads, and how many: the catalog, the files it names,
//! each by its number below the asset root, and the Earth's sphere, each as
//! the catalog, a model or an image. A leaf: io loads them, and the turn's
//! edge sizes its tasks by them.

use crate::sys::io::file::Path;
use pxl_spec::asset::catalog::SLOTS;
use pxl_spec::bake::publication::TILE_SPHERE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf = 0,
    Image = 1,
    Catalog = 2,
}

/// A file below the asset root, by its id, the catalog's
/// (file.json `CATALOG`) or one it names; the Earth's picture whole is the
/// earth layer's, under its version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    File(u32),
    Sphere,
}

/// The sphere's id, past every one below the asset root, which a u16 holds.
const SPHERE: u32 = 1 << 16;

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

/// The files that can be in flight at once: a slot's each, which is wanted
/// as one kind, the catalog and the sphere.
pub const LOAD_FILES: usize = SLOTS + 2;

/// The models that can register at once, each in its own task: one a slot,
/// the most `model_of` starts (domain/scene/model/asset.rs).
pub const REGISTRATIONS: usize = SLOTS;
