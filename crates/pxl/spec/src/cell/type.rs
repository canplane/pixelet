// pxl-spec/cell/type.rs

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

/// A catalog's channels and types by name, in declaration order, for tools
/// that print them; nothing at runtime looks one up through it.
pub type Channels = [(&'static str, &'static [(&'static str, u8)])];
