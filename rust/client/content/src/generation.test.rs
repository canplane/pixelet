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
            drawn_at(at, LEAF_CELL_ZOOM, false),
            at >= LEAF_CELL_ZOOM,
            "at {d}: z{at}"
        );
        for cell_zoom in FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM {
            assert_eq!(
                drawn_at(at, cell_zoom, true),
                at >= cell_zoom,
                "placed at {d}: z{at}"
            );
        }
        for cell_zoom in FIRST_CELL_ZOOM..LEAF_CELL_ZOOM {
            assert_eq!(
                drawn_at(at, cell_zoom, false),
                at == cell_zoom,
                "at {d}: z{at}, z{cell_zoom}"
            );
        }
        seen[usize::from(at >= LEAF_CELL_ZOOM)] = true;
        d *= 1.5;
    }
    assert_eq!(seen, [true, true], "both sides of the leaves are asked");
}

/// Clip levels that hold a brick present only at its zoom and coarser.
struct PresentTo(Zoom);

impl clipmap::Presence for PresentTo {
    fn present(&self, cell_zoom: Zoom, _: [i32; 2]) -> (u32, bool) {
        (0, cell_zoom <= self.0)
    }
}

/// Decor generated at a coarser clip level is drawn where the rays ask a
/// finer zoom that no clip level holds at its column (open sea's empty
/// pages), as the ground drawn there is that clip level's; where a finer one
/// holds it, the finer one's alone.
#[test]
fn decor_at_held_clip_level() {
    let (coarse, c) = (LEAF_CELL_ZOOM - 2, [12_345, 678]);
    for at in coarse..=FINEST_ZOOM {
        let read = ray_zoom(&PresentTo(coarse), at, c);
        assert!(drawn_at(read, coarse, false), "z{at} over z{coarse}");
        let finer = ray_zoom(&PresentTo(LEAF_CELL_ZOOM), at, c);
        assert_eq!(drawn_at(finer, coarse, false), at == coarse, "z{at}");
    }
    assert_eq!(ray_zoom(&PresentTo(coarse), coarse - 1, c), coarse - 1);
}
