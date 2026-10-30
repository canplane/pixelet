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
// reshuffles every one the ground generates; `density` is its mean candidates per
// cell at the cell's zoom, the intensity of the Poisson process that scatters
// it (pxl-spec-core scatter.rs); `on` the looks it may stand on; `fold` the look
// it is folded to in a cell coarser than it stands in, the ground's under it where
// it names none; `orientation`
// a random heading, or along the road or rail its cell lies on. A variant's
// skin is its pack's colormap, kept outside the glb.
//
// A variant's `size` is the box it claims, in its decor's cells: every cell
// of it is the decor's. Its model is drawn as made, a glb unit a unit, its
// origin at the middle of the box's base, where it stands: no origin is
// moved, and glTF fixes none, so a model must be made with it there. It may
// reach a little past the box. A
// decor's cell is a unit, a z24 cell, `scale_steps` zooms coarser (finer
// where negative, as a marker's, marker.json), one for every variant,
// since a placement lands on the cell before its variant is drawn. The
// ground generates a decor only on its own looks, and no two decors share one, so
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum DecorId {
    tree = 1024,
    boat = 1025,
    car = 1026,
    train = 1027,
}

impl DecorId {
    /// Every decor the catalog gives out, in the JSON's own order.
    pub const ALL: [DecorId; 4] = [DecorId::tree, DecorId::boat, DecorId::car, DecorId::train];

    /// The decor a number names, or none for a number the catalog does not
    /// give out.
    pub fn of(val: u16) -> Option<DecorId> {
        DecorId::ALL.into_iter().find(|t| *t as u16 == val)
    }
}

/// How it faces, one row with the GPU's.
pub use pxl_spec_core::decor::Orientation;

/// A model and its pack's colormap, each a slot the catalog fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variant {
    pub model: crate::asset::catalog::Slot,
    pub skin: crate::asset::catalog::Slot,
}

/// A decor: its id, its row (how the ground generates it and the boxes its
/// variants claim, pxl-spec-core decor.spec.rs) and its variants' models, in
/// the row's order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decor {
    pub id: DecorId,
    pub row: &'static pxl_spec_core::decor::Decor,
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
        id: DecorId::tree,
        row: &pxl_spec_core::decor::DECORS[0],
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
    // Boats on open river and lake water; wetland is too shallow. Rivers are a
    // few cells wide, so the per-cell rate reads far denser than on land: cut to
    // a quarter of 0.015 after boats covered the water.
    Decor {
        id: DecorId::boat,
        row: &pxl_spec_core::decor::DECORS[1],
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
        id: DecorId::car,
        row: &pxl_spec_core::decor::DECORS[2],
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
        id: DecorId::train,
        row: &pxl_spec_core::decor::DECORS[3],
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
