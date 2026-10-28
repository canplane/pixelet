// pxl-spec/space/hierarchy/morton.test.rs

use super::*;
use crate::space::hierarchy::tree::{ancestor_of, slot_of};

const POINTS: [[i32; 3]; 6] = [
    [0, 0, 0],
    [-1, -1, -1],
    [-(1 << 23), -(1 << 23), -(1 << 15)],
    [(1 << 23) - 1, (1 << 23) - 1, (1 << 15) - 1],
    [3_620_000, 1_520_000, 41],
    [-5_000_000, 777, -2_000],
];

/// A point's node at `zoom`, a unit point's ancestor.
fn at(p: [i32; 3], zoom: Zoom) -> [i32; 3] {
    ancestor_of(p, UNIT_ZOOM - zoom)
}

fn digits_of(key: u128) -> (u128, u32) {
    let n = 126 - key.trailing_zeros();
    (key >> (127 - n), n)
}

#[test]
fn coarse_key_prefix() {
    for p in POINTS {
        for fine in 0..=UNIT_ZOOM {
            for coarse in 0..=fine {
                let ((f, nf), (c, nc)) = (
                    digits_of(key_of(fine, at(p, fine))),
                    digits_of(key_of(coarse, at(p, coarse))),
                );
                assert_eq!(
                    f.checked_shr(nf - nc).unwrap_or(0),
                    c,
                    "{p:?} z{coarse} under z{fine}"
                );
            }
        }
    }
}

/// Z has no digit until its extent is reached, so to z8 a cube is its column.
#[test]
fn key_3d_extends_2d() {
    for p in POINTS {
        for zoom in 0..=8 {
            let [x, y, z] = at(p, zoom);
            assert_eq!(
                key_of(zoom, [x, y, z]),
                key_of(zoom, [x, y]),
                "{p:?} z{zoom}"
            );
        }
    }
    assert_eq!(bits::<3>(9), bits::<2>(9) + 1);
}

#[test]
fn key_round_trip() {
    for p in POINTS {
        for zoom in 0..=UNIT_ZOOM + 3 {
            let node = ancestor_of(p.map(|v| v << 3), UNIT_ZOOM + 3 - zoom);
            let (z, back) = node_of::<3>(key_of(zoom, node));
            let lane = |v: i32, axis: usize| signed(v, digits(axis, zoom));
            assert_eq!(
                (z, back),
                (zoom, [lane(node[0], 0), lane(node[1], 1), lane(node[2], 2)]),
                "{p:?} z{zoom}"
            );
        }
    }
}

/// Each zoom's digits are the child slot the hierarchy numbers, so a key is a walk down it.
#[test]
fn digit_is_child_slot() {
    for p in POINTS {
        for zoom in 1..=UNIT_ZOOM {
            let [x, y, _] = at(p, zoom);
            let (d, _) = digits_of(key_of(zoom, [x, y]));
            assert_eq!((d & 3) as usize, slot_of([x, y]), "{p:?} z{zoom}");
        }
    }
}
