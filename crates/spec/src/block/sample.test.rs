// pxl-spec/block/sample.test.rs

use crate::bake::constant::ELE_PADDED_SPAN;
use crate::block::cube::sentinel;
use crate::space::layer::tile::{Tile, LEAF_ZOOM};

use super::*;

/// A tile past the coverage gives no surface, so it puts no floor under
/// what is placed there.
#[test]
fn a_chunk_with_no_surface_has_no_ground_and_no_floor() {
    let cells = vec![sentinel::void; (TILE_CELLS * TILE_CELLS) as usize];
    let ele = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    let separation = Separation::default();
    let tile = Tile::new(LEAF_ZOOM, 0);
    let own = TileSource {
        tile: TileView {
            cells: &cells,
            ele: &ele,
            separation: &separation,
        },
        frame: Frame::of(tile),
        origin: [0, 0],
        n: 1,
    };
    let mut sample = ChunkSample::of_span(COL_SPAN);
    sample.read(&Around { own, beside: [None; 8] }, [0, 0], COL_SPAN).unwrap();
    assert_eq!(sample.floor, i32::MIN);
    let i = apron_col(COL_SPAN, 0, 0) as usize;
    assert!(!sample.has_ground(i));
    let mut runs = 0;
    sample
        .ground_runs(i, sample.floor, i32::MAX, |_, _, _| {
            runs += 1;
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(runs, 0);
}

/// A folded tile against a baked one differs only in the outermost padding
/// sample, the bicubic's first tap (weight at most ~0.074), read by rim cells.
/// The step it leaves must sit under a chunk's one-cell skirt.
#[test]
fn a_tile_folded_without_its_ring_seams_under_a_cell() {
    use crate::bake::constant::{ELE_PAD, TILE_ELE_SPAN};
    use crate::bake::fold::fold_ele;

    let padded = ELE_PADDED_SPAN as usize;
    let inner = TILE_ELE_SPAN as i64;
    let pad = ELE_PAD as i64;
    // One unit a sample, a 1:16 slope over a sample's sixteen cells: the worst case.
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

    // The first tap reaches sample 0 only for cells within half a sample of the edge.
    let rim = (TILE_CELLS / TILE_ELE_SPAN / 2) as i32;
    let span = TILE_CELLS as i32;
    let (mut worst_rim, mut worst_inside) = (0i32, 0i32);
    for ty in 0..span {
        for tx in 0..span {
            let d = (bicubic(&baked, f64::from(tx) + 0.5, f64::from(ty) + 0.5) as i32 - bicubic(&folded, f64::from(tx) + 0.5, f64::from(ty) + 0.5) as i32).abs();
            let at_rim = tx < rim || ty < rim || tx >= span - rim || ty >= span - rim;
            let worst = if at_rim { &mut worst_rim } else { &mut worst_inside };
            *worst = (*worst).max(d);
        }
    }
    assert_eq!(worst_inside, 0, "every cell that cannot reach the outermost sample is the bake's");
    assert!(worst_rim > 0, "and the rim is where the two part, or this measures nothing");
    // The skirt is a cell, one unit at the leaves, the tightest the bound is.
    assert!(worst_rim <= 1, "a folded tile stands {worst_rim} units off the baked one");
}
