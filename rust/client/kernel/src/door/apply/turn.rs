// pxl-kernel/door/apply/turn.rs

//! A turn's: what the runtime's frame pacing grants it and the network's
//! credits, which the client submits (TURN_GRANT), then the turn granted and
//! the clock (xpute's SYS_TURN).

use super::refuse_unless;
use crate::door::cmd as abi;
use crate::sys::sched::clock;
use crate::turn;
use crate::turn::TurnGrant;
use xpute_core::abi::sys::SysTurn;
use xpute_core::global::Global;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

/// What the turn rung next is granted, which its SYS_TURN takes.
static GRANT: Global<Option<abi::TurnGrant>> = Global::new();

pub(super) fn grant(c: abi::TurnGrant) -> Outcome {
    refuse_unless(c.refresh_ms >= 0.0 && c.paced_ms >= 0.0, Errno::EINVAL)?;
    GRANT.set(Some(c));
    Ok(Some(0))
}

/// A turn the runtime's policy grants, and the clock (SYS_TURN): refused
/// where nothing was submitted for it.
pub(in crate::door) fn turn(c: SysTurn) -> Outcome {
    refuse_unless(c.unix_ms.is_finite(), Errno::EINVAL)?;
    let Some(g) = GRANT.get(|| None).take() else {
        return refuse_unless(false, Errno::EINVAL).map(|()| None);
    };
    // The step is the one clock's change since the last frame: none on
    // the first, and none where the clock was set back.
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
