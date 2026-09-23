// pxl-kernel/layer/geocode/repository.rs

//! The two packets the reverse-geocode bake writes, resident: the dictionary
//! and one group of pages (pxl-spec bake/place_page.rs). Each is the heap
//! block the shell fetched the body into and a view opened over it, so a
//! name reaches the snapshot without being decoded into a string of its own.
//!
//! Where they are fetched from: the pre-baked static set when one is
//! configured, the dev tile server otherwise. A group the bake wrote no
//! object for answers 404, which is "no page under it is in any place"
//! rather than a failure.

use pxl_spec::bake::place_page::{open_place_dict, open_place_group, PlaceGroupView, PlacesView, DICT_OBJECT};
use pxl_spec::space::pvf3::ivec::IVec;
use xpute_core::status::error::MarshalError;
use xpute_runtime::mem::heap::{free, Block};

/// The dictionary, which the set has one of at the name the bake gives it.
pub fn dict_name() -> &'static str {
    DICT_OBJECT
}

/// A group's name in its bake: the group's own key.
pub fn group_name(group_idx: IVec) -> IVec {
    group_idx
}

/// Every place a polygon names, held for the session: a group's numbers are
/// places in it, so it outlives every group.
///
/// The block goes back when the packet does. A `'static` view over a block
/// is the shape a resident page has here (layer/poi/repository.rs) and the
/// lifetime is a claim the type cannot keep — what it is really good for is
/// the block's own life, which is why giving the block back is the packet's
/// own business and not a caller's to remember.
pub struct ResidentDict {
    pub view: PlacesView<'static>,
    block: Block,
}

impl Drop for ResidentDict {
    fn drop(&mut self) {
        // SAFETY: the block `open_dict` took for this packet alone, given
        // back once, and the view that read it dies with it.
        unsafe { free(self.block) }
    }
}

/// One group of pages, held while the centre is in it.
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

/// The bytes of a body the host copied into a block of its own.
///
/// # Safety
/// The block is a heap block held for this packet alone, `bytes` long, until
/// the packet is released.
unsafe fn packet<'a>(at: usize, bytes: u32) -> &'a [u8] {
    // SAFETY: the caller's contract above.
    unsafe { core::slice::from_raw_parts(at as *const u8, bytes as usize) }
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
