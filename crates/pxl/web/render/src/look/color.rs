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
