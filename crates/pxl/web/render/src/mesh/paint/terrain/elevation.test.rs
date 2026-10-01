// pxl-render/mesh/paint/terrain/elevation.test.rs

use pxl_spec::space::pyramid::PAGE_CELLS;

use super::*;

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
    use pxl_spec::bake::constant::{ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN};
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
    // tap lands on sample 0 only while `tx`, in samples, `+ ELE_PAD - 0.5`
    // falls inside the first sample, which is the cells within half a sample
    // of the page's edge.
    let rim = (PAGE_CELLS / PAGE_ELE_SPAN / 2) as i32;
    let span = PAGE_CELLS as i32;
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
