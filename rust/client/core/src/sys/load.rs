// pxl-core/sys/load.rs

//! The files the core loads: the asset catalog, the files the catalog names
//! by their numbers after ASSET, and the picture of the whole Earth.
//! Each is read as a catalog, a glTF model or an image. sys/io/loader.rs
//! loads them with one task for each file in flight, and `LOAD_FILE_CAP` bounds
//! how many are in flight at once.

use pxl_spec::asset::catalog::SLOT_CNT;

use xpute_link::abi::types::Token;

use crate::spec::file::path;

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
pub const LOAD_FILE_CAP: usize = SLOT_CNT + 2;

impl LoadKind {
    /// Every kind, each at the index of its number.
    pub const ALL: [LoadKind; 3] = [LoadKind::Gltf, LoadKind::Image, LoadKind::Catalog];
}

impl Load {
    /// The file's path: (ASSET, id), or (EARTH, SPHERE). A file's id is an i32 as it
    /// is.
    pub fn path(self) -> Vec<Token> {
        match self {
            Load::File(n) => vec![Token::Int(path::asset::ID), Token::Int(n as i32)],
            Load::Sphere => vec![
                Token::Int(path::earth::ID),
                Token::Int(path::earth::sphere::ID),
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

/// A want's id, with the kind in the high word, because one file can be
/// wanted as more than one kind.
pub fn id_of(kind: LoadKind, load: Load) -> u64 {
    (kind as u64) << 32 | u64::from(load.id())
}

pub fn of_id(id: u64) -> Option<(LoadKind, Load)> {
    let kind = LoadKind::ALL.into_iter().find(|&k| k as u64 == id >> 32)?;
    Some((kind, Load::of(id as u32)?))
}
