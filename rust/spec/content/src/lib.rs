// pxl-content/lib.rs

//! gpu-portable: pixelet's content, one source for the CPU and, through
//! rust-gpu, WGSL, as the engine's is (pxl-engine): the clouds' field
//! (`cloud`). The engine names none of it: pxl-spec, the kernel, the
//! renderer and the shader crate name it where they hand the engine its
//! content.

#![no_std]

pub mod cloud;
pub mod param;

// Pixelet's palette laid into the engine's value space (pxl-engine
// value.rs): its sentinels the engine's, every ground entry in the ground's
// slots, the ways' exactly in the ways', a pier drawn as a building, the
// object's colors in theirs, and decor past them all.
const _: () = {
    use pxl_engine::cube::{object, sentinel, Ground, Object};
    use pxl_engine::decor::DECORS;
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
