// pxl-render/frame/draw.test.rs

use super::*;
use crate::frame::shader::record::PIPELINES;

/// The draws that read the shadow map are the lit ones: decor, its ghost and impostor, the
/// actors, the ghost box and the beacon; the zone, the markers and the debug lines read none.
#[test]
fn lit_draws_read_the_map() {
    for p in [
        PIPELINE_DECOR,
        PIPELINE_GHOST_BOX,
        PIPELINE_GHOST_DECOR,
        PIPELINE_SKINNED,
        PIPELINE_LOCATION_BEACON,
        PIPELINE_IMPOSTOR,
    ] {
        assert!(reads_shadow_map(p), "{p}");
    }
    for p in [PIPELINE_LOCATION_ZONE, PIPELINE_MARKER, PIPELINE_DEBUG] {
        assert!(!reads_shadow_map(p), "{p}");
    }
    assert!((0..PIPELINES).any(|p| !reads_shadow_map(p)));
}

/// A draw that reads the map stands its box; past `light::STANDS` the boxes are taken into the
/// last, which holds every one.
#[test]
fn every_lit_draw_stands_its_box() {
    let mut d = DrawList::default();
    let n = light::STANDS + 5;
    for k in 0..n {
        let v = k as f32;
        d.draw_as(
            PIPELINE_LOCATION_BEACON,
            0,
            0,
            0,
            [v; 3],
            0.0,
            1.0,
            Some([[v; 3], [v + 1.0; 3]]),
        );
    }
    d.draw_as(PIPELINE_MARKER, 0, 0, 0, [0.0; 3], 0.0, 1.0, None);
    assert_eq!(d.stands.len(), light::STANDS);
    let last = d.stands[light::STANDS - 1];
    assert_eq!(last[0], [(light::STANDS - 1) as f32; 3]);
    assert_eq!(last[1], [n as f32; 3]);
    for (k, b) in d.stands[..light::STANDS - 1].iter().enumerate() {
        assert_eq!(*b, [[k as f32; 3], [k as f32 + 1.0; 3]]);
    }
}

/// A ball's box holds the ball decor_ball.wgsl draws a receiver by.
#[test]
fn ball_holds_decor_ball() {
    let [lo, hi] = DrawList::ball([1.0, 2.0, 3.0]);
    let r = (TALLEST + WIDEST) as f32;
    assert_eq!(lo, [1.0 - r, 2.0 - r, 3.0 + TALLEST as f32 * 0.5 - r]);
    assert_eq!(hi, [1.0 + r, 2.0 + r, 3.0 + TALLEST as f32 * 0.5 + r]);
}

/// A draw that reads the map and gives no box is refused, so none skips standing.
#[test]
#[should_panic]
fn a_lit_draw_without_its_box_is_refused() {
    let mut d = DrawList::default();
    d.draw_as(PIPELINE_LOCATION_BEACON, 0, 0, 0, [0.0; 3], 0.0, 1.0, None);
}
