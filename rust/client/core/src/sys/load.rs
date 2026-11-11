// pxl-core/sys/load.rs

//! The files the core loads: the asset catalog, the files the catalog names
//! by their numbers after ASSET, and the picture of the whole Earth.
//! Each is read as a catalog, a glTF model or an image. sys/io/loader.rs
//! loads them with one task for each file in flight, and `LOAD_FILES` bounds
//! how many are in flight at once.

use pxl_spec::asset::catalog::SLOTS;

use xpute_core::io::fs::Token;

use crate::sys::io::file_spec::path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf = 0,
    Image = 1,
    Catalog = 2,
}

/// A file to load. `File` is a file after ASSET by its id: the
/// catalog itself (file.json `CATALOG`) or a file the catalog names.
/// `Sphere` is the picture of the whole Earth, which is opened after EARTH
/// and so under that bake's version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    File(u32),
    Sphere,
}

/// The sphere's id. Every file after ASSET has an id that fits in a u16, so
/// this one is past them all.
const SPHERE: u32 = 1 << 16;

/// The most files in flight at once: one for each catalog slot, since a slot
/// is wanted as one kind only, plus the catalog and the sphere.
pub const LOAD_FILES: usize = SLOTS + 2;

impl LoadKind {
    /// Every kind, each at the index of its number.
    pub const ALL: [LoadKind; 3] = [LoadKind::Gltf, LoadKind::Image, LoadKind::Catalog];
}

impl Load {
    /// The file's path: (ASSET, id), or the sphere's after EARTH.
    pub fn path(self) -> [Token<'static>; 2] {
        match self {
            Load::File(n) => [Token::U32(path::ASSET::ID), Token::U32(n)],
            Load::Sphere => [
                Token::U32(path::EARTH::ID),
                Token::U32(path::EARTH::SPHERE::ID),
            ],
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
