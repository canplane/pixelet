// pxl_core/shell/map_store.rs

use pxl_shared::spec::space::pvf3::pvf3::PVF3;

use crate::sys::global::Global;

/// Data loading lifecycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MapStatus {
    #[default]
    Idle,
    Loading,
    Success,
    Error,
}

impl MapStatus {
    /// The name the UI snapshot carries.
    pub fn as_str(self) -> &'static str {
        match self {
            MapStatus::Idle => "idle",
            MapStatus::Loading => "loading",
            MapStatus::Success => "success",
            MapStatus::Error => "error",
        }
    }
}

#[derive(Default)]
pub struct MapStore {
    // ---- Lifecycle & Status ----
    pub map_status: MapStatus,

    // ---- Navigation & Coordinate ----
    /// Canonical absolute target.
    pub map_center: Option<PVF3>,
}

impl MapStore {}

static MAP_STORE: Global<MapStore> = Global::new();

pub fn map_store() -> &'static mut MapStore {
    MAP_STORE.get(MapStore::default)
}
