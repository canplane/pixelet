// xpute-example/lib.rs

//! The smallest core, meant to be read beside the runtime that drives it
//! (ts/example/sum.test.ts). Its machine (machine.json) declares its memory and
//! its rings, which the runtime lays out where the machine places them. Its one
//! protocol (asker.json) has the runtime send SUM and the core call ASK, and
//! the code of both ends is generated from it. The core answers SUM with the
//! sum of its two numbers and the runtime's answer to ASK, which the runtime
//! gives before the call returns.

#![cfg(target_arch = "wasm32")]

#[path = "asker.spec.rs"]
mod abi;
#[path = "machine.spec.rs"]
mod machine;

use abi::asker;
use xpute_core::entry::BootInfo;
use xpute_core::ipc::door::{Outcome, Submission};
use xpute_kit::status::error::MarshalError;

// The core has no fixed ranges, so its heap starts on the page past the data.
xpute_core::core!(machine::MACHINE, 0, entry);

struct Example;

impl asker::Exports for Example {
    fn sum(&mut self, c: abi::Sum) -> Outcome {
        let answer = asker::ask().map_err(MarshalError::new)?;
        Ok(Some((c.a + c.b) as i32 + answer as i32))
    }
}

/// A turn answers the core's own commands, and answers each of xpute's with 0.
fn entry(_deadline_ms: f64, _boot: fn() -> BootInfo) -> f64 {
    xpute_core::machine::door().turn(|s| match s {
        Submission::Program(cmd, pkt) => asker::apply(&mut Example, cmd, &pkt),
        _ => Ok(Some(0)),
    })
}
