// xpute-guest/lib.rs

//! The guest's end of xpute: its end of the link (xpute-link) to its host,
//! and the clocks it reads.

/// The bindings of xpute's packages on the guest's side, a module to a
/// package, generated from their WIT (xpute-gen-targets).
pub mod abi;
// What `link!` expands to names the link's own items through this crate.
#[doc(hidden)]
pub use xpute_link;
pub mod clock;
pub mod link;
pub mod wait;
