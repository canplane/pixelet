// pxl-core/domain/scene/animation.rs

//! What an animation of the scene shares: a character, the clouds, the
//! location's beacon. Each moves on a clock that steps a seen step at a time
//! (`lens::StepClock`), and reads it here (`read`), which wakes the frame at
//! its next step and records that it stepped. The renderer reads that flag
//! (pxl-render `FrameInput::animated`) so that a frame drawn for an
//! animation's step asks for no frames after it: the animation's next step
//! wakes the frame again.

use crate::domain::view::lens::StepClock;
use crate::spec::step::InvalidationReason;
use crate::sys::sched::invalidate_at;
use pxl_global::Global;

#[derive(Default)]
struct Animation {
    animated: bool,
}

static ANIMATION: Global<Animation> = Global::new();

fn animation() -> &'static mut Animation {
    ANIMATION.get(Animation::default)
}

/// Records that an animation stepped this frame. Only an animation that
/// brings its own next frame calls this, by its clock's wake (`read`) or by
/// asking for the frame, since the renderer then asks for no frame after
/// this one.
pub(crate) fn note_animation() {
    animation().animated = true;
}

/// Reads an animation's `clock` with its source at `t`, as `StepClock::at`
/// does, `next` giving the reading it next steps at. The frame is woken at
/// that step, `ms` milliseconds a unit of `t` after `now_ms` on the monotonic
/// clock, and a step now is recorded (`note_animation`). Returns what the
/// clock shows and whether it stepped.
pub(crate) fn read(
    clock: &mut StepClock,
    now_ms: f64,
    t: f64,
    ms: f64,
    next: impl Fn(f64) -> f64,
) -> (f64, bool) {
    let (time, due, stepped) = clock.at(t, next);
    if due.is_finite() {
        invalidate_at(now_ms + (due - t) * ms, InvalidationReason::Animation);
    }
    if stepped {
        note_animation();
    }
    (time, stepped)
}

/// Whether an animation stepped this frame. Reading it clears it, so the stage
/// reads it once a frame.
pub(crate) fn take() -> bool {
    core::mem::take(&mut animation().animated)
}
