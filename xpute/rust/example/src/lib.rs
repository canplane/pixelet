// xpute-example/lib.rs

//! The smallest guest, read beside the host that drives it
//! (ts/example/sum.test.ts). It answers one command, `SUM`, with the sum of
//! its two numbers and one the host holds, which it asks for by a hostcall
//! answered now, and raises `SUMMED` as it does. Its commands are its table's
//! (spec/example/cmd.json), generated for both ends.

#![cfg(target_arch = "wasm32")]

#[path = "cmd.spec.rs"]
mod cmd;

use cmd::{Command, Sum, SummedEvent};
use xpute_guest::program::Turn;
use xpute_kit::status::errno::Errno;

/// The hostcall: the number the host adds.
pub const ASK: u32 = 1;

xpute_guest::program! {
    on Command::SUM => sum,
}

fn sum(cx: &mut Turn<'_>, p: Sum) -> Result<i32, Errno> {
    let total = p.a + p.b;
    cx.raise(&SummedEvent { total });
    let held = cx.call_now(ASK, &[])?;
    Ok((total + held) as i32)
}
