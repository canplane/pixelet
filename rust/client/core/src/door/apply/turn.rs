// pxl-core/door/apply/turn.rs

//! A turn is granted in two steps. The client first submits what the
//! runtime's frame pacing grants and the network's credits (TURN_GRANT).
//! Then xpute's SYS_TURN rings the granted turn and gives the clock.

use super::refuse_unless;
use crate::door::cmd as abi;
use crate::sys::sched::clock;
use crate::turn;
use crate::turn::TurnGrant;
use xpute_core::abi::sys::SysTurn;
use xpute_core::global::Global;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

/// The grant for the next turn rung, which that turn's SYS_TURN takes.
static GRANT: Global<Option<abi::TurnGrant>> = Global::new();

pub(super) fn grant(c: abi::TurnGrant) -> Outcome {
    refuse_unless(c.refresh_ms >= 0.0 && c.paced_ms >= 0.0, Errno::EINVAL)?;
    GRANT.set(Some(c));
    Ok(Some(0))
}

/// A turn the runtime's policy grants, with the clock (SYS_TURN). It is
/// refused when no TURN_GRANT was submitted for it.
pub(in crate::door) fn turn(c: SysTurn) -> Outcome {
    refuse_unless(c.unix_ms.is_finite(), Errno::EINVAL)?;
    let Some(g) = GRANT.get(|| None).take() else {
        return refuse_unless(false, Errno::EINVAL).map(|()| None);
    };
    // The step is how far the wall clock moved since the last frame, in
    // seconds. It is 0 on the first frame and when the clock was set back.
    let last = clock::wall_now();
    let delta = if last > 0.0 {
        ((c.unix_ms - last) / 1000.0).max(0.0)
    } else {
        0.0
    };
    turn::on_world_frame_asked(TurnGrant {
        delta,
        refresh_ms: g.refresh_ms,
        paced_ms: g.paced_ms,
        interactive: g.interactive,
        credits: g.credits,
    });
    clock::set_wall_now(c.unix_ms);
    Ok(Some(0))
}
