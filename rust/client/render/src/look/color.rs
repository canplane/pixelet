// pxl-render/look/color.rs

use pxl_spec::color::{linear, Rgb};

/// The color `n` in linear light, which every shader lights and blends in.
pub fn linear_rgb(n: Rgb) -> [f64; 3] {
    [16, 8, 0].map(|shift| linear((n >> shift) as u8))
}

/// An indicator's colors.
pub mod indicator {
    use pxl_spec::color::Rgb;

    /// This color is reserved for "you could grab this", never for a preview of a change.
    pub const ACTION: Rgb = 0x4fd1c5;
    /// This color is for removal where the thing cannot be drawn as itself.
    pub const ERASE: Rgb = 0x1d2026;
}
