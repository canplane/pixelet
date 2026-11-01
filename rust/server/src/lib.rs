// pxl-server/lib.rs

//! What `pxl-geo` and `pxl-api` share because each is a server: env, pool,
//! the internal-route gate and the serve loop.

pub(crate) mod admin_auth;
pub mod config;
pub mod db;
pub mod serve;
