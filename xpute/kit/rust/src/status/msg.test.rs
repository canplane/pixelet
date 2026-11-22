// xpute-kit/status/msg.test.rs

//! Holds `cut_msg` to kit/spec/golden/status/msg.tsv, the record every
//! language's must match, and writes it: run with `XPUTE_GOLDEN_WRITE=1`,
//! `cut_matches_record` writes the record instead of comparing.

use std::fmt::Write as _;

use super::*;
use xpute_golden::Golden;

const GOLDEN: &str = include_str!("../../../spec/golden/status/msg.tsv");

/// The record's messages, each past the cap by a character that straddles
/// it: one of four bytes, and the three bytes a lone surrogate is as UTF-8,
/// as TypeScript's side encodes one.
fn msgs() -> [(&'static str, String); 2] {
    let lead = "x".repeat(MSG_CAP as usize - 2);
    [
        ("straddle", format!("{lead}\u{1f600}tail")),
        ("surrogate", format!("{lead}\u{fffd}tail")),
    ]
}

/// A message within the cap is whole, and one past it is cut before the
/// character the cap falls in.
#[test]
fn cut_msg_cuts_at_char_boundary() {
    let cap = MSG_CAP as usize;
    assert_eq!(cut_msg(""), "");
    let whole = "é".repeat(cap / 2);
    assert_eq!(cut_msg(&whole), whole);
    let long = format!("x{whole}");
    assert_eq!(cut_msg(&long), &long[..cap - 1]);
}

/// Each recorded message is cut as the record holds it; with
/// `XPUTE_GOLDEN_WRITE=1`, the record is written instead.
#[test]
fn cut_matches_record() {
    if std::env::var_os("XPUTE_GOLDEN_WRITE").is_some() {
        return write();
    }
    let g = Golden::load(GOLDEN);
    for (k, msg) in msgs() {
        assert_eq!(
            hex::encode(&msg),
            hex::encode(g.bytes(&format!("{k}.in"))),
            "{k}"
        );
        let want = g.bytes(&format!("{k}.out"));
        assert_eq!(hex::encode(cut_msg(&msg)), hex::encode(want), "{k}");
    }
}

/// Writes the record: each message, and what `cut_msg` leaves of it.
fn write() {
    let mut out = String::new();
    for (k, msg) in msgs() {
        let _ = writeln!(out, "{k}.in\t{}", hex::encode(&msg));
        let _ = writeln!(out, "{k}.out\t{}", hex::encode(cut_msg(&msg)));
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../spec/golden/status/msg.tsv");
    std::fs::write(path, out).expect("the record written");
}
