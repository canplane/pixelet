// xpute-guest/wait.rs

//! A sent method as a future, on the guest's side (xpute-link wait.rs), for
//! the generated bindings' calls.

use xpute_link::wait::{wait_by, Wait};
use xpute_link::{Args, Handle, Method};

pub fn wait<L: 'static>(h: Handle, m: Method, args: Args<'_>, lent: L) -> Wait<L> {
    wait_by(crate::link::send, crate::link::cancel, h, m, args, lent)
}
