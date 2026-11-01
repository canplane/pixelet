// xpute-example/lib.rs

//! The smallest guest, read beside the host that drives it
//! (ts/example/sum.test.ts). It answers `SUM` with the sum of its two numbers
//! and one the host holds, which it asks for by a hostcall answered now, and
//! raises `SUMMED` as it does; and `COUNT` with a task that reads a file whole
//! and raises `COUNTED` with its size a few turns later. Its commands are its
//! table's (spec/example/cmd.json), generated for both ends.

#![cfg(target_arch = "wasm32")]

#[path = "cmd.spec.rs"]
mod cmd;

use cmd::{Command, Count, CountedEvent, Hostcall, Root, Sum, SummedEvent};
use xpute_guest::program::{raise, read, Turn};
use xpute_kit::status::errno::Errno;

xpute_guest::program! {
    on Command::SUM => sum,
    on Command::COUNT => count,
}

fn sum(cx: &mut Turn<'_>, p: Sum) -> Result<i32, Errno> {
    let total = p.a + p.b;
    cx.raise(&SummedEvent { total });
    let held = cx.call_now(Hostcall::ASK as u32, &[])?;
    Ok((total + held) as i32)
}

fn count(cx: &mut Turn<'_>, p: Count<'_>) -> Result<i32, Errno> {
    let path = String::from(p.path);
    cx.spawn(async move {
        let res = match read(Root::FILES as u32, &path).await {
            Ok(bytes) => bytes.len() as i32,
            Err(e) => e as i32,
        };
        raise(&CountedEvent { res });
    })?;
    Ok(0)
}
