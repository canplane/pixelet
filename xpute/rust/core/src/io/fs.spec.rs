// xpute-core/io/fs.spec.rs
//
// GENERATED from spec/io/fs.json
//
// The numbers of the file device, whose protocol is abi/fs.json on major FS.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The flags FS_OPEN takes: whether the file is opened to be read whole or written whole.
/// Any other value completes the open with EINVAL.
pub mod flag {
    /// Opens the file to be read whole.
    pub const O_RDONLY: u32 = 0;
    /// Opens the file to be written whole. The file is created if it does not exist, and
    /// replaced at FS_CLOSE by what was written, as O_CREAT | O_TRUNC would do.
    pub const O_WRONLY: u32 = 1;
    pub const KEYS: &[&str] = &["O_RDONLY", "O_WRONLY"];
}
