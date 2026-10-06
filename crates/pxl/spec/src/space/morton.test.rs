// pxl-spec/space/morton.test.rs

use super::*;
use crate::space::hierarchy::{ancestor_of, first_descendant_of, slot_of};
use crate::space::zoom::shamt;

const POINTS: [[i64; 3]; 6] = [
    [0, 0, 0],
    [-1, -1, -1],
    [-(1 << 23), -(1 << 23), -(1 << 15)],
    [(1 << 23) - 1, (1 << 23) - 1, (1 << 15) - 1],
    [3_620_000, 1_520_000, 41],
    [-5_000_000, 777, -2_000],
];

fn xy([x, y, _]: [i64; 3]) -> [i64; 2] {
    [x, y]
}

#[test]
fn a_coarser_key_is_a_prefix_of_a_finer_one() {
    for p in POINTS {
        for fine in 0..=UNIT_Z {
            for coarse in 0..=fine {
                let cut = bits::<3>(fine) - bits::<3>(coarse);
                assert_eq!(key(fine, p).checked_shr(cut).unwrap_or(0), key(coarse, p), "3D {p:?} z{coarse} under z{fine}");
                let cut = bits::<2>(fine) - bits::<2>(coarse);
                assert_eq!(key(fine, xy(p)).checked_shr(cut).unwrap_or(0), key(coarse, xy(p)), "2D {p:?} z{coarse} under z{fine}");
            }
        }
    }
}

/// Z has no digit until its lane is reached, so to z8 a cube is its column.
#[test]
fn a_3d_key_is_the_2d_one_to_z8() {
    for p in POINTS {
        for zoom in 0..=8 {
            assert_eq!(key(zoom, p), key(zoom, xy(p)), "{p:?} z{zoom}");
        }
        assert_eq!(bits::<3>(9), bits::<2>(9) + 1);
    }
}

#[test]
fn a_key_reads_back_as_its_node_s_corner() {
    for p in POINTS {
        for zoom in 0..=UNIT_Z {
            let corner: [i64; 3] = base(zoom, key(zoom, p));
            assert_eq!(key(zoom, corner), key(zoom, p));
            let floor = |v: i64, w: u32| ((v + (1 << (w - 1))) >> (UNIT_Z - zoom).min(w as Zoom)) << (UNIT_Z - zoom).min(w as Zoom);
            assert_eq!(corner[0] + (1 << 23), floor(p[0], 24), "{p:?} z{zoom}");
            assert_eq!(corner[2] + (1 << 15), floor(p[2], 16), "{p:?} z{zoom}");
        }
    }
}

/// Each zoom's digit is the child slot the hierarchy numbers, so a key is a walk down it.
#[test]
fn a_digit_is_the_child_s_slot() {
    for p in POINTS {
        for zoom in 1..=UNIT_Z {
            let at = |z: Zoom| ancestor_of(xy(p).map(|v| v + (1 << 23)), shamt(z) as u32);
            assert_eq!(key(zoom, xy(p)) & 3, slot_of(at(zoom)) as u64, "{p:?} z{zoom}");
            assert_eq!(first_descendant_of(at(zoom - 1), 1), at(zoom).map(|v| v & !1));
        }
    }
}

#[test]
fn the_unit_s_3d_key_fills_the_word() {
    assert_eq!(key(UNIT_Z, [(1 << 23) - 1, (1 << 23) - 1, (1 << 15) - 1]), u64::MAX);
    assert_eq!(key(UNIT_Z, [-(1 << 23), -(1 << 23), -(1 << 15)]), 0);
}
