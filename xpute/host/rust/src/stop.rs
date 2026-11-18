// xpute-host/stop.rs

//! The stop the host implements for the guest (wit/stop/stop.wit): the guest's
//! report of a broken invariant, handed to the host's runtime (runtime.rs
//! `stopped`), which reports it, before the guest stops (stop.ts). The host's
//! own failures are its runtime's to tell, as a stop and a trap, and so is a
//! guest's trap, as the web's stop and trap are two messages.

use std::rc::Rc;

use xpute_kit::status::errno::Errno;
use xpute_link::Side;

use crate::abi;
use crate::runtime::{Stop, runtime};

struct Report;

impl abi::stop::Host for Report {
    fn stopped(
        &self,
        errnum: i32,
        file: String,
        line: u32,
        n1: u64,
        n2: u64,
        msg: Option<String>,
    ) -> Result<(), Errno> {
        (runtime().stopped)(Stop {
            errnum,
            file,
            line,
            n1,
            n2,
            msg,
        });
        Ok(())
    }
}

/// Mounts the stop on `side`.
pub fn mount(side: &'static Side) {
    abi::stop::mount(side, Rc::new(Report));
}
