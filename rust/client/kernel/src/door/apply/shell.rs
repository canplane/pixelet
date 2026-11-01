// pxl-kernel/door/apply/shell.rs

//! SHELL: what the kernel shows the shell: posted, never taken.

use super::State;
use crate::domain::control::cmd::Command;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    _s: &mut State,
    cmd: Command,
    _pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::SHELL_SNAPSHOT => Ok(None),
        _ => Ok(None),
    }
}
