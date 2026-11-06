// pxl-kernel/domain/scene/region.rs

//! Whether anything the scene animates stepped this turn, waking the turn at
//! its own steps (`animated`), which the renderer reads so the resolve asks
//! no frame after it (pxl-render `FrameInput::animated`).

use xpute_guest::global::Global;

#[derive(Default)]
struct Region {
    animated: bool,
}

static REGION: Global<Region> = Global::new();

fn region() -> &'static mut Region {
    REGION.get(Region::default)
}

/// What the scene animates stepped this turn: what wakes the turn at its
/// own steps, so no frame is asked after it.
pub fn note_animation() {
    region().animated = true;
}

/// Whether what is animated stepped; the turn's taken.
pub fn take() -> bool {
    core::mem::take(&mut region().animated)
}
