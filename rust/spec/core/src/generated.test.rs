// pxl-spec-core/generated.test.rs

use super::*;
use crate::cast::Spacing;

/// Decor of the leaves' window is drawn where the rays ask the leaf cell's
/// zoom or finer at its distance, decor of a coarser window where they ask
/// that window's zoom alone, so a distance draws one window's, and placed
/// decor where they ask the coarsest zoom it stands at or finer.
#[test]
fn decor_at_its_window() {
    let detail = Detail {
        range: [0, crate::FINEST_ZOOM],
        eye: [0.0; 3],
        spacing: Spacing::NONE,
    };
    let mut d = 0.25f32;
    let mut seen = [false; 2];
    while d < 1.0e6 {
        let d2 = d * d;
        let at = detail.at(d2);
        assert_eq!(
            drawn_at(detail, d2, LEAF_CELL, false),
            at >= LEAF_CELL,
            "at {d}: z{at}"
        );
        for cell in FIRST_CELL..=LEAF_CELL {
            assert_eq!(
                drawn_at(detail, d2, cell, true),
                at >= cell,
                "placed at {d}: z{at}"
            );
        }
        for cell in FIRST_CELL..LEAF_CELL {
            assert_eq!(
                drawn_at(detail, d2, cell, false),
                at == cell,
                "at {d}: z{at}, z{cell}"
            );
        }
        seen[usize::from(at >= LEAF_CELL)] = true;
        d *= 1.5;
    }
    assert_eq!(seen, [true, true], "both sides of the leaves are asked");
}
