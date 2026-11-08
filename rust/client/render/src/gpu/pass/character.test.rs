// pxl-render/gpu/pass/character.test.rs

use super::*;

/// The sRGB level, 0 to 255, that encode.wgsl `to_srgb` makes of the linear
/// value `c`.
fn levels(c: f32) -> f32 {
    let lo = c * 12.92;
    let hi = 1.055 * libm::powf(c, 1.0 / 2.4) - 0.055;
    255.0 * if c <= 0.0031308 { lo } else { hi }
}

#[test]
fn a_clear_encodes_back_to_the_uis_bytes() {
    // The finish dithers by up to half a level either way before the color
    // is stored. A clear within a tenth of a level of its byte therefore
    // comes back as that byte, except at the dither's very ends.
    for v in 0..=255u32 {
        let [r, g, b, a] = clear_of(v << 16 | v << 8 | v);
        assert_eq!(a, 1.0, "a character surface shows nothing through");
        for c in [r, g, b] {
            assert!((levels(c) - v as f32).abs() < 0.1, "{v} came back {c}");
        }
    }
    let [r, g, b, _] = clear_of(0xcbc6bf);
    let back = [r, g, b].map(|c| levels(c).round_ties_even() as u32);
    assert_eq!(back, [0xcb, 0xc6, 0xbf], "each channel its own");
}
