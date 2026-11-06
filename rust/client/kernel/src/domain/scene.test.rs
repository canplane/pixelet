// pxl-kernel/domain/scene.test.rs

use super::*;
use crate::domain::layer::object::preview::{Ghost, GhostBox};
use crate::domain::scene::model::models;
use crate::domain::view::frame::pose::{view, view_compose, ObserverPose};
use crate::sys;
use pxl_render::frame::draw::{lists_bytes, set_lists, DRAW_WORDS};
use pxl_spec::space::coord::mat4;
use xpute_guest::mem::heap::alloc;

/// A frame whose eye moved and whose world did not lists the same draws with
/// the same uniforms and the same stands, so the renderer writes none of them
/// (pxl-render gpu/read.rs `Written`) and the light's receivers read the same:
/// the ghost's box, which reads the shadow map and so stands, and the gizmos'
/// lines, on Q3 and finer, the eye near them and across X's seam from them.
#[test]
fn an_eye_moved_writes_no_draw() {
    let _g = sys::serial();
    const LIVE: usize = 16;
    let (at, _) = alloc(lists_bytes(LIVE)).unwrap();
    // SAFETY: a block of the heap's, the lists' alone for the test's run.
    unsafe { set_lists(at, LIVE) };
    let (mut was, mut now) = (DrawList::default(), DrawList::default());
    let mut s = Scene::default();
    s.ghost.set(
        &mut was,
        &Ghost::Box(GhostBox {
            at: [8_388_607.5, -1234.25, 17.0],
            zoom: 20,
            cube: None,
            erase: false,
        }),
    );
    let lines = [0.0f32; 14];
    s.debug.set(
        &mut was,
        [-8_388_607.996, 4.0e6 + 1.0 / 3.0, -3.0],
        &lines,
        14,
    );
    s.coverage.set(&mut was, [12.0, -7.0, 2.0], &lines, 14);
    let models = models();
    let pose = |eye| {
        Some(ObserverPose {
            eye,
            rot: mat4::new(),
        })
    };
    let held = view().pose;
    view_compose(pose([8_388_600.0, -1200.0, 40.0]));
    s.begin(&mut was, models);
    view_compose(pose([-8_388_600.0 + 0.5, 3.9e6, -2000.25]));
    s.begin(&mut now, models);
    view_compose(held);
    assert!(was.same_draws(&now));
    let lists = was.take();
    assert_eq!(
        lists.draws.len() / DRAW_WORDS,
        3,
        "the box and both gizmos are drawn"
    );
    assert_eq!(lists.stands.len(), 1, "the box stands");
}
