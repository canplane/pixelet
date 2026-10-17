// pxl-spec/block.rs

//! The block catalog: the generated tables — the ids' kinds, the cubes and
//! their colors, the decors, the marker, what an id carries beyond itself —
//! and `object`, where a block becomes the value the object store keeps.

#[path = "block/color.spec.rs"]
pub mod color;
/// The ground and the cubes, one table with the GPU's (pxl-portable).
pub use pxl_portable::cube;
pub use pxl_portable::mask;
pub use pxl_portable::placed;
/// The ground's cells as they are held, one layout with the GPU's.
pub use pxl_portable::window;
#[path = "block/decor.spec.rs"]
pub mod decor;
pub mod ext;
#[path = "block/id.spec.rs"]
pub mod id;
pub mod limit;
#[path = "block/marker.spec.rs"]
pub mod marker;
pub mod object;
pub mod volume;

pub use decor::DecorId;

use crate::bake::tile::terrain::GroundId;
use crate::space::coord::aabb::Aabb;
use crate::space::coord::zoom::units;

/// What is in a block: the ground's look, a way's or a pier's, or what is
/// placed, air included (pxl-kernel query/node.rs `drawn` answers which).
/// Where there is none, it is void. The ground's carries the looks its block
/// folds: coarser than the leaves a cell folds the cells under it, its color
/// theirs; at the leaves and finer, its own four times.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Ground(GroundId, [GroundId; 4]),
    Placed(u32),
}

impl Block {
    /// Whether it fills its block: ground does, and of what is placed a cube
    /// that is not air; decor stands in a block it leaves empty.
    pub fn fills(self) -> bool {
        match self {
            Block::Ground(..) => true,
            Block::Placed(state) => !object::object_value_is_air(state) && object::cube_of(state).is_some(),
        }
    }
}

/// A catalog color, packed `0xRRGGBB` by the generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swatch {
    pub rgb: u32,
}

impl Swatch {
    pub const fn new(rgb: u32) -> Swatch {
        Swatch { rgb }
    }
}

impl decor::Variant {
    /// The largest dimension of its model, in units; NaN is kept rather than
    /// dropped, so a model the ingest could not measure is never taken for none.
    pub fn extent(&self) -> f64 {
        self.model.ingested().extent.unwrap_or([0.0; 3]).into_iter().fold(0.0, largest)
    }
}

impl decor::Decor {
    /// What a placed one is stored as.
    pub const fn value(&self) -> u16 {
        self.id as u16
    }

    /// The largest over every variant's model, in units.
    pub fn extent(&self) -> f64 {
        self.variants.iter().map(decor::Variant::extent).fold(0.0, largest)
    }

    /// How far any variant's box, or its model about the box's middle, reaches
    /// from the box's min corner however it turns, in units.
    pub fn reach(&self) -> f64 {
        let cell = units(self.row.cell_zoom());
        let reach = |(v, size): (&decor::Variant, &[u32; 3])| {
            let [ex, ey, ez] = v.model.ingested().extent.unwrap_or([0.0; 3]);
            let [x, y, z] = size.map(|n| f64::from(n) * cell);
            let (across, up) = (x.max(y).max(ex).max(ey), z.max(ez));
            libm::sqrt(2.0 * across * across + up * up)
        };
        self.variants.iter().zip(&self.row.sizes).map(reach).fold(0.0, largest)
    }

    /// The sides of `variant`'s box turned `turn`, in units.
    fn sides(&self, variant: u32, turn: u32) -> [f64; 3] {
        self.row.cells(variant, turn).map(|n| f64::from(n) * units(self.row.cell_zoom()))
    }

    /// `variant` turned `turn` with its box's min corner at `corner`, in
    /// units: where it stands, its base's middle, which its model's origin is
    /// put at, and what it carries at `scale`.
    pub fn stand(&self, corner: [f64; 3], variant: u32, turn: u32, scale: f64) -> ([f64; 3], decor::DecorMeta) {
        let [x, y, _] = self.sides(variant, turn);
        let at = [corner[0] + x / 2.0, corner[1] + y / 2.0, corner[2]];
        (at, decor::DecorMeta { variant, turn, scale })
    }

    /// The box one standing at `at`, carrying `m`, claims: every cell in it is its.
    pub fn claim(&self, at: [f64; 3], m: &decor::DecorMeta) -> Aabb {
        let [x, y, z] = self.sides(m.variant, m.turn);
        let min = [at[0] - x / 2.0, at[1] - y / 2.0, at[2]];
        Aabb {
            min,
            max: [min[0] + x, min[1] + y, min[2] + z],
        }
    }

    /// Where its variants are numbered from: the variants of the decors before it.
    pub fn first_group(&self) -> u32 {
        decor::DECORS.iter().take_while(|d| d.id != self.id).map(|d| d.row.variants).sum()
    }

    /// The generator writes one entry a decor, so every id has one.
    pub fn of(id: DecorId) -> &'static decor::Decor {
        use xpute_kit::status::bug::OrBug;
        decor::DECORS.iter().find(|d| d.id == id).or_bug(xpute_kit::status::errno::Errno::ENOTRECOVERABLE)
    }
}

impl decor::DecorMeta {
    /// Its heading in radians.
    pub fn spin(&self) -> f64 {
        f64::from(self.turn % 4) * core::f64::consts::FRAC_PI_2
    }
}

fn largest(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}
