// pxl-spec/asset/catalog.rs

//! The catalog: the one file the client plugs in, through which the core finds every other
//! file (pxl-core `sys/io/file.json5` `CATALOG`). Which assets exist, and each file reference a
//! definition makes, are compiled in as the world's rule (`spec/spec/asset/`): a reference is
//! a `Slot`. Which file fills each slot, and what the ingest measured of it, are the catalog's.
//!
//! It is laid out little-endian, as pxl-tool `spec/catalog.rs` writes it: the slots' count as a
//! u32; each slot's file as a u32, its number below the asset root; then each slot's extent in
//! ENU as three f64, zero for an image.

#[path = "catalog.spec.rs"]
pub mod spec;
pub use spec::*;

/// A file reference a definition makes, numbered in the asset specs' order
/// (pxl-tool `spec/catalog.rs` numbers them).
pub type Slot = u16;

pub struct Catalog {
    files: Vec<u32>,
    extents: Vec<[f64; 3]>,
}

impl Catalog {
    /// None for bytes laid out otherwise, or filling another count of slots
    /// than the one compiled.
    pub fn read(bytes: &[u8]) -> Option<Catalog> {
        let word = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
        let real = |at: usize| Some(f64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?));
        if word(0)? as usize != SLOT_CNT || bytes.len() != 4 + SLOT_CNT * (4 + 3 * 8) {
            return None;
        }
        let files = (0..SLOT_CNT)
            .map(|s| word(4 + 4 * s))
            .collect::<Option<Vec<_>>>()?;
        let at = 4 + 4 * SLOT_CNT;
        let extents = (0..SLOT_CNT)
            .map(|s| {
                Some([
                    real(at + 24 * s)?,
                    real(at + 24 * s + 8)?,
                    real(at + 24 * s + 16)?,
                ])
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Catalog { files, extents })
    }

    /// The file `slot` names, by its number below the asset root.
    pub fn file(&self, slot: Slot) -> u32 {
        self.files[usize::from(slot)]
    }

    /// What the ingest measured of the file `slot` names, in ENU units;
    /// zero for an image.
    pub fn extent(&self, slot: Slot) -> [f64; 3] {
        self.extents[usize::from(slot)]
    }
}

/// The extent of the file `slot` names, zero before a catalog is read.
pub fn model_extent(catalog: Option<&Catalog>, slot: Slot) -> [f64; 3] {
    catalog.map_or([0.0; 3], |c| c.extent(slot))
}

#[cfg(test)]
#[path = "catalog.test.rs"]
mod test;
