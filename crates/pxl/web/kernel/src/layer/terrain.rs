// pxl-kernel/layer/terrain.rs

//! The ground itself: which pages should be resident, how one is fetched and
//! where it lies, the elevation sampled under a point, and what the paint is
//! taught about its materials. The streaming knobs are config.json's, which
//! says why each is what it is.

pub mod config;
#[path = "terrain/config.spec.rs"]
pub mod config_spec;
pub mod elevation;
pub mod fixed;
pub mod ground_snap;
pub mod level;
pub mod resident;
pub mod store;
pub mod teach;
pub mod tile_fetcher;
pub mod tile_repository;
