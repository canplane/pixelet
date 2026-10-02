// pxl-kernel/layer/geocode/repository.rs

//! The reverse-geocode bake's two packets, resident: the dictionary and one
//! group of pages, each a view over the heap block the shell fetched into, so
//! names are never decoded into strings. A group with no object answers 404,
//! meaning no page under it is in any place.

use pxl_spec::bake::place_page::{open_place_dict, open_place_group, PlaceGroupView, PlacesView, DICT_OBJECT};
use xpute_core::status::error::MarshalError;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{free, Block};

pub fn dict_name() -> &'static str {
    DICT_OBJECT
}

/// Held for the session: a group's numbers are places in it. The block goes
/// back when the packet drops, since the `'static` view cannot keep its own
/// lifetime.
pub struct ResidentDict {
    pub view: PlacesView<'static>,
    block: Block,
}

impl Drop for ResidentDict {
    fn drop(&mut self) {
        // SAFETY: the block `open_dict` took for this packet alone, freed once; the
        // view dies with it.
        unsafe { free(self.block) }
    }
}

pub struct ResidentGroup {
    pub view: PlaceGroupView<'static>,
    block: Block,
}

impl Drop for ResidentGroup {
    fn drop(&mut self) {
        // SAFETY: as `ResidentDict`'s.
        unsafe { free(self.block) }
    }
}

/// # Safety
/// The block is a heap block held for this packet alone, `bytes` long, until
/// the packet is released.
unsafe fn packet<'a>(at: usize, bytes: u32) -> &'a [u8] {
    // SAFETY: the caller's contract above.
    unsafe { core::slice::from_raw_parts(ptr_at::<u8>(at), bytes as usize) }
}

/// # Safety
/// As `packet`.
pub unsafe fn open_dict(at: usize, bytes: u32) -> Result<ResidentDict, MarshalError> {
    Ok(ResidentDict {
        // SAFETY: the caller's contract.
        view: open_place_dict(unsafe { packet(at, bytes) })?,
        block: (at, bytes),
    })
}

/// # Safety
/// As `packet`.
pub unsafe fn open_group(at: usize, bytes: u32) -> Result<ResidentGroup, MarshalError> {
    Ok(ResidentGroup {
        // SAFETY: the caller's contract.
        view: open_place_group(unsafe { packet(at, bytes) })?,
        block: (at, bytes),
    })
}
