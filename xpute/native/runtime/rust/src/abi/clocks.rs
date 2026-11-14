// xpute-native-runtime/abi/clocks.rs
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

pub mod monotonic_clock;
pub mod system_clock;
pub mod types;

/// The methods of the named interfaces the host serves that answer
/// later, each `(handle, method)`, as WIT's `async func` marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] =
    &[(monotonic_clock::HANDLE, monotonic_clock::WAIT_FOR)];
