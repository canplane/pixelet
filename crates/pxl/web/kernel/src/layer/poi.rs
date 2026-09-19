// pxl-kernel/layer/poi.rs

//! The places a real map knows about: which pages to fetch, the packets they
//! arrive in, the index that answers what is near, and the badge each one is
//! drawn as. A POI is world-anchored like a billboard, so its placement goes
//! through the same budget.

pub mod badge;
pub mod fetcher;
pub mod index;
pub mod placement;
pub mod repository;
pub mod store;
pub mod theme;
pub mod theme_spec;
