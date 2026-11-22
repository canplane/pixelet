// xpute-host/stop.test.rs

use std::sync::Mutex;
use std::time::Instant;

use xpute_golden::Golden;

use super::*;
use crate::abi::stop::Host as _;
use crate::runtime::{self, Runtime};

/// The stop the runtime was last handed.
static STOPPED: Mutex<Option<Stop>> = Mutex::new(None);

/// A stop that came across the link with a file and a message past the cap
/// is handed to the runtime with both cut as the record of the cut holds
/// them (kit/spec/golden/status/msg.tsv), and an empty message is none.
#[test]
fn stop_arrives_cut() {
    let g = Golden::load(include_str!("../../../kit/spec/golden/status/msg.tsv"));
    let text = |k: &str| String::from_utf8(g.bytes(k)).expect("UTF-8");
    let (long, cut) = (text("straddle.in"), text("straddle.out"));
    runtime::init(Runtime {
        post: |_| {},
        after: |_, _| (Instant::now(), 0),
        clear: |_| None,
        stopped: |s| *STOPPED.lock().unwrap() = Some(s),
        log: |_, _, _| {},
    });
    let told = |file: &str, msg: &str| {
        Report
            .stopped(-5, file.to_owned(), 7, 1, 2, Some(msg.to_owned()))
            .expect("the stop answered");
        STOPPED.lock().unwrap().take().expect("a stop")
    };
    let s = told(&long, &long);
    assert_eq!((s.errnum, s.line, s.n1, s.n2), (-5, 7, 1, 2));
    assert_eq!((s.file, s.msg), (cut.clone(), Some(cut)));
    assert_eq!(told("a/b.rs", "").msg, None);
}
