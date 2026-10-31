// pxl-kernel/domain/scene/region.rs

//! What the scene animates, as each turn draws it: the keys of the draws it
//! bounds among those whose moves would otherwise draw the frame whole
//! (`drawn`), which the renderer lets move and marks, each that changed drawn
//! where it was and where it is (pxl-render `FrameInput::bounded`,
//! gpu/pass/mark.rs). Whether anything it animates stepped this turn, waking
//! the turn at its own steps, is marked beside (`animated`).

use xpute_guest::global::Global;

#[derive(Default)]
struct Bounded {
    /// The turn's keys as they are drawn, and those `take` last handed,
    /// which the renderer reads.
    now: Vec<u32>,
    handed: Vec<u32>,
    animated: bool,
}

static BOUNDED: Global<Bounded> = Global::new();

fn bounded() -> &'static mut Bounded {
    BOUNDED.get(Bounded::default)
}

/// A draw this turn the scene bounds, `key` the handle its record names.
pub fn drawn(key: u32) {
    bounded().now.push(key);
}

/// What the scene animates stepped this turn: what wakes the turn at its
/// own steps, so no frame is asked after it.
pub fn animated() {
    bounded().animated = true;
}

/// The keys of the turn's bounded draws (pxl-render `FrameInput::bounded`),
/// and whether what is animated stepped; the turn's taken.
pub fn take() -> (&'static [u32], bool) {
    let b = bounded();
    let animated = core::mem::take(&mut b.animated);
    core::mem::swap(&mut b.handed, &mut b.now);
    b.now.clear();
    let b: &'static Bounded = b;
    (&b.handed, animated)
}
