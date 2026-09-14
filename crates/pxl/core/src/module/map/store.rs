// pxl_core/module/map/store.rs

use pxl_shared::spec::space::pvf3::pvf3::PVF3;

use crate::global::Global;

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
    /// The TypeScript enum's string.
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

    /// Moves on every change, for the shell's selectors.
    pub revision: u32,
}

impl MapStore {
    pub fn set_map_status(&mut self, status: MapStatus) {
        if self.map_status != status {
            self.map_status = status;
            self.revision += 1;
        }
    }

    /// Stores absolute PVF3 without rebasing/localization.
    pub fn set_map_center(&mut self, map_center: PVF3) {
        self.map_center = Some(map_center);
        self.revision += 1;
    }
}

static MAP_STORE: Global<MapStore> = Global::new();

/// `useMapStore.getState()`.
pub fn use_map_store() -> &'static mut MapStore {
    MAP_STORE.get(MapStore::default)
}
