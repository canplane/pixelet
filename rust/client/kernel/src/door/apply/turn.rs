// pxl-kernel/door/apply/turn.rs

//! TURN: a turn's: what the runtime grants it, the network's credits and the
//! clock.

use super::refuse_unless;
use crate::domain::control::cmd as abi;
use crate::sys::sched::clock;
use crate::turn;
use crate::turn::RuntimeFrame;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(super) fn grant(c: abi::TurnGrant) -> Outcome {
    refuse_unless(
        c.refresh_ms >= 0.0 && c.paced_ms >= 0.0 && c.unix_ms.is_finite(),
        Errno::EINVAL,
    )?;
    // The step is the one clock's change since the last frame: none on
    // the first, and none where the clock was set back.
    let last = clock::wall_now();
    let delta = if last > 0.0 {
        ((c.unix_ms - last) / 1000.0).max(0.0)
    } else {
        0.0
    };
    turn::on_world_frame_asked(RuntimeFrame {
        delta,
        refresh_ms: c.refresh_ms,
        paced_ms: c.paced_ms,
        interactive: c.interactive,
        credits: c.credits,
    });
    clock::set_wall_now(c.unix_ms);
    Ok(Some(0))
}
