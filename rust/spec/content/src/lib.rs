// pxl-content/lib.rs

//! gpu-portable: pixelet's content, the rules the CPU runs and the GPU's WGSL
//! is written to, as the engine's are (pxl-engine): the rules the engine
//! asks of it (pxl-engine `content::Content`), answered by `Pixelet`: its
//! caves (`cave`), its strata (`strata`) and its decor's bounds; its palette
//! (`cube`, `decor`), laid into the engine's value space; the decor the
//! ground holds and how it is generated (`scatter`, `generation`); and the
//! clouds' field (`cloud`). pxl-spec, the kernel and the renderer name
//! `Pixelet` where they hand the engine its content.

#![no_std]

pub mod cave;
pub mod cloud;
#[path = "cube.spec.rs"]
pub mod cube;
#[path = "decor.spec.rs"]
pub mod decor;
pub mod generation;
pub mod param;
pub mod scatter;
pub mod share;
pub mod strata;

use pxl_engine::content::Content;
use pxl_engine::Zoom;

/// Pixelet's content: the rules its world follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pixelet;

impl Content for Pixelet {
    fn hollow(node: (Zoom, [i32; 3])) -> u32 {
        cave::hollow(node)
    }
    fn by_depth(cell: Zoom, palette_idx: u32, top: i32, z: i32) -> u32 {
        strata::by_depth(cell, palette_idx, top, z)
    }
    fn bedrock(face: i32) -> i32 {
        strata::bedrock(face)
    }
    fn earth(z: i32) -> u32 {
        strata::earth(z)
    }
}

// Pixelet's palette laid into the engine's value space (pxl-engine
// value.rs): its sentinels the engine's, every ground entry in the ground's
// slots, the ways' exactly in the ways', a pier drawn as a building, the
// object's colors in theirs, and decor past them all.
const _: () = {
    use crate::cube::{object, sentinel, Ground, Object};
    use crate::decor::DECORS;
    use pxl_engine::value;
    assert!(
        sentinel::void as u32 == value::VOID
            && sentinel::unknown as u32 == value::UNKNOWN
            && object::air as u32 == value::AIR,
        "the palette's sentinels and air are the engine's"
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
    let mut k = 0;
    while k < DECORS.len() {
        let v = DECORS[k].val;
        assert!(
            !value::grounds(v) && !value::objects(v) && v != value::AIR,
            "decor past the cube kind"
        );
        k += 1;
    }
};
