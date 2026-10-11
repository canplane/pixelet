// pxl-spec/block.rs

//! The block catalog: the generated tables — the ids' kinds, the cubes and
//! their colors, the decors, the marker, what an id carries beyond itself —
//! `object`, where a block becomes the value the object store keeps, and what
//! is in each block of the world, composed from the ground and what is placed.

#[path = "block/color.spec.rs"]
pub mod color;
#[path = "block/cube.spec.rs"]
pub mod cube;
#[path = "block/decor.spec.rs"]
pub mod decor;
pub mod ext;
#[path = "block/id.spec.rs"]
pub mod id;
#[path = "block/marker.spec.rs"]
pub mod marker;
pub mod object;
pub mod sample;
pub mod stand;

pub use decor::DecorId;

use crate::space::coord::aabb::Aabb;
use crate::space::coord::zoom::{units, Zoom, UNIT_ZOOM};

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

    /// Its box's sides in cells, turned `turn` quarter turns: an odd one swaps
    /// x and y, the box keeping its min corner.
    pub const fn cells(&self, turn: u32) -> [u32; 3] {
        let [x, y, z] = self.size;
        if turn.is_multiple_of(2) {
            [x, y, z]
        } else {
            [y, x, z]
        }
    }
}

impl decor::Decor {
    /// What a placed one is stored as.
    pub const fn value(&self) -> u16 {
        self.id as u16
    }

    pub const fn variant_count(&self) -> u32 {
        self.variants.len() as u32
    }

    /// Whether the ground grows it on a cell of `look`.
    pub fn allows(&self, look: crate::bake::tile::terrain::GroundId) -> bool {
        self.on.contains(&look)
    }

    /// Its cell's zoom: a unit's, `scale_steps` coarser.
    pub const fn cell_zoom(&self) -> Zoom {
        (UNIT_ZOOM as i16 - self.scale_steps as i16) as Zoom
    }

    /// The largest over every variant's model, in units.
    pub fn extent(&self) -> f64 {
        self.variants.iter().map(decor::Variant::extent).fold(0.0, largest)
    }

    /// The widest side any variant's box has, in cells: two corners nearer
    /// than this on both axes may claim one cell.
    pub fn widest(&self) -> u32 {
        self.variants.iter().map(|v| v.size[0].max(v.size[1])).max().unwrap_or(1)
    }

    /// How far any variant's box, or its model about the box's middle, reaches
    /// from the box's min corner however it turns, in units.
    pub fn reach(&self) -> f64 {
        let cell = units(self.cell_zoom());
        let reach = |v: &decor::Variant| {
            let [ex, ey, ez] = v.model.ingested().extent.unwrap_or([0.0; 3]);
            let [x, y, z] = v.size.map(|n| f64::from(n) * cell);
            let across = x.max(y).max(ex).max(ey);
            libm::sqrt(2.0 * across * across + z.max(ez).powi(2))
        };
        self.variants.iter().map(reach).fold(0.0, largest)
    }

    /// The sides of `variant`'s box turned `turn`, in units.
    fn sides(&self, variant: u32, turn: u32) -> [f64; 3] {
        self.variants[variant as usize].cells(turn).map(|n| f64::from(n) * units(self.cell_zoom()))
    }

    /// `variant` turned `turn` with its box's min corner at `corner`, in
    /// units: where it stands, its base's middle, and what it carries at
    /// `scale`.
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
        decor::DECORS.iter().take_while(|d| d.id != self.id).map(|d| d.variant_count()).sum()
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
