// pxl-kernel/domain/scene/environment/cloud.test.rs

use super::*;
use crate::domain::view::lens;
use crate::sys;
use pxl_render::frame::draw::{lists_bytes, set_lists};
use xpute_guest::mem::heap::alloc;

/// The drift's step as it was written before the view held the seen step:
/// a twelfth of a cell at the nearest drawn, over the wind's speed.
fn drift_step_s_as_written(seen: f64) -> f64 {
    1.0 / 12.0 * lens::cell_there(seen) / libm::hypot(WIND[0], WIND[1])
}

#[test]
fn cloud_steps_unchanged() {
    let _g = sys::serial();
    let was = clouds().seen;
    for seen in [0.5, 3.0, 47.25, 1.0e3, 6.4e4, 3.1e6, f64::INFINITY] {
        clouds().seen = seen;
        assert_eq!(
            drift_step_s().to_bits(),
            drift_step_s_as_written(seen).to_bits(),
            "at {seen}"
        );
    }
    clouds().seen = was;
}

/// A still eye under the wind: only time moves, and a patch's draw stands in the list, its
/// record and uniform as they were, and so does its stand, until the drift crosses into the
/// next cell, which moves the stand alone; and every stand holds the patch as it is drawn,
/// moved by the drift (`stood_corner`, `patch_stand`).
#[test]
fn only_time_moving_moves_no_draw() {
    let _g = sys::serial();
    const LIVE: usize = 4;
    let (at, _) = alloc(lists_bytes(LIVE)).unwrap();
    // SAFETY: a block of the heap's, the lists' alone for the test's run.
    unsafe { set_lists(at, LIVE) };
    let key = [-3, 7];
    let eye = [
        key[0] as f64 * PATCH + 100.0,
        key[1] as f64 * PATCH + 300.0,
        20.0,
    ];
    let (e, heights) = (900.0, [5000.0, 6200.0]);
    let (mut list, id) = (DrawList::default(), DrawId::default());
    let mut turn = |time: f64| {
        let off = drift(time);
        let stand = patch_stand(stood_corner(key, off, eye), PATCH, e, heights);
        let corner = [
            key[0] as f64 * PATCH - off[0],
            key[1] as f64 * PATCH - off[1],
        ];
        let drawn = Stand::of(
            [corner[0] - e, corner[1] - e, heights[0] - e],
            [corner[0] + PATCH + e, corner[1] + PATCH + e, heights[1] + e],
        );
        for k in 0..3 {
            assert!(
                stand.0[0][k] <= drawn.0[0][k] && drawn.0[1][k] <= stand.0[1][k],
                "{time} s: {stand:?} holds {drawn:?}"
            );
        }
        list.draw_instances(
            &id,
            PIPELINE_DECOR,
            9,
            FIRST_GROUP as u32,
            3,
            [1, 3, 20],
            patch_place(key),
            1.0,
            true,
            Some(stand),
        );
        let l = list.take();
        (l.changes.len(), l.stands.to_vec())
    };
    let (changes, first) = turn(0.0);
    assert_eq!(changes, 1, "drawn first");
    // Within a cell of drift: X's wind carries the field a cell in 4096 / 24 seconds.
    for time in [0.29, 1.0, 10.0, 100.0, 170.0] {
        assert_eq!(turn(time), (0, first.clone()), "{time} s");
    }
    let (changes, stands) = turn(200.0);
    assert_eq!(changes, 0, "the record stands");
    assert_ne!(stands, first, "past a cell the stand moves");
}
