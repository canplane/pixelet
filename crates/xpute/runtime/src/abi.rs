// xpute-runtime/abi.rs

//! What the guest and its host agree on by number: why a fetch the host
//! ran did not deliver, generated from `spec/xpute/abi/` into both
//! languages; and a handle to what the host keeps — the table that issues
//! one here, the slot read off it there, and the split between them a
//! number both sides write.

#[path = "abi/fetch.spec.rs"]
pub mod fetch;
pub mod handle;
