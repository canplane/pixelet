// pxl-spec/block/sample.test.rs

use crate::bake::constant::ELE_PADDED_SPAN;
use crate::block::cube::sentinel;
use crate::space::layer::tile::{Tile, LEAF_ZOOM};

use super::*;

/// `laid` with the cells column `i` has a cave in taken out, as `ground_runs`
/// takes them out of all but bedrock.
fn carved(sample: &ChunkSample, i: usize, laid: &[(i32, i32, GroundId)]) -> Vec<(i32, i32, GroundId)> {
    if !caves_in(sample.cell) {
        return laid.to_vec();
    }
    let rock = crate::block::limit::bedrock(sample.ele[i], sample.scale);
    let (mut out, mut caves) = (Vec::new(), sample.caves(i));
    for &(lo, hi, g) in laid {
        let mut start = None;
        for z in lo..=hi + 1 {
            let solid = z <= hi && (g == sentinel::unknown || z >= sample.ele[i] - sample.scale || z < rock || !caves.hollow(sample.cell_z(z)));
            match (solid, start) {
                (true, None) => start = Some(z),
                (false, Some(s)) => {
                    out.push((s, z - 1, g));
                    start = None;
                }
                _ => {}
            }
        }
    }
    out
}

/// A tile past the coverage gives no surface, and no ground at any height.
#[test]
fn a_chunk_with_no_surface_has_no_ground() {
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
        past: None,
    };
    let mut sample = ChunkSample::of_span(COL_SPAN);
    sample.read(&Around { own, beside: [None; 8] }, [0, 0], COL_SPAN, GROUND.cell()).unwrap();
    let i = apron_col(COL_SPAN, 0, 0) as usize;
    assert!(!sample.has_ground(i));
    let mut runs = 0;
    sample
        .ground_runs(i, i32::MIN, i32::MAX, |_, _, _| {
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

/// A column is laid as Minecraft Beta lays one: its look on its face, its
/// filler under it, sand over sandstone, and stone under all.
#[test]
fn a_column_is_its_look_over_its_filler_over_stone() {
    use crate::bake::constant::ELE_PADDED_SPAN;

    let check = |look: GroundId, cell: Zoom, face: i16, from: i32, strata: &[(i32, i32, GroundId)]| {
        let cells = vec![look; (TILE_CELLS * TILE_CELLS) as usize];
        let ele = vec![face; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
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
            past: None,
        };
        let mut sample = ChunkSample::of_span(COL_SPAN);
        sample.read(&Around { own, beside: [None; 8] }, [0, 0], COL_SPAN, cell).unwrap();
        let i = apron_col(COL_SPAN, 0, 0) as usize;
        let mut laid = Vec::new();
        sample
            .ground_runs(i, from, i32::MAX, |lo, hi, g| {
                laid.push((lo, hi, g));
                Ok::<(), ()>(())
            })
            .unwrap();
        // The strata, where the world has no cave.
        assert_eq!(laid, carved(&sample, i, strata), "{look} at {cell}, face {face}");
    };
    let leaf = GROUND.cell();
    check(ground::grass, leaf, 10, 0, &[(0, 5, ground::stone), (6, 8, ground::soil), (9, 9, ground::grass)]);
    check(ground::sand, leaf, 10, 0, &[(0, 3, ground::stone), (4, 5, ground::sandstone), (6, 9, ground::sand)]);
    check(ground::water, leaf, 10, 0, &[(0, 5, ground::stone), (6, 9, ground::water)]);
    // Coarser than the leaves, a cell is a summary of its faces: its look whole.
    for coarser in [leaf - 1, leaf - 5, GROUND.cell_zoom(GROUND.top())] {
        check(ground::grass, coarser, 10, 0, &[(0, 9, ground::grass)]);
    }
    // Bedrock under all, `BEDROCK_DEPTH` under sea level, or under a face below it.
    let unknown = crate::block::cube::sentinel::unknown;
    check(
        ground::grass,
        leaf,
        10,
        -200,
        &[(-200, -65, unknown), (-64, 5, ground::stone), (6, 8, ground::soil), (9, 9, ground::grass)],
    );
    check(
        ground::grass,
        leaf,
        -100,
        -200,
        &[(-200, -165, unknown), (-164, -105, ground::stone), (-104, -102, ground::soil), (-101, -101, ground::grass)],
    );
    // Bedrock is a stratum too: coarser than the leaves, the look whole.
    check(ground::grass, leaf - 1, 10, -200, &[(-200, 9, ground::grass)]);
}

/// Past the leaves a cell is finer than a leaf's, so its ground keeps its
/// strata, each a leaf cell thick in its finer cells.
#[test]
fn past_the_leaves_strata_stay_a_leaf_cell_thick() {
    use crate::bake::constant::ELE_PADDED_SPAN;

    let cells = vec![ground::grass; (TILE_CELLS * TILE_CELLS) as usize];
    let ele = vec![10i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    let separation = Separation::default();
    let own = TileSource {
        tile: TileView {
            cells: &cells,
            ele: &ele,
            separation: &separation,
        },
        frame: Frame::of(Tile::new(LEAF_ZOOM, 0)),
        origin: [0, 0],
        n: 1,
        past: None,
    };
    let mut leaf = ChunkSample::of_span(COL_SPAN);
    leaf.read(&Around { own, beside: [None; 8] }, [0, 0], COL_SPAN, GROUND.cell()).unwrap();
    let mut finer = ChunkSample::of_span(COL_SPAN);
    finer.refine(&leaf, 1, [0, 0], COL_SPAN).unwrap();
    let mut laid = Vec::new();
    finer
        .ground_runs(apron_col(COL_SPAN, 0, 0) as usize, 0, i32::MAX, |lo, hi, g| {
            laid.push((lo, hi, g));
            Ok::<(), ()>(())
        })
        .unwrap();
    let i = apron_col(COL_SPAN, 0, 0) as usize;
    assert_eq!(laid, carved(&finer, i, &[(0, 11, ground::stone), (12, 17, ground::soil), (18, 19, ground::grass)]));
}

/// The service area is walled one cell thick: the first column past it is
/// bedrock up to the face beside it, and the one past that stays open.
#[test]
fn the_service_area_is_walled_one_cell_thick() {
    let mut cells = vec![ground::grass; (TILE_CELLS * TILE_CELLS) as usize];
    for y in 0..TILE_CELLS {
        for x in 8..TILE_CELLS {
            cells[GROUND.cell_index(x, y) as usize] = sentinel::unknown;
        }
    }
    let ele = vec![10i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    let separation = Separation::default();
    let own = TileSource {
        tile: TileView {
            cells: &cells,
            ele: &ele,
            separation: &separation,
        },
        frame: Frame::of(Tile::new(LEAF_ZOOM, 0)),
        origin: [0, 0],
        n: 1,
        past: None,
    };
    let mut sample = ChunkSample::of_span(COL_SPAN);
    sample.read(&Around { own, beside: [None; 8] }, [0, 0], COL_SPAN, GROUND.cell()).unwrap();
    let laid = |x: u32| {
        let mut laid = Vec::new();
        sample
            .ground_runs(apron_col(COL_SPAN, x, 4) as usize, -100, i32::MAX, |lo, hi, g| {
                laid.push((lo, hi, g));
                Ok::<(), ()>(())
            })
            .unwrap();
        laid
    };
    assert_eq!(laid(8), [(-100, 9, sentinel::unknown)]);
    assert!(laid(9).is_empty());
    assert_eq!(laid(7).last(), Some(&(9, 9, ground::grass)));
}

/// Caves take the ground where the world has them, about an edit, last of
/// all: never the bedrock, the ground's face, nor a column by a way.
#[test]
fn caves_come_last() {
    let mut cells = vec![ground::grass; (TILE_CELLS * TILE_CELLS) as usize];
    for y in 0..TILE_CELLS {
        cells[GROUND.cell_index(16, y) as usize] = ground::road;
    }
    let ele = vec![40i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    let separation = Separation::default();
    let own = TileSource {
        tile: TileView {
            cells: &cells,
            ele: &ele,
            separation: &separation,
        },
        frame: Frame::of(Tile::new(LEAF_ZOOM, 0)),
        origin: [0, 0],
        n: 1,
        past: None,
    };
    let mut sample = ChunkSample::of_span(COL_SPAN);
    sample.read(&Around { own, beside: [None; 8] }, [0, 0], COL_SPAN, GROUND.cell()).unwrap();
    let column = |sample: &ChunkSample, x: u32, y: u32| {
        let mut laid = Vec::new();
        sample
            .ground_runs(apron_col(COL_SPAN, x, y) as usize, -100, i32::MAX, |lo, hi, g| {
                laid.push((lo, hi, g));
                Ok::<(), ()>(())
            })
            .unwrap();
        laid
    };
    let mut caved = 0;
    for y in 0..COL_SPAN - 2 * APRON {
        for x in 0..COL_SPAN - 2 * APRON {
            let laid = column(&sample, x, y);
            assert_eq!(laid.first(), Some(&(-100, -65, sentinel::unknown)), "bedrock at {x}, {y}");
            assert_eq!(laid.last().map(|r| r.1), Some(39), "the face at {x}, {y}");
            let gaps = laid.windows(2).filter(|w| w[1].0 > w[0].1 + 1).count();
            assert!(gaps == 0 || !(15..=17).contains(&x), "a cave by the way at {x}, {y}");
            caved += usize::from(gaps > 0);
        }
    }
    assert!(caved > 0, "no cave anywhere");
}

/// Past the service area a cell is nothing, whatever the tile folded there:
/// land read as `unknown`, the bedrock a wall is made of.
#[test]
fn a_cell_past_the_service_area_is_no_ground_whatever_the_tile_holds() {
    let cells = vec![ground::land; (TILE_CELLS * TILE_CELLS) as usize];
    let ele = vec![5i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    let separation = Separation::default();
    let source = |past: Option<fn([f64; 2]) -> bool>| TileSource {
        tile: TileView {
            cells: &cells,
            ele: &ele,
            separation: &separation,
        },
        frame: Frame::of(Tile::new(LEAF_ZOOM, 0)),
        origin: [0, 0],
        n: 1,
        past,
    };
    assert_eq!(source(None).look(3, 4), ground::land);
    assert_eq!(source(Some(|_| true)).look(3, 4), sentinel::unknown);
    assert_eq!(source(Some(|_| true)).cell_at([3, 4]).map(|(look, _)| look), Some(sentinel::unknown));
}
