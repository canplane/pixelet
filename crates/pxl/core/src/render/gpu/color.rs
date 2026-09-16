// pxl_core/render/gpu/color.rs

/// A packed `0xRRGGBB` in linear light, which is what every colour the shaders
/// light and blend with is in.
pub fn linear_rgb(n: u32) -> [f64; 3] {
    let c = |v: u32| {
        let s = v as f64 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    [c((n >> 16) & 255), c((n >> 8) & 255), c(n & 255)]
}

/// The same, for the one source still written as text: the environment themes
/// (module/world/environment/theme.json), whose `fog` crosses to the shell as
/// a CSS colour and so keeps its `#rrggbb` while its neighbours in the same
/// object have not yet been split from it.
pub fn linear_rgb_hex(hex: &str) -> [f64; 3] {
    linear_rgb(u32::from_str_radix(hex.get(1..).unwrap_or(""), 16).unwrap_or(0))
}
