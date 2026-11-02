// pxl-content/cave.test.rs

extern crate std;

use super::*;

/// `fade` runs from none to whole, and falls under a value it took before by
/// `FADE_DROP` at the most, which `noise_span` widens a box's fades by.
#[test]
fn fade_falls_by_its_drop() {
    assert_eq!((fade(0), fade(T_ONE)), (0, T_ONE));
    let (mut most, mut fall) = (0, 0);
    for t in 0..=T_ONE {
        let f = fade(t);
        fall = fall.max(most - f);
        most = most.max(f);
    }
    assert_eq!(fall, FADE_DROP);
}

/// Caves are some of the ground, not none and not most, over leaves spread
/// across many of the caves' lattice cells rather than in a few, where one
/// cave decides the share.
#[test]
fn caves_hollow_some() {
    let mut n = 0;
    for x in 0..64 {
        for y in 0..64 {
            for z in -32..0 {
                n += usize::from(hollow((LEAF_CELL, [x * 4093, y * 4099, z * 61])));
            }
        }
    }
    let share = n as f64 / (64.0 * 64.0 * 32.0);
    assert!((0.005..0.5).contains(&share), "{share}");
}

/// A cube a cave is said to take all of holds no cube under it, at any zoom
/// down past the leaves, that is not hollow; some are said so, on either
/// side of X's wrap and of zero.
#[test]
fn all_hollow_holds_every_cube_under() {
    let mut said = 0;
    for zoom in CAVE_COARSEST..LEAF_CELL {
        let side = 1i32 << (LEAF_CELL - zoom);
        let half = 1i32 << (zoom - 1);
        let n = 1024 / side;
        for x in (-n..n).chain(half - n..half + n) {
            for y in -2..2 {
                for z in -64 / side..0 {
                    let c = [x, y * 37, z];
                    if !all_hollow((zoom, c)) {
                        continue;
                    }
                    said += 1;
                    for under in zoom + 1..=LEAF_CELL + 1 {
                        let k = 1i32 << (under - zoom);
                        for i in 0..k * k * k {
                            let u = [
                                c[0] * k + i % k,
                                c[1] * k + i / k % k,
                                c[2] * k + i / (k * k),
                            ];
                            assert!(hollow((under, u)), "z{zoom} {c:?}: z{under} {u:?}");
                        }
                    }
                }
            }
        }
    }
    assert!(said > 0);
}

/// The caves only take away, and only under a leaf column's face cell, the
/// cell under its top: every cell from it up is what the strata make it,
/// cave or none (pxl-engine `ground::Cell::asset`).
#[test]
fn caves_under_face_alone() {
    use crate::Pixelet;
    use pxl_engine::answer::FULL;
    use pxl_engine::content::Content;
    use pxl_engine::ground::Cell;
    use pxl_engine::value;
    let mut taken = 0;
    for x in 0..64 {
        for y in 0..64 {
            let top = (x * 7 + y * 13) % 40 - 20;
            let column = Cell {
                palette_idx: 7,
                top,
                part: FULL,
                fold: [7; 4],
            };
            for z in top - 90..top + 2 {
                let c = [x * 977, y * 991, z];
                let asset =
                    column.asset::<Pixelet>(LEAF_CELL, z, || Pixelet::hollow((LEAF_CELL, c)));
                let strata = Pixelet::by_depth(LEAF_CELL, column.palette_idx, top, z);
                if z >= top - 1 {
                    assert_eq!(asset, strata, "{c:?}, top {top}");
                } else if asset != strata {
                    assert_eq!(asset, value::VOID, "{c:?}");
                    taken += 1;
                }
            }
        }
    }
    assert!(taken > 0);
}
