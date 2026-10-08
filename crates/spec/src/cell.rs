// pxl-spec/cell.rs

//! The cell model: the generated tables — colors, cubes, cube types, decor,
//! their types — and `object`, where a cell becomes the value the object
//! store keeps.

#[path = "cell/color.spec.rs"]
pub mod color;
#[path = "cell/cube_type.spec.rs"]
mod cube_type;
#[path = "cell/decor.spec.rs"]
pub mod decor;
pub mod ext;
#[path = "cell/id.spec.rs"]
pub mod id;
#[path = "cell/marker.spec.rs"]
pub mod marker;
pub mod object;

pub use cube_type::{cube_types, Kind};
pub use decor::{decor_types, DecorId};

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

impl decor::DecorKind {
    /// What a placed one is stored as.
    pub const fn value(&self) -> u16 {
        self.decor as u16
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

    /// Where its variants are numbered from: the variants of the kinds before it.
    pub fn first_group(&self) -> u32 {
        decor::KINDS.iter().take_while(|k| k.decor != self.decor).map(|k| k.variant_count()).sum()
    }

    /// The generator writes one a decor, so every decor has its kind.
    pub fn of(decor: DecorId) -> &'static decor::DecorKind {
        use xpute_kit::status::bug::OrBug;
        decor::KINDS.iter().find(|k| k.decor == decor).or_bug(xpute_kit::status::errno::Errno::ENOTRECOVERABLE)
    }
}

fn largest(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}
