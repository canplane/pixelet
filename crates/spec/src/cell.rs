// pxl-spec/cell.rs

//! The cell model: the generated tables — colors, cubes, cube types, decor,
//! their types — and `object`, where a cell becomes the value the object
//! store keeps.

#[path = "cell/color.spec.rs"]
pub mod color;
#[path = "cell/cube_type.spec.rs"]
pub mod cube_type;
#[path = "cell/decor.spec.rs"]
pub mod decor;
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
