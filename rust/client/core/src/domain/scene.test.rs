// pxl-core/domain/scene.test.rs

use crate::domain::preview::{Ghost, GhostBox};
use crate::domain::scene::gizmo::debug_line::DebugMesh;
use crate::domain::scene::indicator::ghost::GhostDraw;
use crate::domain::view::frame::pose::{view, view_compose, ObserverPose};
use crate::sys;
use pxl_render::gpu::draw::{lists_bytes, set_lists, DrawList, DRAW_PIPELINES};
use pxl_spec::space::coord::mat4;
use xpute_core::mem::heap::alloc;

/// When only the eye moves, every draw keeps its uniforms and its receiver box
/// (pxl-render gpu/draw.rs `ReceiverBox`), so no draw changes (`Change`) and
/// the light sees the same receivers. The draws are the ghost's box, which
/// reads the shadow map and so has a receiver box, and the two gizmos' lines.
/// Their positions fall both on Q3 steps and between them. The eye moves from
/// near them to the far side of the seam where X wraps.
#[test]
fn an_eye_moved_changes_no_draw() {
    let _g = sys::serial();
    const LIVE: usize = 16;
    let (at, _) = alloc(lists_bytes(LIVE)).unwrap();
    // SAFETY: a block of the heap's, the lists' alone for the test's run.
    unsafe { set_lists(at, LIVE) };
    let mut list = DrawList::default();
    let (mut ghost, mut debug, mut coverage) = (
        GhostDraw::default(),
        DebugMesh::default(),
        DebugMesh::default(),
    );
    ghost.set(
        &mut list,
        &Ghost::Box(GhostBox {
            at: [8_388_607.5, -1234.25, 17.0],
            zoom: 20,
            cube: None,
            erase: false,
        }),
    );
    let lines = [0.0f32; 14];
    debug.set(
        &mut list,
        [-8_388_607.996, 4.0e6 + 1.0 / 3.0, -3.0],
        &lines,
        14,
    );
    coverage.set(&mut list, [12.0, -7.0, 2.0], &lines, 14);
    let draw = |list: &mut DrawList| {
        ghost.draw(list);
        debug.draw(list);
        coverage.draw(list);
    };
    let pose = |eye| {
        Some(ObserverPose {
            eye,
            rot: mat4::new(),
        })
    };
    let before = view().pose;
    view_compose(pose([8_388_600.0, -1200.0, 40.0]));
    draw(&mut list);
    let was = list.take();
    assert_eq!(
        DRAW_PIPELINES
            .map(|p| was.draws.of(p).len())
            .iter()
            .sum::<usize>(),
        3,
        "the box and both gizmos are drawn"
    );
    let receivers = was.receivers.to_vec();
    assert_eq!(receivers.len(), 1, "the box stands");
    view_compose(pose([-8_388_600.0 + 0.5, 3.9e6, -2000.25]));
    draw(&mut list);
    let now = list.take();
    view_compose(before);
    assert!(now.changes.is_empty() && !now.reordered, "no draw changed");
    assert_eq!(now.receivers, &receivers[..]);
}
