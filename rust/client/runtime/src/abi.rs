// pxl-runtime/abi.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

pub mod camera;
pub mod edit;
pub mod input;
pub mod object;
pub mod overlay;
pub mod perf;
pub mod scene;
pub mod sensor;
pub mod setting;
pub mod source;
pub mod text;
pub mod types;
pub mod watch;

/// The methods of the named interfaces the host serves that answer
/// later, each `(handle, method)`, as WIT's `async func` marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] = &[(object::HANDLE, object::APPLY)];
