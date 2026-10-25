// pxl-kernel/sys/io/dispatch.rs

//! What the request scheduler started and stopped this turn, handed to the
//! reader each is for: a baked stream's to `fetch`, an asset's to `loader`.

use xpute_kit::collection::list::List;

use crate::sys::io::param::MAX_CREDITS;
use crate::sys::io::request::{take_started, take_stops};
use crate::sys::io::{fetch, loader};

pub fn run() {
    // The key is an opaque handle to the shell.
    take_started(|credit, stream, subject| match stream.bake() {
        Some(_) => fetch::start(credit, stream, subject),
        None => {
            if let Some((_, load)) = loader::of_subject(subject) {
                loader::open_asset(credit, load);
            }
        }
    });
    // Collected first: stopping a read settles it, which must not happen inside
    // the request scheduler's own walk.
    let mut stops = List::<u64, { MAX_CREDITS as usize }>::new();
    take_stops(|key| stops.push(key));
    for key in stops {
        if !fetch::cancel(key) {
            loader::cancel_asset(key);
        }
    }
}
