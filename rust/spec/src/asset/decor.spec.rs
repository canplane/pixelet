// pxl-spec/asset/decor.spec.rs
//
// GENERATED from spec/spec/asset/decor.json
//
// The decors, the cube catalog's sibling: drawn as a model, not a cell the rays meet.
//
// Numbered in the decor kind (id.json), so a placed leaf's value says it is
// decor: an object value is a u16.
//
// Each decor is said here once: its number, its models, and how the ground
// generates it on its own. A decor's `seed` names its scatter, so changing it
// reshuffles every one the ground generates; `density` is the chance a cell of
// its holds one, the mean of a tree of hashes of the cells (pxl-content
// scatter.rs); `on` the palette entries it may stand on; `orient` a random
// heading, or along the road or rail its cell lies on. A variant's
// skin is its pack's colormap, kept outside the glb.
//
// A variant's `size` is the box it claims, in its decor's cells: every cell
// of it is the decor's. Its model is drawn as made, a glb unit a unit, its
// origin at the middle of the box's base, where it stands: no origin is
// moved, and glTF fixes none, so a model must be made with it there. It may
// reach a little past the box. A decor's cell is a unit, a z24 cell. The
// ground generates a decor only on its own palette entries, and no two decors share one, so
// two decors' boxes never meet.
//
// A decor thing carries `meta`, the fields its id gives it beside where it
// stands: which variant of its decor, its heading in quarter turns (an odd
// one swaps its box's sides) and its scale. The ground evaluates them from
// its cell; a placed one's are the placement's.
//
// Names as the spec spells them, so one record has one name on every side.
#![allow(
    non_upper_case_globals,
    non_camel_case_types,
    clippy::absolute_paths,
    reason = "generated from the spec"
)]
pub const tree: u16 = 1024;
pub const boat: u16 = 1025;
pub const car: u16 = 1026;
pub const train: u16 = 1027;

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
pub struct Row {
    /// What a placed one is stored as.
    pub val: u32,
    /// Its scatter: changing it reshuffles every one the ground generates.
    pub seed: u32,
    /// The chance a cell of its holds one, over 2^32: a cell's mean
    /// (pxl-content scatter.rs `cnt`).
    pub chance: u32,
    pub orient: Orientation,
    /// The palette entries it may stand on, a bit each.
    pub on: [u32; 8],
    pub variants: u32,
    /// Each variant's box, in its decor's cells; none past `variants`.
    pub sizes: [[u32; 3]; VARIANTS],
}

#[rustfmt::skip]
pub const ROWS: [Row; 4] = [
    // tree
    Row {
        val: 1024,
        seed: 0x7902530c,
        chance: 107374182,
        orient: Orientation::random,
        on: [0x00000902, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000],
        variants: 9,
        sizes: [[1, 1, 3], [1, 1, 3], [1, 1, 3], [1, 1, 3], [1, 1, 3], [1, 1, 2], [1, 1, 2], [1, 1, 2], [1, 1, 2]],
    },
    // boat
    Row {
        val: 1025,
        seed: 0xb0a5802d,
        chance: 4294967,
        orient: Orientation::random,
        on: [0x00000000, 0x00000001, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000],
        variants: 4,
        sizes: [[3, 2, 1], [2, 4, 3], [2, 4, 5], [2, 3, 2], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]],
    },
    // car
    Row {
        val: 1026,
        seed: 0x0ca63010,
        chance: 12884902,
        orient: Orientation::along_transport,
        on: [0x00000000, 0x0001e800, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000],
        variants: 5,
        sizes: [[1, 3, 1], [1, 3, 1], [1, 3, 1], [1, 3, 1], [1, 3, 1], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]],
    },
    // train
    Row {
        val: 1027,
        seed: 0x7ba5802d,
        chance: 6442451,
        orient: Orientation::along_transport,
        on: [0x00000000, 0x007c0000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000],
        variants: 4,
        sizes: [[1, 3, 2], [1, 3, 2], [1, 3, 2], [1, 3, 2], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]],
    },
];

/// A model and its pack's colormap, each a slot the catalog fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variant {
    pub model: crate::asset::catalog::Slot,
    pub skin: crate::asset::catalog::Slot,
}

/// A decor: its id, its row (how the ground generates it and the boxes its
/// variants claim, `Row`) and its variants' models, in
/// the row's order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decor {
    pub id: crate::asset::id::AssetId,
    pub row: &'static Row,
    pub variants: &'static [Variant],
}

