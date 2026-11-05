// pxl-render/gpu/split.test.rs

use super::*;

const ENCODER: Id = 1;
const ARGS: Id = 2;
const OTHER_ARGS: Id = 3;
const PLAIN: Id = 4;
const WRITES_ARGS: Id = 5;
const NEXT: Id = 6;
const PASS: Id = 10;

fn with_args() -> Splits {
    let mut s = Splits::default();
    s.made(ARGS, true);
    s.made(OTHER_ARGS, true);
    s.made(PLAIN, false);
    s.grouped(WRITES_ARGS, [ARGS, PLAIN].into_iter());
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
    s.made(ARGS, false);
    assert!(!s.is_args(ARGS));
    s.grouped(WRITES_ARGS, [PLAIN].into_iter());
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
