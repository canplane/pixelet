// pxl_core/render/gpu/color.rs

/// An sRGB hex colour (`#rrggbb`) in linear light, which is what every colour
/// the shaders light and blend with is in.
pub fn linear_rgb(hex: &str) -> [f64; 3] {
    let n = u32::from_str_radix(hex.get(1..).unwrap_or(""), 16).unwrap_or(0);
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
