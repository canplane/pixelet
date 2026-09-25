// pxl-kernel/shell/map_store.rs

use pxl_spec::space::pvf3::pvf3::Pvf3;

pub use crate::abi::cmd::MapStatus;
use crate::global::Global;

#[derive(Default)]
pub struct MapStore {
    // ---- Lifecycle & Status ----
    pub map_status: MapStatus,

    // ---- Navigation & Coordinate ----
    /// Canonical absolute target.
    pub map_center: Option<Pvf3>,
}

impl MapStore {}

static MAP_STORE: Global<MapStore> = Global::new();

pub fn map_store() -> &'static mut MapStore {
    MAP_STORE.get(MapStore::default)
}
