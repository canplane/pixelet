// pxl-kernel/shell/map_store.rs

pub use crate::abi::cmd::MapStatus;
use crate::global::Global;

#[derive(Default)]
pub struct MapStore {
    pub map_status: MapStatus,
}

static MAP_STORE: Global<MapStore> = Global::new();

pub fn map_store() -> &'static mut MapStore {
    MAP_STORE.get(MapStore::default)
}
