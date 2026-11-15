// pxl-core/domain/scene/environment/cloud.test.rs

use super::*;
use crate::domain::view::lens;
use pxl_content::cloud::drift;
use pxl_render::gpu::draw::{lists_sz, set_lists};

/// The drift's step written out by hand: a twelfth of a camera grid cell at
/// the nearest drawn cloud, over the wind's speed. `drift_step_s` must give
/// the same bits.
fn drift_step_s_as_written(seen: f64) -> f64 {
    1.0 / 12.0 * lens::cell_there(seen) / libm::hypot(WIND[0], WIND[1])
}

#[test]
fn cloud_steps_unchanged() {
    let was = clouds().nearest_dist;
    for seen in [0.5, 3.0, 47.25, 1.0e3, 6.4e4, 3.1e6, f64::INFINITY] {
        clouds().nearest_dist = seen;
        assert_eq!(
            drift_step_s().to_bits(),
            drift_step_s_as_written(seen).to_bits(),
            "at {seen}"
        );
    }
    clouds().nearest_dist = was;
}

/// With the eye still and only time moving the clouds, a patch's draw stays in
/// the list unchanged, its record and uniform as they were. Its stand stays
/// too, until the drift crosses into the next cell, and then only the stand
/// moves. At every time the stand holds the patch as drawn, moved by the
/// drift (`box_corner`, `patch_box`).
#[test]
fn only_time_moving_moves_no_draw() {
    const LIVE: usize = 4;
    set_lists(Vec::leak(vec![0; lists_sz(LIVE) as usize / 4]), LIVE);
    let key = [-3, 7];
    let eye = [
        key[0] as f64 * PATCH + 100.0,
        key[1] as f64 * PATCH + 300.0,
        20.0,
    ];
    let (e, alts) = (900.0, [5000.0, 6200.0]);
    let (mut list, id) = (DrawList::default(), DrawId::default());
    let mut frame = |time: f64| {
        let off = drift(time);
        let receiver = patch_box(box_corner(key, off, eye), PATCH, e, alts);
        let corner = [
            key[0] as f64 * PATCH - off[0],
            key[1] as f64 * PATCH - off[1],
        ];
        let drawn = ReceiverBox::of(
            [corner[0] - e, corner[1] - e, alts[0] - e],
            [corner[0] + PATCH + e, corner[1] + PATCH + e, alts[1] + e],
        );
        for k in 0..3 {
            assert!(
                receiver.0[0][k] <= drawn.0[0][k] && drawn.0[1][k] <= receiver.0[1][k],
                "{time} s: {receiver:?} holds {drawn:?}"
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
            Some(receiver),
        );
        let l = list.take();
        (l.changes.len(), l.receivers.to_vec())
    };
    let (changes, first) = frame(0.0);
    assert_eq!(changes, 1, "drawn first");
    // These times stay within a cell of drift: the wind along X carries the
    // field a cell in 4096 / 24 seconds.
    for time in [0.29, 1.0, 10.0, 100.0, 170.0] {
        assert_eq!(frame(time), (0, first.clone()), "{time} s");
    }
    let (changes, receivers) = frame(200.0);
    assert_eq!(changes, 0, "the record stands");
    assert_ne!(receivers, first, "past a cell the stand moves");
}
