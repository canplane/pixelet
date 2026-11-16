// pxl-render/gpu/track.test.rs

use super::*;

const ARGS: Id = 2;
const OTHER_ARGS: Id = 3;
const PLAIN: Id = 4;
const WRITES_ARGS: Id = 5;
const PASS: Id = 10;

fn with_args() -> Tracker {
    let mut s = Tracker::default();
    s.note_buffer(ARGS, true, 1);
    s.note_buffer(OTHER_ARGS, true, 2);
    s.note_buffer(PLAIN, false, 3);
    s.grouped(WRITES_ARGS, [(16, ARGS), (8, PLAIN)].into_iter());
    s
}

/// A handle let go and made again names what it is now.
#[test]
fn a_handle_made_again_is_what_it_is_now() {
    let mut s = with_args();
    s.note_buffer(ARGS, false, 4);
    assert!(!s.is_args(ARGS));
    s.grouped(WRITES_ARGS, [(8, PLAIN)].into_iter());
    s.opened(PASS, true);
    s.bound(PASS, 0, WRITES_ARGS);
    assert_eq!(s.binds_writable(PASS, ARGS), None);
}

/// A call reading its count from args a group set for it binds writable is named, by the
/// group's index and the binding; args another group writes, or a group let go, name none.
#[test]
fn args_bound_writable_where_a_call_reads_them_are_named() {
    let mut s = with_args();
    s.opened(PASS, true);
    assert_eq!(s.binds_writable(PASS, ARGS), None, "no group set");
    s.bound(PASS, 1, WRITES_ARGS);
    assert_eq!(s.binds_writable(PASS, ARGS), Some((1, 16)));
    assert_eq!(s.made_at(ARGS), 1, "named by the line that made it");
    assert_eq!(s.binds_writable(PASS, OTHER_ARGS), None, "another's args");
    s.let_go(WRITES_ARGS);
    assert_eq!(s.binds_writable(PASS, ARGS), None, "the group let go");
}

/// A render pass is one usage scope whole: a group set before an indirect draw, or after it,
/// that binds the draw's args writable is named, the group by its handle.
#[test]
fn a_render_pass_names_args_written_anywhere_in_it() {
    let mut s = with_args();
    s.opened(PASS, false);
    s.bound(PASS, 0, WRITES_ARGS);
    s.bound(PASS, 0, 7);
    assert_eq!(
        s.binds_writable(PASS, ARGS),
        Some((WRITES_ARGS, 16)),
        "set before"
    );
    let mut s = with_args();
    s.opened(PASS, false);
    assert_eq!(s.binds_writable(PASS, ARGS), None);
    assert_eq!(
        s.binds_read(PASS, WRITES_ARGS),
        Some((ARGS, 16)),
        "set after"
    );
    assert_eq!(s.binds_read(PASS, 7), None);
}

/// A render pass that declares what it draws names a pipeline set in it of no record it
/// declares, or of none; a pass that declares none names none.
#[test]
fn a_declared_render_pass_names_a_pipeline_it_does_not_declare() {
    let mut s = with_args();
    s.recorded(20, 7);
    s.recorded(21, 8);
    s.opened(PASS, false);
    assert_eq!(s.undeclared(PASS, 21), None, "none declared");
    s.declared(PASS, &[7]);
    assert_eq!(s.undeclared(PASS, 20), None);
    assert_eq!(s.undeclared(PASS, 21), Some(8));
    assert_eq!(s.undeclared(PASS, 22), Some(u32::MAX), "made of no record");
}
