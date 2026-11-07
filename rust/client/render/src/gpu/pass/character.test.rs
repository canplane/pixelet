// pxl-render/gpu/pass/character.test.rs

use super::*;

/// encode.wgsl's `to_srgb` in levels.
fn levels(c: f32) -> f32 {
    let lo = c * 12.92;
    let hi = 1.055 * libm::powf(c, 1.0 / 2.4) - 0.055;
    255.0 * if c <= 0.0031308 { lo } else { hi }
}

#[test]
fn a_clear_encodes_back_to_the_shells_bytes() {
    // The finish's dither is half a level either way: within a tenth of a
    // level, a byte stays itself but at the dither's very ends.
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
