// pxl-kernel/domain/scene.test.rs

use super::*;
use crate::domain::layer::object::preview::{Ghost, GhostBox};
use crate::domain::scene::model::models;
use crate::domain::view::frame::pose::{view, view_compose, ObserverPose};
use crate::sys;
use pxl_render::frame::draw::{lists_bytes, set_lists, DRAW_PIPELINES};
use pxl_spec::space::coord::mat4;
use xpute_guest::mem::heap::alloc;

/// A turn whose eye moved and whose world did not sets the same draws with
/// the same uniforms and the same stands, so it changes none of them
/// (pxl-render frame/draw.rs `Change`) and the light's receivers read the same:
/// the ghost's box, which reads the shadow map and so stands, and the gizmos'
/// lines, on Q3 and finer, the eye near them and across X's seam from them.
#[test]
fn an_eye_moved_changes_no_draw() {
    let _g = sys::serial();
    const LIVE: usize = 16;
    let (at, _) = alloc(lists_bytes(LIVE)).unwrap();
    // SAFETY: a block of the heap's, the lists' alone for the test's run.
    unsafe { set_lists(at, LIVE) };
    let mut list = DrawList::default();
    let mut s = Scene::default();
    s.ghost.set(
        &mut list,
        &Ghost::Box(GhostBox {
            at: [8_388_607.5, -1234.25, 17.0],
            zoom: 20,
            cube: None,
            erase: false,
        }),
    );
    let lines = [0.0f32; 14];
    s.debug.set(
        &mut list,
        [-8_388_607.996, 4.0e6 + 1.0 / 3.0, -3.0],
        &lines,
        14,
    );
    s.coverage.set(&mut list, [12.0, -7.0, 2.0], &lines, 14);
    let models = models();
    let pose = |eye| {
        Some(ObserverPose {
            eye,
            rot: mat4::new(),
        })
    };
    let held = view().pose;
    view_compose(pose([8_388_600.0, -1200.0, 40.0]));
    s.draw(&mut list, models);
    let was = list.take();
    assert_eq!(
        DRAW_PIPELINES
            .map(|p| was.draws.of(p).len())
            .iter()
            .sum::<usize>(),
        3,
        "the box and both gizmos are drawn"
    );
    let stands = was.stands.to_vec();
    assert_eq!(stands.len(), 1, "the box stands");
    view_compose(pose([-8_388_600.0 + 0.5, 3.9e6, -2000.25]));
    s.draw(&mut list, models);
    let now = list.take();
    view_compose(held);
    assert!(now.changes.is_empty() && !now.reordered, "no draw changed");
    assert_eq!(now.stands, &stands[..]);
}
