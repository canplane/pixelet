// pxl-spec/bake/constant.test.rs

use xpute_core::golden::Golden;

use super::*;

const GOLDEN: &str = include_str!("../../golden/bake/constant.tsv");

#[test]
fn constants_are_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    assert_eq!(CELL.shamt(), v.i32("CELL"));
    assert_eq!(U_PER_CELL.to_bits(), v.f64("U_PER_CELL").to_bits());
    assert_eq!(XY_CELL_PER_M.to_bits(), v.f64("XY_CELL_PER_M").to_bits());
    for (j, y) in [0.0, 1234567.8, -4500000.0, 12000000.0].into_iter().enumerate() {
        assert_eq!(Z_CELL_PER_M(y).to_bits(), v.f64(&format!("Z_CELL_PER_M.{j}")).to_bits(), "Z_CELL_PER_M({y})");
    }
    assert_eq!(crate::space::pyramid::PAGE_CELLS, v.u32("PAGE_CELLS"));
    assert_eq!(crate::space::pyramid::CHUNK_CELLS, v.u32("CHUNK_CELLS"));
    assert_eq!(PAGE_ELE_SPAN, v.u32("PAGE_ELE_SPAN"));
    assert_eq!(ELE_PAD, v.u32("ELE_PAD"));
    assert_eq!(ELE_PADDED_SPAN, v.u32("ELE_PADDED_SPAN"));
    assert_eq!(
        (OBJECT_LAYOUT.dim, OBJECT_LAYOUT.min.shamt(), OBJECT_LAYOUT.max.shamt()),
        (v.u32("OBJECT_LAYOUT.dim"), v.i32("OBJECT_LAYOUT.min"), v.i32("OBJECT_LAYOUT.max"))
    );
}

#[test]
fn a_level_0_cell_is_the_standard_z16_pixel_exactly() {
    use crate::space::adapter::epsg3857::M_PER_U;
    // The OGC WebMercatorQuad z0 resolution, 256-pixel tiles: the world's
    // width over 256. At z16 it is the metres a ground cell spans.
    let z16_m_per_px = 156543.03392804097 / (1u64 << 16) as f64;
    assert_eq!(M_PER_U * U_PER_CELL, z16_m_per_px);
    assert_eq!(crate::space::pyramid::zoom_of_level(0), 15);
    assert_eq!(crate::space::pyramid::zoom_of_level(crate::space::pyramid::TOP_LEVEL), 5);
}

/// A cell's Z is its band's lowest cell and its place in the band, the floor
/// on either side of zero; a step is its cell in the band and the steps
/// into it.
#[test]
fn a_cell_z_is_its_band_and_a_step_its_cell() {
    for z in [-129i64, -64, -1, 0, 63, 64, 130] {
        let (band, local) = band_of(z);
        assert!(local < BAND_CELLS);
        assert_eq!(band_cell_z(band) + i64::from(local), z);
    }
    for step in [[0u32, 0, 0], [CELL_STEPS - 1, CELL_STEPS, 3 * CELL_STEPS + 1]] {
        let (cell, into) = in_cell(step);
        assert!(into.iter().all(|&v| v < CELL_STEPS));
        assert_eq!(std::array::from_fn(|i| cell[i] * CELL_STEPS + into[i]), step);
    }
}
