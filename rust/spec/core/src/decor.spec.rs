// pxl-spec-core/decor.spec.rs
//
// GENERATED from spec/spec/asset/decor.json
//
// The decors, the cube catalog's sibling: drawn as a model, not a cell the rays meet.
//
// The decors as the GPU reads them too: how the ground generates each one, the
// looks it stands on and the boxes its variants claim, in the catalog's
// order. pxl-spec asset/decor.spec.rs adds their models over each row.
#![allow(non_camel_case_types, reason = "generated from the spec")]

/// The most variants a decor has.
pub const VARIANTS: usize = 9;

/// How a decor the ground generates faces: any way, or along the road or rail its
/// cell lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Orientation {
    random,
    along_transport,
}

/// A decor as the ground generates it, in 32-bit words alone, laid out as a
/// GPU reads the table from a buffer of these words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Decor {
    /// What a placed one is stored as.
    pub val: u32,
    /// Its scatter: changing it reshuffles every one the ground generates.
    pub seed: u32,
    /// The chance a cell of its holds one, over 2^32: a cell's mean
    /// (pxl-spec-core scatter.rs `count`).
    pub chance: u32,
    pub orientation: Orientation,
    /// The looks it may stand on, a bit each.
    pub on: [u32; 8],
    /// Its cell: a unit's zoom, this many coarser.
    pub scale_steps: i32,
    pub variants: u32,
    /// Each variant's box, in its decor's cells; none past `variants`.
    pub sizes: [[u32; 3]; VARIANTS],
}

#[rustfmt::skip]
pub const DECORS: [Decor; 4] = [
    // tree
    Decor {
        val: 1024,
        seed: 0x7902530c,
        chance: 171798692,
        orientation: Orientation::random,
        on: [0x00000902, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000],
        scale_steps: 0,
        variants: 9,
        sizes: [[1, 1, 3], [1, 1, 3], [1, 1, 3], [1, 1, 3], [1, 1, 3], [1, 1, 2], [1, 1, 2], [1, 1, 2], [1, 1, 2]],
    },
    // boat
    Decor {
        val: 1025,
        seed: 0xb0a5802d,
        chance: 8589935,
        orientation: Orientation::random,
        on: [0x00000000, 0x00000001, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000],
        scale_steps: 0,
        variants: 4,
        sizes: [[3, 2, 1], [2, 4, 3], [2, 4, 5], [2, 3, 2], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]],
    },
    // car
    Decor {
        val: 1026,
        seed: 0x0ca63010,
        chance: 17179869,
        orientation: Orientation::along_transport,
        on: [0x00000000, 0x0001e800, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000],
        scale_steps: 0,
        variants: 5,
        sizes: [[1, 3, 1], [1, 3, 1], [1, 3, 1], [1, 3, 1], [1, 3, 1], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]],
    },
    // train
    Decor {
        val: 1027,
        seed: 0x7ba5802d,
        chance: 17179869,
        orientation: Orientation::along_transport,
        on: [0x00000000, 0x007c0000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000],
        scale_steps: 0,
        variants: 4,
        sizes: [[1, 3, 2], [1, 3, 2], [1, 3, 2], [1, 3, 2], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]],
    },
];
