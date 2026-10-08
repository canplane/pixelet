// pxl-render/look/color.rs

/// Linear light, which every shader lights and blends in.
pub fn linear_rgb(n: u32) -> [f64; 3] {
    [16, 8, 0].map(|shift| linear((n >> shift) as u8))
}

/// One sRGB channel in linear light.
pub fn linear(v: u8) -> f64 {
    let c = v as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::pow((c + 0.055) / 1.055, 2.4)
    }
}

/// Linear light back to an sRGB channel.
pub fn srgb(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * libm::pow(c, 1.0 / 2.4) - 0.055 };
    xpute_kit::math::scalar::round(s * 255.0).clamp(0.0, 255.0) as u8
}

/// An indicator's colors.
pub mod indicator {
    /// Reserved for "you could grab this", never for a preview of a change.
    pub const ACTION: u32 = 0x4fd1c5;
    /// For removal where the thing cannot be drawn as itself. Dark, since users
    /// read ACTION's cyan as "move".
    pub const ERASE: u32 = 0x1d2026;
}
