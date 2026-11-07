// xpute-example/lib.rs

//! The smallest core, read beside the runtime that drives it
//! (ts/example/sum.test.ts). Its machine (machine.json) declares its memory
//! and its rings, which the runtime lays out as the machine places them; its one
//! protocol (asker.json) has the runtime send SUM and the core call ASK, and
//! both ends' code is generated from it. It answers SUM with the sum of its two numbers and
//! the runtime's answer to ASK, answered before the call returns.

#![cfg(target_arch = "wasm32")]

#[path = "asker.spec.rs"]
mod abi;
#[path = "../machine.spec.rs"]
mod machine;

use abi::asker;
use xpute_core::entry::BootInfo;
use xpute_core::ipc::door::{Outcome, Submission};
use xpute_kit::status::error::MarshalError;

// No fixed ranges: the heap starts on the page past the data.
xpute_core::core!(machine::MACHINE, 0, entry);

struct Example;

impl asker::Exports for Example {
    fn sum(&mut self, c: abi::Sum) -> Outcome {
        let answer = asker::ask().map_err(MarshalError::new)?;
        Ok(Some((c.a + c.b) as i32 + answer as i32))
    }
}

/// A turn: its own commands answered, xpute's taken as they come.
fn entry(_deadline_ms: f64, _boot: fn() -> BootInfo) -> f64 {
    xpute_core::machine::door().turn(|s| match s {
        Submission::Program(cmd, pkt) => asker::apply(&mut Example, cmd, &pkt),
        _ => Ok(Some(0)),
    })
}
