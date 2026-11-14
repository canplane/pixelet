// pxl-spec/value.rs

//! wgsl-port: the value space that every layer stores and the engine reads. A ground cell
//! is a byte. An object's value is a u16: its byte range is the ground's, and the range past
//! a byte holds what only an object can be. Decor values lie past the cube kind
//! (`asset/id.json`). This fixes the ground's and the objects' ranges around the sentinels
//! (`asset/cube.json` `sentinel`), reads the ways' off the ground's records, and checks the
//! palette against them.

use crate::asset::cube::{self, Ground, Object, Sentinel};

/// The ground's asset ids, first and last: every byte but the
/// sentinels.
pub const GROUND: [u32; 2] = [0x01, 0xfe];

/// The ways' cubes among the ground's, first and last: the ground records marked
/// `transport` (`asset/cube.json`), which lie in one run. Decor faces along a
/// way, and no pier stands on one.
pub const TRANSPORT: [u32; 2] = {
    let mut run = [u32::MAX, 0];
    let mut k = 0;
    while k < Ground::ALL.len() {
        if cube::GROUND[k].transport {
            let v = Ground::ALL[k] as u32;
            if v < run[0] {
                run[0] = v;
            }
            if v > run[1] {
                run[1] = v;
            }
        }
        k += 1;
    }
    run
};

/// The colors only an object takes, first and last: past air, within the
/// cube kind (`asset/id.json` `IDX_BITS`).
pub const OBJECT: [u32; 2] = [0x101, 0x3ff];

/// Whether a value read into a word is a ground asset id (`GROUND`). A GPU has no byte
/// type, so every value is tested as a word.
pub const fn is_ground(val: u32) -> bool {
    val >= GROUND[0] && val <= GROUND[1]
}

/// Whether a value read into a word is a way's cube (`TRANSPORT`).
pub const fn is_transport(val: u32) -> bool {
    val >= TRANSPORT[0] && val <= TRANSPORT[1]
}

/// Whether a value read into a word is a color only an object takes
/// (`OBJECT`).
pub const fn is_object(val: u32) -> bool {
    val >= OBJECT[0] && val <= OBJECT[1]
}

// The palette laid into the value space: no class holds a sentinel, every ground cube lies
// in the ground's range, the ways' cubes lie in one run, so the ways' range holds them and
// no other (`TRANSPORT`), and the object's colors lie in their range.
const _: () = {
    let mut k = 0;
    while k < Sentinel::ALL.len() {
        let v = Sentinel::ALL[k] as u32;
        assert!(!is_ground(v) && !is_object(v), "no class holds a sentinel");
        k += 1;
    }
    let mut k = 0;
    while k < Ground::ALL.len() {
        let g = Ground::ALL[k];
        assert!(is_ground(g as u32), "a ground cube in the ground's range");
        assert!(
            is_transport(g as u32) == cube::GROUND[k].transport,
            "the ways' cubes lie in one run"
        );
        k += 1;
    }
    let mut k = 0;
    while k < Object::ALL.len() {
        assert!(
            is_object(Object::ALL[k] as u32),
            "an object's color in the objects' range"
        );
        k += 1;
    }
};
