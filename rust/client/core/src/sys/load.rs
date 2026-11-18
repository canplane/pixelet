// pxl-core/sys/load.rs

//! The files the core loads: the asset catalog, the files the catalog names
//! by their numbers after ASSET, and the imagery's global image.
//! Each is read as a catalog, a glTF model or an image. sys/io/loader.rs
//! loads them with one task for each file in flight, and `LOAD_FILE_CAP` bounds
//! how many are in flight at once.

use pxl_spec::asset::catalog::SLOT_CNT;

use xpute_link::abi::types::Token;

use crate::spec::file::path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoadKind {
    Gltf = 0,
    Image = 1,
    Catalog = 2,
}

/// A file to load. `File` is a file after ASSET by its id: the catalog itself
/// (file.json5 `CATALOG`) or a file the catalog names. `Global` is the
/// imagery's global image, which is opened after IMAGERY and so under that
/// bake's version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Load {
    File(u32),
    Global,
}

/// The global image's id. Every file after ASSET has an id that fits in a
/// u16, so this one is past them all.
const GLOBAL: u32 = 1 << 16;

/// The most files in flight at once: one for each catalog slot, since a slot
/// is wanted as one kind only, plus the catalog and the global image.
pub(crate) const LOAD_FILE_CAP: usize = SLOT_CNT + 2;

impl LoadKind {
    /// Every kind, each at the index of its number.
    pub(crate) const ALL: [LoadKind; 3] = [LoadKind::Gltf, LoadKind::Image, LoadKind::Catalog];
}

impl Load {
    /// The file's path: (ASSET, id), or (IMAGERY, GLOBAL). A file's id is an
    /// i32 as it is.
    pub(crate) fn path(self) -> Vec<Token> {
        match self {
            Load::File(n) => vec![Token::Int(path::asset::ID), Token::Int(n as i32)],
            Load::Global => vec![
                Token::Int(path::imagery::ID),
                Token::Int(path::imagery::global::ID),
            ],
        }
    }

    pub(crate) fn id(self) -> u32 {
        match self {
            Load::File(n) => n,
            Load::Global => GLOBAL,
        }
    }

    pub(crate) fn of(id: u32) -> Option<Load> {
        match id {
            GLOBAL => Some(Load::Global),
            n if n < GLOBAL => Some(Load::File(n)),
            _ => None,
        }
    }
}

/// A want's id, with the kind in the high word, because one file can be
/// wanted as more than one kind.
pub(crate) fn id_of(kind: LoadKind, load: Load) -> u64 {
    (kind as u64) << 32 | u64::from(load.id())
}

pub(crate) fn of_id(id: u64) -> Option<(LoadKind, Load)> {
    let kind = LoadKind::ALL.into_iter().find(|&k| k as u64 == id >> 32)?;
    Some((kind, Load::of(id as u32)?))
}
