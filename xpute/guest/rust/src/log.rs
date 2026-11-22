// xpute-guest/log.rs

//! The guest's log records, told to its host by the log it implements
//! (wit/log/log.wit), as cpp/xpute/guest/log.hpp is a C++ guest's: a level, a
//! context and a message, each record worth telling and neither an error nor
//! a bug.

use xpute_kit::status::msg::cut_msg;

use crate::abi::log::{self, Level};

/// Tells the host `msg`, at `level`, about `context`, each cut as far as a
/// message's cap holds it (`cut_msg`). A host that hears
/// nothing more, the link trapped among them, has nothing to answer, so the
/// answer is not read.
pub fn log(level: Level, context: &str, msg: &str) {
    let _ = log::log(level, cut_msg(context), cut_msg(msg));
}

#[cfg(test)]
#[path = "log.test.rs"]
mod test;
