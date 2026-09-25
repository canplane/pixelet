// pxl-render/look/color.rs

/// A packed `0xRRGGBB` in linear light, which is what every color the shaders
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
