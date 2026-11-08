// pxl-render/gpu/split.test.rs

use super::*;

const ENCODER: Id = 1;
const ARGS: Id = 2;
const OTHER_ARGS: Id = 3;
const PLAIN: Id = 4;
const WRITES_ARGS: Id = 5;
const NEXT: Id = 6;
const PASS: Id = 10;

fn with_args() -> Split {
    let mut s = Split::default();
    s.made(ARGS, true, 1);
    s.made(OTHER_ARGS, true, 2);
    s.made(PLAIN, false, 3);
    s.grouped(WRITES_ARGS, [(16, ARGS), (8, PLAIN)].into_iter());
    s.begun(ENCODER);
    s
}

/// A pass that writes args, then a call that reads them: the call is cut
/// into a command buffer of its own, and the encoder is submitted as both
/// in order.
#[test]
fn a_call_reading_args_written_before_it_is_cut_apart() {
    let mut s = with_args();
    s.opened(PASS, ENCODER, true, Some((7, 1)));
    assert!(!s.cuts(PASS, ARGS), "nothing written yet");
    s.bound(PASS, 0, WRITES_ARGS, &[]);
    s.ran(PASS);
    assert!(s.cuts(PASS, ARGS), "written by its own pass");
    assert!(!s.cuts(PASS, OTHER_ARGS), "args not written cut nothing");
    s.split(ENCODER, NEXT);
    assert_eq!(s.at(ENCODER), NEXT);
    assert!(!s.cuts(PASS, ARGS), "written in the buffer before");
    s.closed(PASS);
    assert_eq!(s.submitted(ENCODER), vec![ENCODER, NEXT]);
}

/// A compute pass cut keeps what its next part is begun with.
#[test]
fn a_cut_pass_keeps_its_pipeline_groups_and_end() {
    let mut s = with_args();
    s.opened(PASS, ENCODER, true, Some((7, 3)));
    s.piped(PASS, 8);
    s.bound(PASS, 1, WRITES_ARGS, &[256]);
    assert_eq!(s.pass_on(PASS), Some((ENCODER, true)));
    let o = s.open(PASS).unwrap();
    assert_eq!((o.pipeline, o.end), (8, Some((7, 3))));
    assert_eq!(o.groups[1], (WRITES_ARGS, vec![256]));
    o.at = 11;
    assert_eq!(s.pass_at(PASS), 11, "its calls go to the part recorded now");
}

/// A copy into args is a write, a copy into anything else is not; a render
/// pass begins apart wherever args were written.
#[test]
fn a_copy_into_args_writes_them() {
    let mut s = with_args();
    s.copied(ENCODER, PLAIN);
    assert!(!s.draws_apart(ENCODER));
    s.copied(ENCODER, OTHER_ARGS);
    assert!(s.draws_apart(ENCODER));
    s.opened(PASS, ENCODER, true, None);
    assert!(s.cuts(PASS, OTHER_ARGS));
}

/// A handle let go and made again names what it is now.
#[test]
fn a_handle_made_again_is_what_it_is_now() {
    let mut s = with_args();
    s.made(ARGS, false, 4);
    assert!(!s.is_args(ARGS));
    s.grouped(WRITES_ARGS, [(8, PLAIN)].into_iter());
    s.opened(PASS, ENCODER, true, None);
    s.bound(PASS, 0, WRITES_ARGS, &[]);
    s.ran(PASS);
    assert!(!s.draws_apart(ENCODER));
}

/// An encoder never cut is submitted as itself, and one unknown too.
#[test]
fn an_encoder_never_cut_is_itself() {
    let mut s = with_args();
    assert_eq!(s.at(ENCODER), ENCODER);
    assert_eq!(s.submitted(ENCODER), vec![ENCODER]);
    assert_eq!(s.submitted(9), vec![9]);
}

/// A call reading its count from args a group set for it binds writable is named, by the
/// group's index and the binding; args another group writes, or a group let go, name none.
#[test]
fn args_bound_writable_where_a_call_reads_them_are_named() {
    let mut s = with_args();
    s.opened(PASS, ENCODER, true, None);
    assert_eq!(s.binds_writable(PASS, ARGS), None, "no group set");
    s.bound(PASS, 1, WRITES_ARGS, &[]);
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
    s.opened(PASS, ENCODER, false, None);
    s.bound(PASS, 0, WRITES_ARGS, &[]);
    s.bound(PASS, 0, 7, &[]);
    assert_eq!(
        s.binds_writable(PASS, ARGS),
        Some((WRITES_ARGS, 16)),
        "set before"
    );
    let mut s = with_args();
    s.opened(PASS, ENCODER, false, None);
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
    s.opened(PASS, ENCODER, false, None);
    assert_eq!(s.undeclared(PASS, 21), None, "none declared");
    s.declared(PASS, &[7]);
    assert_eq!(s.undeclared(PASS, 20), None);
    assert_eq!(s.undeclared(PASS, 21), Some(8));
    assert_eq!(s.undeclared(PASS, 22), Some(u32::MAX), "made of no record");
}
