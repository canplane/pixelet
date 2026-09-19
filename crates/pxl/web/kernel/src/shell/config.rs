// pxl-kernel/shell/config.rs

//! What ships as data rather than as code: the deployment's values, and every
//! same-origin route the app requests. Both are read by more than one side,
//! which is why neither is a literal where it is used.

pub mod env;
pub mod route;
