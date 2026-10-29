// pxl-spec/procedural.test.rs

use super::*;
use crate::space::layer::lattice::GROUND;

/// Caves are some of the ground, not none and not most, over leaves spread
/// across many of the caves' first cubes rather than in a few, where one
/// cave decides the share.
#[test]
fn caves_hollow_some() {
    let mut n = 0;
    for x in 0..64 {
        for y in 0..64 {
            for z in -32..0 {
                n += usize::from(Cave::of(GROUND.cell(), [x * 4093, y * 4099, z * 61]).hollow());
            }
        }
    }
    let share = n as f64 / (64.0 * 64.0 * 32.0);
    eprintln!("cave share {share}");
    assert!((0.005..0.1).contains(&share), "{share}");
}

/// The caves fold to themselves: at every zoom a cube is the majority of its
/// eight children.
#[test]
fn cave_cube_majority() {
    for zoom in CAVE_COARSEST - 1..GROUND.cell() {
        for x in 0..6 {
            for y in 0..6 {
                for z in -6..0 {
                    let c = [x, y, z];
                    let hollow = (0..8)
                        .filter(|&k| {
                            Cave::of(zoom + 1, [0, 1, 2].map(|a| 2 * c[a] + (k >> a & 1))).hollow()
                        })
                        .count();
                    assert_eq!(
                        hollow > 4,
                        Cave::of(zoom, c).hollow(),
                        "z{zoom} {c:?}: {hollow} of 8"
                    );
                }
            }
        }
    }
}
