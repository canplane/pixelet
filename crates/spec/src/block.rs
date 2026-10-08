// pxl-spec/block.rs

//! The block catalog: the generated tables — the ids' kinds, the cubes and
//! their colors, the decors, the marker, what an id carries beyond itself —
//! and `object`, where a block becomes the value the object store keeps.

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

pub use decor::DecorId;

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

    pub const fn variant_count(&self) -> u32 {
        self.variants.len() as u32
    }

    /// Whether the ground grows it on a cell of `look`.
    pub fn allows(&self, look: crate::bake::format::GroundCell) -> bool {
        self.on.contains(&look)
    }

    /// The largest over every variant's model, in units.
    pub fn extent(&self) -> f64 {
        self.variants.iter().map(decor::Variant::extent).fold(0.0, largest)
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

fn largest(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}
