// pxl-kernel/layer/decor.rs

//! What the world grows on its own: trees on green, cars on roads, boats on
//! water, trains on rails. A kind says what it may stand on and how a place
//! is chosen; the layer holds every kind's models, scatters the instances each
//! of its cells holds (decor/cell.rs), and answers which one a ray struck.

pub mod asset_store;
pub mod build;
pub mod cell;
pub mod hit;
pub mod instance;
pub mod kind;
pub mod registry;
pub mod scatter;
