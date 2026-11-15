// xpute-native-host/log.rs

//! The log the host serves the guest (wit/log/log.wit), and its own: each
//! record, a level, a context and a message, is handed to its runtime
//! (runtime.rs `log`), which writes it, as log.ts hands a page's.

use std::rc::Rc;

use xpute_kit::status::errno::Errno;
use xpute_link::Side;

use crate::abi::log::{self, Level};
use crate::runtime::runtime;

/// Hands its runtime a record of the host's, or of what it serves:
/// `message`, at `level`, about `context`.
pub fn log(level: Level, context: &str, message: &str) {
    (runtime().log)(level, context, message);
}

struct Log;

impl log::Host for Log {
    fn log(&self, level: Level, context: String, message: String) -> Result<(), Errno> {
        self::log(level, &context, &message);
        Ok(())
    }
}

/// Mounts the log on `side`.
pub fn serve(side: &'static Side) {
    log::mount(side, Rc::new(Log));
}
