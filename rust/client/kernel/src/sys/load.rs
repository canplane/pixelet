// pxl-kernel/sys/load.rs

//! What the kernel loads, and how many: the catalog, the files it names,
//! each by its number below the asset root, and the Earth's sphere, each as
//! the catalog, a model or an image. A leaf: io loads them, and the turn's
//! edge sizes its tasks by them.

use pxl_spec::asset::catalog::SLOTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf = 0,
    Image = 1,
    Catalog = 2,
}

/// A file below the asset root, by its number, the catalog's
/// (host.json `CATALOG`) or one it names; the Earth's picture whole is the
/// earth layer's, under its version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    File(u32),
    Sphere,
}

/// The sphere's number, past every one below the asset root, which a u16
/// holds.
const SPHERE: u32 = 1 << 16;

impl Load {
    pub fn number(self) -> u32 {
        match self {
            Load::File(n) => n,
            Load::Sphere => SPHERE,
        }
    }

    pub fn of(number: u32) -> Option<Load> {
        match number {
            SPHERE => Some(Load::Sphere),
            n if n < SPHERE => Some(Load::File(n)),
            _ => None,
        }
    }
}

/// The files that can be in flight at once: a slot's each, which is wanted
/// as one kind, the catalog and the sphere.
pub const LOAD_FILES: usize = SLOTS + 2;
