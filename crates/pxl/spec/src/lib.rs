//! pixelet's spec, in Rust: what every side follows and nothing decides
//! again — the space and its coordinates, the cell catalogs, the commands
//! and messages the shell and the kernel number, the bake's formats and the
//! shipped assets' tables. The server and the client read the same one; the
//! TypeScript's counterpart is @pxl/shared.

pub mod asset;
pub mod bake;
pub mod cell;
pub mod cmd;
pub mod fetch;
pub mod message;
pub mod space;
