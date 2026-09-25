// pxl-server/lib.rs

//! What `pxl-geo` and `pxl-api` both need before they can answer anything:
//! the environment they read, the pool they open, the gate over their
//! internal routes, and the life that runs them (serve.rs).
//!
//! It holds nothing either of them answers *with*. The two share no table, no
//! route and no wire shape — they run on different databases and different
//! machines — so what is here is the part of a server that is the same because
//! it is a server, not because they are related.

pub mod admin_auth;
pub mod config;
pub mod db;
pub mod serve;
