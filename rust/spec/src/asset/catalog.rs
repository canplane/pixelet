// pxl-spec/asset/catalog.rs

//! The catalog: the one file the host plugs in, through which the kernel
//! finds every other (pxl-kernel sys/io/host.json `CATALOG`). Which assets
//! exist, and each file reference a definition makes, is compiled, the
//! world's rule (`spec/spec/asset/`): a reference is a `Slot`. Which file
//! fills each slot, and what the ingest measured of it, is the catalog's.
//!
//! Laid out little-endian, as pxl-tool spec/catalog.rs writes it: the
//! slots' count as a u32, each slot's file as a u32, its number below the
//! asset root, then each slot's extent, ENU, as three f64, zero for an image.

#![expect(
    non_upper_case_globals,
    reason = "the generated slots are named as the assets they hold (`pack`)"
)]

#[path = "catalog.spec.rs"]
mod spec;
pub use spec::*;

/// A file reference a definition makes, numbered in the asset specs' order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(pub u16);

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
        if word(0)? as usize != SLOTS || bytes.len() != 4 + SLOTS * (4 + 3 * 8) {
            return None;
        }
        let files = (0..SLOTS)
            .map(|s| word(4 + 4 * s))
            .collect::<Option<Vec<_>>>()?;
        let at = 4 + 4 * SLOTS;
        let extents = (0..SLOTS)
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
        self.files[usize::from(slot.0)]
    }

    /// What the ingest measured of the file `slot` names, in ENU units;
    /// zero for an image.
    pub fn extent(&self, slot: Slot) -> [f64; 3] {
        self.extents[usize::from(slot.0)]
    }
}

#[cfg(test)]
#[path = "catalog.test.rs"]
mod test;
