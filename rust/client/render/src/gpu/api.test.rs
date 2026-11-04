// pxl-render/gpu/api.test.rs

use super::*;

#[test]
fn a_pass_clears_to_transparent_unless_given() {
    let d = pass_desc(1, Load::Clear, 0, Load::Clear, 0.0, None);
    assert_eq!(d.color_clear, [0.0; 4]);
    let rgba = [0.5, 0.25, 0.125, 1.0];
    let d = d.color_clear(rgba).load_stencil();
    assert_eq!(d.color_clear, rgba, "the stencil's load leaves it");
}

#[test]
fn state_holds_what_was_set() {
    let mut s = DrawState::default();
    assert!(!s.is_pipeline_set(7), "a pass begins holding no pipeline");
    s.set_pipeline(7);
    assert!(s.is_pipeline_set(7));
    assert!(!s.is_pipeline_set(8), "another pipeline is set again");

    s.set_bind_group(0, 3, &[]);
    assert!(s.is_bind_group_set(0, 3, &[]));
    assert!(
        !s.is_bind_group_set(1, 3, &[]),
        "a group is held at its own index"
    );
    assert!(!s.is_bind_group_set(0, 4, &[]));
    // A pipeline set over it leaves the groups as they were, as WebGPU does.
    s.set_pipeline(8);
    assert!(s.is_bind_group_set(0, 3, &[]));
}

#[test]
fn state_never_holds_a_group_with_offsets() {
    // Its offsets move per draw (decor's `uniform_at`): set once, it is set
    // at every draw after.
    let mut s = DrawState::default();
    s.set_bind_group(1, 5, &[256]);
    assert!(!s.is_bind_group_set(1, 5, &[256]));
    assert!(!s.is_bind_group_set(1, 5, &[512]));
    assert!(
        !s.is_bind_group_set(1, 5, &[]),
        "nor the same group without them"
    );
    s.set_bind_group(1, 5, &[]);
    assert!(s.is_bind_group_set(1, 5, &[]));
    s.set_bind_group(1, 5, &[0]);
    assert!(
        !s.is_bind_group_set(1, 5, &[]),
        "offsets let go of what was held"
    );
}

#[test]
fn state_holds_a_buffer_by_its_range() {
    let mut s = DrawState::default();
    s.set_vertex_buf(0, 9, 0, 64);
    assert!(s.is_vertex_buf_set(0, 9, 0, 64));
    assert!(
        !s.is_vertex_buf_set(1, 9, 0, 64),
        "a lane is held at its own slot"
    );
    assert!(
        !s.is_vertex_buf_set(0, 9, 64, 64),
        "another range of the same buffer"
    );
    assert!(!s.is_vertex_buf_set(0, 9, 0, 0));
    s.set_idx_buf(9, 128, 32);
    assert!(s.is_idx_buf_set(9, 128, 32));
    assert!(!s.is_idx_buf_set(9, 0, 32));
}
