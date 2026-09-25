// pxl-spec/cell/type.rs

/// A catalog color, packed `0xRRGGBB` by the generator from the hex
/// color.json writes it in: what everything that has to light or blend with
/// it reads. The hex is the JSON's and the shell's; nothing here reads it,
/// so the module does not carry it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swatch {
    pub rgb: u32,
}

impl Swatch {
    pub const fn new(rgb: u32) -> Swatch {
        Swatch { rgb }
    }
}

/// A catalog's channels by name, each its types by name, in declaration
/// order: what a tool that prints a channel or a type walks. Nothing that
/// runs finds one through it; a channel is `GroundChannelKey` or a type's own
/// enum, and a color is color.spec.rs's for that number.
pub type Channels = [(&'static str, &'static [(&'static str, u8)])];
