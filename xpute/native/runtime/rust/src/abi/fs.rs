// xpute-native-runtime/abi/fs.rs
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

pub mod file;
pub mod types;

/// The interfaces the world imports, each `(name, handle)`, which are
/// mounted before the guest boots: one with functions of its own under its
/// handle, one of resources alone by an opener at a prefix.
pub const IMPORTS: &[(&str, xpute_link::Handle)] = &[("xpute:fs/file", 0x5b6743)];

/// The methods of the named interfaces the host serves that answer
/// later, each `(handle, method)`, as WIT's `async func` marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] = &[];
