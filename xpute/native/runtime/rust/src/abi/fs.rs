// xpute-native-runtime/abi/fs.rs
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

pub mod file;
pub mod types;

/// The interfaces the world imports that the host serves under a named
/// handle, each `(name, handle)`, which are mounted before the guest boots.
pub const IMPORTS: &[(&str, xpute_link::Handle)] = &[];

/// The methods of the named interfaces the host serves that answer
/// later, each `(handle, method)`, as WIT's `async func` marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] = &[];
