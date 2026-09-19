// pxl-kernel/io.rs

//! The kernel's I/O: what it asks the host to fetch, and what becomes of what
//! arrives. Ordering, the concurrency quota and cancelling what stopped being
//! wanted are the mechanism's (xpute_runtime::io::queue); which demands are
//! worth making, and what a body decodes into, is decided here.

pub mod demand;
pub mod fetch;
pub mod fetch_scheduler;
pub mod load;
pub mod model_asset;
