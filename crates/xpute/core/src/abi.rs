// xpute-core/abi.rs

//! What two sides agree on by number, at the one layer that knows nothing of
//! either: the word and its bit fields, and how a command's number is
//! split. The numbers themselves are whoever's — `xpute_runtime::abi` holds
//! what a guest and its host agree on, `pxl_spec::abi` what one kernel and
//! one shell do.

pub mod cmd;
pub mod word;
