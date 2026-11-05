// xpute-example/lib.rs

//! The smallest guest, read beside the host that drives it
//! (ts/example/sum.test.ts). Its machine (machine.json) declares its memory
//! and its rings, which the host lays out as the machine places them; its one
//! protocol (asker.json) has the host send SUM and the guest call ASK, and
//! both ends' code is generated from it. It answers SUM with the sum of its two numbers and
//! the host's answer to ASK, answered before the call returns.

#![cfg(target_arch = "wasm32")]

#[path = "asker.spec.rs"]
mod abi;
#[path = "../machine.spec.rs"]
mod machine;

use abi::asker;
use xpute_guest::entry::BootInfo;
use xpute_guest::ipc::door::{Outcome, Submission};
use xpute_guest::sched::wake::NO_WAKE;
use xpute_kit::status::error::MarshalError;

// No fixed ranges: the heap starts on the page past the data.
xpute_guest::guest!(machine::MACHINE, 0, entry);

struct Example;

impl asker::Exports for Example {
    fn sum(&mut self, c: abi::Sum) -> Outcome {
        let answer = asker::ask().map_err(MarshalError::new)?;
        Ok(Some((c.a + c.b) as i32 + answer as i32))
    }
}

/// A turn: what waits applied in order, then the next asked for while any is left.
fn entry(_quota_ms: f64, _boot: fn() -> BootInfo) -> f64 {
    let door = xpute_guest::machine::door();
    door.rise();
    door.receive(|submission| match submission {
        // It keeps nothing across its life, and draws nothing.
        Submission::Life(_) | Submission::GpuReady => Ok(Some(0)),
        // It opens no file.
        Submission::Opened(_) | Submission::Closed(_) => Ok(None),
        Submission::Program(cmd, pkt) => asker::apply(&mut Example, cmd, &pkt),
    });
    if door.pending() {
        0.0
    } else {
        NO_WAKE
    }
}
