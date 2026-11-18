// pxl-core/domain/scene.test.rs

use crate::domain::eye::{self, EyePose};
use crate::domain::preview::{Ghost, GhostBox};
use crate::domain::scene::gizmo::debug_line::DebugMesh;
use crate::domain::scene::indicator::ghost::GhostDraw;
use crate::domain::view::frame::pose::view_compose;
use pxl_render::gpu::draw::{DRAW_PIPELINES, DrawList, lists_sz, set_lists};
use pxl_spec::space::coord::mat4;

/// When only the eye moves, every draw keeps its uniforms and its receiver box
/// (pxl-render gpu/draw.rs `ReceiverBox`), so no draw changes (`Change`) and
/// the light sees the same receivers. The draws are the ghost's box, which
/// reads the shadow map and so has a receiver box, and the two gizmos' lines.
/// Their positions fall both on Q3 steps and between them. The eye moves from
/// near them to the far side of the seam where X wraps.
#[test]
fn an_eye_moved_changes_no_draw() {
    const LIVE: usize = 16;
    set_lists(Vec::leak(vec![0; lists_sz(LIVE) as usize / 4]), LIVE);
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
        Some(EyePose {
            eye,
            rot: mat4::new(),
        })
    };
    let before = eye::pose();
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