/// What a decor thing carries beside where it stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecorMeta {
    pub variant: u32,
    pub turn: u32,
    pub scale: f64,
}

pub const DECORS: [Decor; 4] = [
    // Trees on green and bare land. Bare is the fallback type for untagged land
    // and this pack draws it as grass; sports fields are left out as maintained,
    // not natural, ground. Sparse: a hillside with some trees, not a forest wall
    // at 1u spacing.
    Decor {
        id: tree as u32,
        row: &ROWS[0],
        variants: &[
            Variant {
                model: crate::asset::catalog::Slot(1),
                skin: crate::asset::catalog::Slot(2),
            },
            Variant {
                model: crate::asset::catalog::Slot(3),
                skin: crate::asset::catalog::Slot(4),
            },
            Variant {
                model: crate::asset::catalog::Slot(5),
                skin: crate::asset::catalog::Slot(6),
            },
            Variant {
                model: crate::asset::catalog::Slot(7),
                skin: crate::asset::catalog::Slot(8),
            },
            Variant {
                model: crate::asset::catalog::Slot(9),
                skin: crate::asset::catalog::Slot(10),
            },
            Variant {
                model: crate::asset::catalog::Slot(11),
                skin: crate::asset::catalog::Slot(12),
            },
            Variant {
                model: crate::asset::catalog::Slot(13),
                skin: crate::asset::catalog::Slot(14),
            },
            Variant {
                model: crate::asset::catalog::Slot(15),
                skin: crate::asset::catalog::Slot(16),
            },
            Variant {
                model: crate::asset::catalog::Slot(17),
                skin: crate::asset::catalog::Slot(18),
            },
        ],
    },
    // Boats on open water: rivers, lakes and the sea near a coast; wetland is
    // too shallow. The sea is the same `water` ground, and boats stand on it
    // near a coast alone: a tile of open sea is left out of the bake and drawn
    // from a coarser one, at whose cells no boat stands (pxl-content
    // `scatter::stands_at`), so the clip levels a boat stands at hold sea only in
    // the tiles a coast crosses. Rivers are a few cells wide, so the same per-cell
    // rate reads far denser than on land.
    Decor {
        id: boat as u32,
        row: &ROWS[1],
        variants: &[
            Variant {
                model: crate::asset::catalog::Slot(19),
                skin: crate::asset::catalog::Slot(20),
            },
            Variant {
                model: crate::asset::catalog::Slot(21),
                skin: crate::asset::catalog::Slot(22),
            },
            Variant {
                model: crate::asset::catalog::Slot(23),
                skin: crate::asset::catalog::Slot(24),
            },
            Variant {
                model: crate::asset::catalog::Slot(25),
                skin: crate::asset::catalog::Slot(26),
            },
        ],
    },
    // Cars on major roads only: neighborhood streets and pedestrian ways are
    // where users build, and a car there reads as in the way. Aligned to the
    // road, since a random heading reads as a crash. Sparse: a car every couple
    // of cells reads as gridlock.
    Decor {
        id: car as u32,
        row: &ROWS[2],
        variants: &[
            Variant {
                model: crate::asset::catalog::Slot(27),
                skin: crate::asset::catalog::Slot(28),
            },
            Variant {
                model: crate::asset::catalog::Slot(29),
                skin: crate::asset::catalog::Slot(30),
            },
            Variant {
                model: crate::asset::catalog::Slot(31),
                skin: crate::asset::catalog::Slot(32),
            },
            Variant {
                model: crate::asset::catalog::Slot(33),
                skin: crate::asset::catalog::Slot(34),
            },
            Variant {
                model: crate::asset::catalog::Slot(35),
                skin: crate::asset::catalog::Slot(36),
            },
        ],
    },
    // Trains on rail lines the ground shows (a tunnel's is not painted), aligned
    // to the rail, since one lying across it reads as derailed. Very sparse:
    // trains should read as occasional.
    Decor {
        id: train as u32,
        row: &ROWS[3],
        variants: &[
            Variant {
                model: crate::asset::catalog::Slot(37),
                skin: crate::asset::catalog::Slot(38),
            },
            Variant {
                model: crate::asset::catalog::Slot(39),
                skin: crate::asset::catalog::Slot(40),
            },
            Variant {
                model: crate::asset::catalog::Slot(41),
                skin: crate::asset::catalog::Slot(42),
            },
            Variant {
                model: crate::asset::catalog::Slot(43),
                skin: crate::asset::catalog::Slot(44),
            },
        ],
    },
];
