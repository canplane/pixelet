// pxl-render/frame/shader/word.test.rs

use std::path::Path;

use super::*;

#[test]
fn the_moon_map_is_as_wide_as_the_sky_reads_it() {
    // download_moon writes the map; the sky binds and reads it at
    // `MOON_TEXELS` a side, so a map cut anew at another size must move it.
    let png = std::fs::read(Path::new(env!("PXL_REPO")).join("asset/content/sky/moon.png"))
        .expect("the Moon's map (make download-moon)");
    let side = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    assert_eq!(
        (side(16), side(20)),
        (MOON_TEXELS, MOON_TEXELS),
        "the PNG's header against `MOON_TEXELS`"
    );
}
