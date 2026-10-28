// pxl-render/look/atmosphere.test.rs

use super::*;
use crate::look::color::linear_rgb;

#[test]
fn a_colour_in_linear_light_is_its_hex() {
    // Written to four places.
    for (hex, linear) in [(0x9fb4d6, NIGHT_COLOR), (0xf9f0da, CITY_COLOR)] {
        for (want, got) in linear_rgb(hex).into_iter().zip(linear) {
            assert!(
                (want - got).abs() < 5e-5,
                "#{hex:06x}: {want} against {got}"
            );
        }
    }
}
