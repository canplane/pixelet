// pxl-spec/asset/cube.rs

//! The cube kind (`spec/spec/asset/cube.json`): the ground and the cubes, in one table
//! the GPU reads too, laid into the value space (value.rs), and a cube as the catalog gives it
//! out to be placed.

use std::sync::LazyLock;

#[path = "cube.spec.rs"]
mod spec;
pub use spec::{ground, object, sentinel, Ground, Object};

use crate::asset::color;
use crate::asset::id::AssetId;
use crate::value;

// The palette laid into the value space: the sentinels are the value space's, every ground
// entry lies in the ground's range and the ways' exactly in the ways', a pier is drawn as a
// building, and the object's colors lie in their range.
const _: () = {
    assert!(
        sentinel::void as u32 == value::VOID
            && sentinel::unknown as u32 == value::UNKNOWN
            && object::air as u32 == value::AIR,
        "the palette's sentinels and air are the value space's"
    );
    assert!(
        Ground::building as u32 == value::PIER,
        "a pier is drawn as a building"
    );
    let mut k = 0;
    while k < Ground::ALL.len() {
        let g = Ground::ALL[k];
        assert!(
            value::grounds(g as u32),
            "a ground entry in the ground's slots"
        );
        assert!(
            value::transports(g as u32) == g.transport(),
            "a way's entry in the ways' slots, and no other"
        );
        k += 1;
    }
    let mut k = 0;
    while k < Object::ALL.len() {
        assert!(
            value::objects(Object::ALL[k] as u32),
            "an object's color in the objects' slots"
        );
        k += 1;
    }
};

/// A cube of the catalog (`asset/cube.json`) is its id: a palette entry of the
/// ground's, a byte, or past a byte one that is only ever placed; a player may
/// place either. None for VOID, UNKNOWN, AIR or a decor kind.
pub fn cube_of(id: AssetId) -> Option<AssetId> {
    let known = match u8::try_from(id) {
        Ok(byte) => Ground::of(byte).is_some(),
        Err(_) => u16::try_from(id).ok().and_then(Object::of).is_some(),
    };
    known.then_some(id)
}

pub fn cube_rgb(id: AssetId) -> Option<u32> {
    match u8::try_from(id) {
        Ok(byte) => color::ground(byte),
        Err(_) => color::object(u16::try_from(id).ok()?),
    }
    .map(|s| s.rgb)
}

/// The placed colors first, then the palette in paint order.
pub static ALL_CUBES: LazyLock<Vec<AssetId>> = LazyLock::new(|| {
    Object::ALL
        .into_iter()
        .map(|o| o as AssetId)
        .chain(Ground::ALL.into_iter().map(|g| g as AssetId))
        .collect()
});

#[cfg(test)]
#[path = "cube.test.rs"]
mod test;
