// pxl-content/generation.test.rs

use super::*;
use pxl_engine::cast::{Detail, Spacing};

/// Decor of the leaves' clip level is drawn where the rays ask the leaf cell's
/// zoom or finer at its distance, decor of a coarser clip level where they ask
/// that clip level's zoom alone, so a distance draws one clip level's, and
/// placed decor where they ask the coarsest zoom it stands at or finer.
#[test]
fn decor_at_its_clip_level() {
    let detail = Detail {
        range: [0, FINEST_ZOOM],
        spacing: Spacing::NONE,
    };
    let mut d = 0.25f32;
    let mut seen = [false; 2];
    while d < 1.0e6 {
        let d2 = d * d;
        let at = detail.at(d2);
        assert_eq!(
            drawn_at(at, LEAF_CELL, false),
            at >= LEAF_CELL,
            "at {d}: z{at}"
        );
        for cell in FIRST_CELL..=LEAF_CELL {
            assert_eq!(drawn_at(at, cell, true), at >= cell, "placed at {d}: z{at}");
        }
        for cell in FIRST_CELL..LEAF_CELL {
            assert_eq!(
                drawn_at(at, cell, false),
                at == cell,
                "at {d}: z{at}, z{cell}"
            );
        }
        seen[usize::from(at >= LEAF_CELL)] = true;
        d *= 1.5;
    }
    assert_eq!(seen, [true, true], "both sides of the leaves are asked");
}

/// The clip levels holding a brick present at `held` and coarser alone.
struct HeldTo(Zoom);

impl clipmap::Presence for HeldTo {
    fn present(&self, cell: Zoom, _: [i32; 2]) -> (u32, bool) {
        (0, cell <= self.0)
    }
}

/// Decor generated at a coarser clip level is drawn where the rays ask a
/// finer zoom that no clip level holds at its column (open sea's empty
/// pages), as the ground drawn there is that clip level's; where a finer one
/// holds it, the finer one's alone.
#[test]
fn decor_at_held_clip_level() {
    let (coarse, c) = (LEAF_CELL - 2, [12_345, 678]);
    for at in coarse..=FINEST_ZOOM {
        let held = held_at(&HeldTo(coarse), at, c);
        assert!(drawn_at(held, coarse, false), "z{at} over z{coarse}");
        let finer = held_at(&HeldTo(LEAF_CELL), at, c);
        assert_eq!(drawn_at(finer, coarse, false), at == coarse, "z{at}");
    }
    assert_eq!(held_at(&HeldTo(coarse), coarse - 1, c), coarse - 1);
}

/// A footing word gives back what it holds, every top cell a unit's in the
/// world's height, and is no record's, nor `RETAINED` nor `UNGENERATED`.
#[test]
fn footing_word_round_trips() {
    for held in [false, true] {
        for base in [-(1 << 15), -2, -1, 0, 1, 7, (1 << 15) - 1] {
            let w = footing_word(held, base);
            assert!(is_footing(w), "{held} {base}");
            assert_eq!(footing_of(w), (held, base));
            assert_ne!(w, RETAINED);
            assert_ne!(w, UNGENERATED);
        }
    }
    assert!(!is_footing(RETAINED) && !is_footing(UNGENERATED));
    assert!(!is_footing(record_word(3, 255, 3, pxl_engine::FINEST_ZOOM)));
}
