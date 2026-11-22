// xpute-kit/status/error.rs

use core::fmt;
use core::panic::Location;
use std::borrow::Cow;

use crate::status::errno::{Errno, Kind};
use crate::status::msg::cut_msg;

/// This is an errno, the place in the source where the error was made, its
/// file and its line, and what a person reads of it beyond the errno, where
/// its maker says more. It is returned, never fatal: a broken invariant is a
/// bug (`status/bug.rs`). What happened is the errno's `kind`. Across the
/// link it goes whole, so the error its caller holds is the one that was
/// made, with the place and the message of its maker's side.
#[derive(Debug)]
pub struct XputeError {
    pub errnum: Errno,
    /// The file of the place it was made: this program's own source, or
    /// what came across the link with an error another side made.
    pub file: Cow<'static, str>,
    pub line: u32,
    /// What a person reads beyond the errno, made once and never grown, so
    /// it holds no capacity; a message past the cap is cut as it is made
    /// (`cut_msg`), and an empty one is none.
    pub msg: Option<Box<str>>,
}

impl XputeError {
    #[track_caller]
    pub fn new(errnum: Errno) -> XputeError {
        let at = Location::caller();
        XputeError {
            errnum,
            file: Cow::Borrowed(cut_msg(at.file())),
            line: at.line(),
            msg: None,
        }
    }

    /// The error `errnum`, with what a person reads of it.
    #[track_caller]
    pub fn with_msg(errnum: Errno, msg: impl Into<Box<str>>) -> XputeError {
        XputeError {
            msg: bounded(msg.into()),
            ..XputeError::new(errnum)
        }
    }

    /// The error `errnum` made at `line` of `file` with the message `msg`:
    /// one another side made, as it came across the link. Its file and its
    /// message are cut as any error's are, whatever that side sent.
    pub fn at(errnum: Errno, mut file: String, line: u32, msg: Option<String>) -> XputeError {
        file.truncate(cut_msg(&file).len());
        XputeError {
            errnum,
            file: Cow::Owned(file),
            line,
            msg: msg.and_then(|msg| bounded(msg.into())),
        }
    }

    pub const fn kind(&self) -> Kind {
        self.errnum.kind()
    }
}

/// `msg` as an error holds it: cut as far as the cap holds it, and none where
/// it is empty.
fn bounded(msg: Box<str>) -> Option<Box<str>> {
    let cut = cut_msg(&msg);
    match cut.len() {
        0 => None,
        len if len == msg.len() => Some(msg),
        _ => Some(cut.into()),
    }
}

/// The error `errnum`, made where the errno becomes an error, as `?` makes
/// one of an errno a callee returned.
impl From<Errno> for XputeError {
    #[track_caller]
    fn from(errnum: Errno) -> XputeError {
        XputeError::new(errnum)
    }
}

impl fmt::Display for XputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at {}:{}", self.errnum, self.file, self.line)?;
        match &self.msg {
            Some(m) => write!(f, ": {m}"),
            None => Ok(()),
        }
    }
}

impl std::error::Error for XputeError {}

/// What a person reads of the errno `errnum`: its name and its text, or its
/// number where no errno is it, as @xpute/kit `strerror` writes it.
pub fn strerror(errnum: i32) -> String {
    Errno::of(errnum).map_or_else(
        || format!("errno {errnum}"),
        |e| format!("{}: {}", e.name(), e.text()),
    )
}

#[cfg(test)]
#[path = "error.test.rs"]
mod test;
