// pxl-spec/color.rs

//! A color as the code passes it: sRGB, its channels packed into one `u32`; and one channel
//! between sRGB and linear light, the light every shader lights and blends in.

/// `0xRRGGBB`.
pub type Rgb = u32;

/// `0xRRGGBBAA`, its alpha straight, not premultiplied.
pub type Rgba = u32;

/// One sRGB channel, 0 to 255, in linear light.
pub fn linear(v: u8) -> f64 {
    let c = f64::from(v) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::pow((c + 0.055) / 1.055, 2.4)
    }
}

/// Linear light `c`, held to 0 to 1, as the nearest sRGB channel, 0 to 255.
pub fn srgb(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * libm::pow(c, 1.0 / 2.4) - 0.055
    };
    (s * 255.0).round_ties_even() as u8
}
