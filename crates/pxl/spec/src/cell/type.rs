// pxl-spec/cell/type.rs

/// A catalog colour, in both the forms anyone wants it: as written, for a
/// record a person reads, and packed `0xRRGGBB` by the generator, for
/// everything that has to light or blend with it. One table carries both
/// because they are one colour — a second table keyed the same way is how
/// they come to disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swatch {
    pub hex: &'static str,
    pub rgb: u32,
}

impl Swatch {
    pub const fn new(hex: &'static str, rgb: u32) -> Swatch {
        Swatch { hex, rgb }
    }
}

/// A catalog's channels by name, each its types by name, in declaration
/// order: what a tool that prints a channel or a type walks. Nothing that
/// runs finds one through it; a channel is `GroundChannelKey` or a type's own
/// enum, and a colour is color.spec.rs's for that number.
pub type Channels = [(&'static str, &'static [(&'static str, u8)])];
