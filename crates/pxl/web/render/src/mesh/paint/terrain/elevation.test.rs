// pxl-render/mesh/paint/terrain/elevation.test.rs

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::cell::cube_types;

use super::*;
use pxl_spec::bake::format::GROUND_CH_KEYS;
use pxl_spec::bake::pack::{pack, GroundCell};

const FLAT_ELE: i16 = 100;

/// A flat source raster: bicubic-interpolating a constant field returns that
/// same constant everywhere, with no edge/padding artifacts to account for —
/// isolates the recess logic from the interpolation math it sits on top of.
fn flat_page_ele() -> Vec<i16> {
    vec![FLAT_ELE; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize]
}

/// Channels of `span`² cells, each lane's cell 0 set as given.
fn ground(span: usize, land: u8, water: u8, structure: u8, transport: u8) -> Ground {
    let first = pack(GROUND_CH_KEYS.map(|k| match k {
        GroundChannelKey::LAND => land,
        GroundChannelKey::WATER => water,
        GroundChannelKey::STRUCTURE => structure,
        GroundChannelKey::TRANSPORT => transport,
    }));
    let mut cells = vec![0 as GroundCell; span * span];
    cells[0] = first;
    Ground { cells: cells.into() }
}

#[test]
fn dense_ele_region_a_water_channel_cell_is_recessed_by_its_types_recess() {
    let g = ground(2, 0, cube_types::water::river, 0, 0);
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, Some(&g), &mut out);
    assert_eq!(out[0], FLAT_ELE - 1);
    assert_eq!(out[1], FLAT_ELE);
}

#[test]
fn dense_ele_region_land_sea_is_recessed_too_even_though_it_s_not_the_water_channel() {
    let g = ground(2, cube_types::land::sea, 0, 0, 0);
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, Some(&g), &mut out);
    assert_eq!(out[0], FLAT_ELE - 1);
}

#[test]
fn dense_ele_region_a_structure_over_water_e_g_a_bridge_is_not_recessed() {
    let g = ground(2, 0, cube_types::water::river, cube_types::structure::building, 0);
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, Some(&g), &mut out);
    assert_eq!(out[0], FLAT_ELE);
}

#[test]
fn dense_ele_region_transport_over_water_e_g_a_road_is_not_recessed() {
    let g = ground(2, 0, cube_types::water::lake, 0, cube_types::transport::road_default);
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, Some(&g), &mut out);
    assert_eq!(out[0], FLAT_ELE);
}

#[test]
fn dense_ele_region_omitting_ground_recesses_nothing() {
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, None, &mut out);
    assert_eq!(out[0], FLAT_ELE);
    assert_eq!(out[1], FLAT_ELE);
}

#[test]
fn dense_ele_region_plain_land_is_not_recessed() {
    let g = ground(2, cube_types::land::bare, 0, 0, 0);
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, Some(&g), &mut out);
    assert_eq!(out[0], FLAT_ELE);
}

/// What a page folded from the four under it costs where it meets one the
/// bake wrote, which is the one place the two differ (pxl-spec bake/fold.rs).
///
/// The page and the padding sample beside it are the bake's to the bit; the
/// outermost padding sample is the block's edge extended. That sample is the
/// first tap of the bicubic, whose weight is `-0.5x³ + x² - 0.5x` — at most
/// about 0.074 — and it is read only by the cells along the page's own rim.
/// The step it can leave between two such pages has to sit under the skirt a
/// chunk hangs off its tiles, which is one of its own cells deep
/// (mesh/chunk_mesher.rs).
#[test]
fn a_page_folded_without_its_ring_seams_under_a_cell() {
    use pxl_spec::bake::constant::{ELE_PAD, ELE_PADDED_SPAN, PAGE_CELL_SPAN, PAGE_ELE_SPAN};
    use pxl_spec::bake::fold::fold_ele;

    let padded = ELE_PADDED_SPAN as usize;
    let inner = PAGE_ELE_SPAN as i64;
    let pad = ELE_PAD as i64;
    // A ramp steep enough to be the worst of it: a unit of height every
    // sample, which over the sixteen cells a sample spans is a 1:16 slope.
    let f = |x: i64, y: i64| x + 3 * y;
    let grid = |qx: i64, qy: i64| -> Vec<i16> {
        (0..padded * padded)
            .map(|s| {
                let (ix, iy) = ((s % padded) as i64 - pad, (s / padded) as i64 - pad);
                f(qx * inner + ix, qy * inner + iy) as i16
            })
            .collect()
    };
    let grids: Vec<Vec<i16>> = (0..16).map(|k| grid(k as i64 % 4 - 1, k as i64 / 4 - 1)).collect();
    let all: [Option<&[i16]>; 16] = std::array::from_fn(|k| Some(grids[k].as_slice()));
    let four: [Option<&[i16]>; 16] = std::array::from_fn(|k| [5, 6, 9, 10].contains(&k).then(|| grids[k].as_slice()));

    let mut baked = vec![0i16; padded * padded];
    let mut folded = vec![0i16; padded * padded];
    fold_ele(all, &mut baked);
    fold_ele(four, &mut folded);

    // Which cells can reach the outermost sample at all: the bicubic's first
    // tap lands on sample 0 only while `tx / ELE_CELL_STEP + ELE_PAD - 0.5`
    // falls inside the first sample, which is the cells within half a sample
    // of the page's edge.
    let rim = (ELE_CELL_STEP / 2.0) as i32;
    let span = PAGE_CELL_SPAN as i32;
    let (mut worst_rim, mut worst_inside) = (0i32, 0i32);
    for ty in 0..span {
        for tx in 0..span {
            let d = (sample_ele(&baked, tx, ty) as i32 - sample_ele(&folded, tx, ty) as i32).abs();
            let at_rim = tx < rim || ty < rim || tx >= span - rim || ty >= span - rim;
            let worst = if at_rim { &mut worst_rim } else { &mut worst_inside };
            *worst = (*worst).max(d);
        }
    }
    assert_eq!(worst_inside, 0, "every cell that cannot reach the outermost sample is the bake's");
    assert!(worst_rim > 0, "and the rim is where the two part, or this measures nothing");
    // A chunk's skirt is one of its own cells, and a cell is one unit at the
    // leaves — the finest a page is ever drawn at, so the tightest the bound
    // ever is.
    assert!(worst_rim <= 1, "a folded page stands {worst_rim} units off the baked one");
}
