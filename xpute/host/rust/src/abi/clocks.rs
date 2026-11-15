// xpute-host/abi/clocks.rs
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

/// The methods of the interfaces the world imports that answer at once,
/// each `(handle, method)`, a function without WIT's `async`, which a
/// runtime refuses to carry to a UI on another thread.
pub const CALLS: &[(xpute_link::Handle, xpute_link::Method)] = &[(0x73f703, 0), (0x7dfa61, 0)];
