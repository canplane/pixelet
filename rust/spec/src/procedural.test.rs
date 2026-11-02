// pxl-spec/procedural.test.rs

use super::*;
use crate::space::layer::lattice::GROUND;

/// Caves are some of the ground, not none and not most, over leaves spread
/// across many of the caves' lattice cells rather than in a few, where one
/// cave decides the share.
#[test]
fn caves_hollow_some() {
    let mut n = 0;
    for x in 0..64 {
        for y in 0..64 {
            for z in -32..0 {
                n += usize::from(hollow((GROUND.cell(), [x * 4093, y * 4099, z * 61])));
            }
        }
    }
    let share = n as f64 / (64.0 * 64.0 * 32.0);
    eprintln!("cave share {share}");
    assert!((0.005..0.2).contains(&share), "{share}");
}

/// A cube a cave is said to take all of holds no leaf that is not hollow,
/// and some are said so.
#[test]
fn all_hollow_holds_its_leaves() {
    let mut said = 0;
    for zoom in CAVE_COARSEST..GROUND.cell() {
        let side = 1 << (GROUND.cell() - zoom);
        let n = 4096 / side;
        for x in 0..n {
            for y in 0..4 {
                for z in -64 / side..0 {
                    let c = [x, y * 37, z];
                    if !all_hollow((zoom, c)) {
                        continue;
                    }
                    said += 1;
                    for k in 0..side * side * side {
                        let leaf = [
                            c[0] * side + k % side,
                            c[1] * side + k / side % side,
                            c[2] * side + k / (side * side),
                        ];
                        assert!(hollow((GROUND.cell(), leaf)), "z{zoom} {c:?}: {leaf:?}");
                    }
                }
            }
        }
    }
    assert!(said > 0);
}
