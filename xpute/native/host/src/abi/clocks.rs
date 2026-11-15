// xpute-native-host/abi/clocks.rs
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

pub mod monotonic_clock;
pub mod system_clock;
pub mod types;

/// The interfaces the world imports, each `(name, handle)`, which are
/// mounted before the guest boots: one with functions of its own under its
/// handle, one of resources alone by an opener at a prefix.
pub const IMPORTS: &[(&str, xpute_link::Handle)] = &[
    ("xpute:clocks/monotonic-clock", 0x73f703),
    ("xpute:clocks/system-clock", 0x7dfa61),
];

/// The methods of the package's own named interfaces its world imports
/// that answer later, each `(handle, method)`, as WIT's `async func`
/// marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] =
    &[(monotonic_clock::HANDLE, monotonic_clock::WAIT_FOR)];
