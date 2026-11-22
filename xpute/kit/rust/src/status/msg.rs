// xpute-kit/status/msg.rs

//! A message (`kit/spec/msg.json5`): what a person reads of an error, a bug
//! or a log record, bounded text, which its maker cuts.

#[path = "msg.spec.rs"]
pub mod spec;
pub use spec::MSG_CAP;

/// `msg` as far as the cap holds it, cut at a character boundary. It is a
/// part of `msg` itself, so cutting one allocates nothing, a bug's static
/// message among them.
pub fn cut_msg(msg: &str) -> &str {
    let mut len = msg.len().min(MSG_CAP as usize);
    while !msg.is_char_boundary(len) {
        len -= 1;
    }
    &msg[..len]
}

#[cfg(test)]
#[path = "msg.test.rs"]
mod test;
