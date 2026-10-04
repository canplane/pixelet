// xpute-runtime/abi/cmd.test.rs
//
// GENERATED from spec/xpute/abi/cmd.json — do not edit.

use super::*;
use xpute_core::abi::cmd::cmd;

#[test]
fn every_number_is_its_major_and_minor() {
    use Command::*;
    let parts = [(SYS_OPENED, Major::SYS as u32, SysMinor::OPENED as u32)];
    for (c, major, minor) in parts {
        assert_eq!(c as u32, cmd(major, minor), "{c:?}");
        assert_eq!(Command::of(c as u32), Some(c));
    }
    assert_eq!(Command::of(0x0fff), None);
}
