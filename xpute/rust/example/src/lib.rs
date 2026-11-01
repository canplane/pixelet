// xpute-example/lib.rs

//! The smallest guest, read beside the host that drives it
//! (ts/example/sum.test.ts). It answers one command, `SUM`, with the sum of
//! its two numbers and one the host holds, which it asks for by a hostcall
//! answered now.

#![cfg(target_arch = "wasm32")]

use xpute_guest::program::{Packet, Turn};
use xpute_kit::status::errno::Errno;

/// The command, the program's own: two numbers, answered with their sum and
/// the host's.
pub const SUM: u32 = 0x1001;
/// The hostcall: the number the host adds.
pub const ASK: u32 = 1;

xpute_guest::program! {
    on SUM => sum,
}

fn sum(cx: &mut Turn<'_>, pkt: Packet<'_>) -> Result<i32, Errno> {
    let (a, b) = (pkt.number(0)?, pkt.number(1)?);
    let held = cx.call_now(ASK, &[])?;
    Ok((a + b) as i32 + held as i32)
}
