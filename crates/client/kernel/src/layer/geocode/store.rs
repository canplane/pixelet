// pxl-kernel/layer/geocode/store.rs

//! The reverse-geocode bake's two packets, resident: the dictionary, and the
//! one group the map center is in. A group spans sixteen tiles a side, so
//! holding more buys little. Each is a view over the heap block the shell
//! fetched into, so names are never decoded into strings.
//!
//! A 404 is an answer: the bake wrote no object, so no tile under it is in
//! any place, and only moving to another group asks again. A failed fetch is
//! not one; its want waits out the request scheduler's retry.

use pxl_spec::bake::place_tile::{open_place_dict, open_place_group, PlaceGroupView, PlacesView};
use pxl_spec::space::coord::pvf3::ivec::IVec;
use xpute_core::status::error::MarshalError;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{free, Block};

use xpute_runtime::global::Global;

/// The block goes back when the packet drops, since the `'static` view cannot
/// keep its own lifetime.
pub struct Resident<V> {
    pub view: V,
    block: Block,
}

impl<V> Drop for Resident<V> {
    fn drop(&mut self) {
        // SAFETY: the block `open` took for this packet alone, freed once; the
        // view dies with it.
        unsafe { free(self.block) }
    }
}

/// Takes the block: it is freed with the packet, or at once if it does not open.
///
/// # Safety
/// The block is a heap block held for this packet alone, until it is freed.
unsafe fn open<V>(block: Block, view: impl FnOnce(&'static [u8]) -> Result<V, MarshalError>) -> Result<Resident<V>, MarshalError> {
    // SAFETY: the caller's contract.
    let pkt = unsafe { core::slice::from_raw_parts(ptr_at::<u8>(block.0), block.1 as usize) };
    match view(pkt) {
        Ok(view) => Ok(Resident { view, block }),
        Err(err) => {
            // SAFETY: as above, and nothing views it now.
            unsafe { free(block) };
            Err(err)
        }
    }
}

#[derive(Default)]
struct GeocodeStore {
    /// None inside where the bake wrote no dictionary.
    dict: Option<Option<Resident<PlacesView<'static>>>>,
    /// None inside where the bake wrote no object.
    group: Option<(IVec, Option<Resident<PlaceGroupView<'static>>>)>,
}

static STORE: Global<GeocodeStore> = Global::new();

fn store() -> &'static mut GeocodeStore {
    STORE.get(GeocodeStore::default)
}

pub fn dict() -> Option<&'static Resident<PlacesView<'static>>> {
    store().dict.as_ref()?.as_ref()
}

pub fn dict_answered() -> bool {
    store().dict.is_some()
}

pub fn answered() -> Option<IVec> {
    store().group.as_ref().map(|(idx, _)| *idx)
}

/// Only where it is the one held and has an object.
pub fn group_of(group_idx: IVec) -> Option<&'static Resident<PlaceGroupView<'static>>> {
    match &store().group {
        Some((idx, held)) if *idx == group_idx => held.as_ref(),
        _ => None,
    }
}

/// # Safety
/// As `open`.
pub unsafe fn adopt_dict(block: Block) -> Result<(), MarshalError> {
    // SAFETY: the caller's contract. Dropping what was held gives its block back.
    store().dict = Some(Some(unsafe { open(block, open_place_dict) }?));
    Ok(())
}

/// # Safety
/// As `open`.
pub unsafe fn adopt_group(group_idx: IVec, block: Block) -> Result<(), MarshalError> {
    // SAFETY: the caller's contract.
    store().group = Some((group_idx, Some(unsafe { open(block, open_place_group) }?)));
    Ok(())
}

pub fn absent_dict() {
    store().dict = Some(None);
}

pub fn absent_group(group_idx: IVec) {
    store().group = Some((group_idx, None));
}
