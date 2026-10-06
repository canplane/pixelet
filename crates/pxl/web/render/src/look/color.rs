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
    xpute_core::math::scalar::round(s * 255.0).clamp(0.0, 255.0) as u8
}

/// The mean of sRGB colors, taken in linear light.
pub fn mean_srgb(colors: impl IntoIterator<Item = [u8; 3]>) -> Option<[u8; 3]> {
    let (mut sum, mut n) = ([0.0; 3], 0.0);
    for c in colors {
        for k in 0..3 {
            sum[k] += linear(c[k]);
        }
        n += 1.0;
    }
    (n > 0.0).then(|| sum.map(|v| srgb(v / n)))
}
