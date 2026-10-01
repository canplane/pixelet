// pxl-kernel/shell/map_store.rs

use pxl_spec::space::pvf3::pvf3::Pvf3;

pub use crate::abi::cmd::MapStatus;
use crate::global::Global;

#[derive(Default)]
pub struct MapStore {
    // ---- Lifecycle & Status ----
    pub map_status: MapStatus,

    // ---- Navigation & Coordinate ----
    /// Where navigation has the map: canonical and absolute. The orbit looks
    /// at it; the free camera is carried there when a navigation command
    /// moves it (`navigate_to`), and not by every write — a pan or a regrounding.
    pub map_center: Option<Pvf3>,
    /// Moves with every navigation command: a search, a recenter, the first
    /// fix, a fix followed, a tap on the ground.
    pub navigated: u32,
}

impl MapStore {
    /// Navigation moves the map to `target`: a command, which a controller
    /// that does not stand about the map center takes as where to go.
    pub fn navigate_to(&mut self, target: Pvf3) {
        self.map_center = Some(target);
        self.navigated = self.navigated.wrapping_add(1);
    }
}

static MAP_STORE: Global<MapStore> = Global::new();

pub fn map_store() -> &'static mut MapStore {
    MAP_STORE.get(MapStore::default)
}
