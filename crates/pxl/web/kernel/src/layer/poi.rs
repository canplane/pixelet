// pxl-kernel/layer/poi.rs

//! Places as a layer of the pyramid: a tile counts the places in each of
//! its cells, drawn as badges (placement.rs).

pub mod badge;
pub mod placement;

use pxl_spec::bake::poi_badge::KIND_CATEGORIES;
use pxl_spec::bake::poi_tile::{self, PoiTile};

use crate::control::camera::controller::units_per_px_at;
use crate::io::demand::{Category, Phases};
use crate::param::{POI_ALL_ZOOM, POI_MAJOR_ZOOM};
use crate::sched::profiler::record::Failure;
use crate::shell::setting::store::settings_store;
use crate::world::pyramid::{Layer, Pyramid};
use pxl_spec::space::adapter::epsg3857::cell_zoom_of;
use pxl_spec::space::zoom::units;
use xpute_runtime::global::Global;

static POI: Global<Pyramid<PoiTile>> = Global::new();

impl Layer for PoiTile {
    const CATEGORY: Category = Category::poi;
    fn reach() -> f64 {
        MAJOR_UNITS_PER_PX / units_per_px_at(1.0)
    }

    fn pyramid() -> &'static mut Pyramid<PoiTile> {
        POI.get(Pyramid::default)
    }

    /// Small, so held on the heap; the record is only a count slot.
    fn receive(_record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<PoiTile, Failure> {
        Ok(poi_tile::decode(pkt)?)
    }

    fn empty() -> Option<PoiTile> {
        Some(PoiTile::default())
    }
}

/// The units a logical pixel spans where the view is at each zoom.
pub const ALL_UNITS_PER_PX: f64 = units(cell_zoom_of(POI_ALL_ZOOM));
pub const MAJOR_UNITS_PER_PX: f64 = units(cell_zoom_of(POI_MAJOR_ZOOM));

pub fn enabled() -> bool {
    settings_store().poi
}

/// Lower is more prominent. The reader's choice, so a change needs no bake:
/// OpenMapTiles' `poi_class_rank` (layers/poi/class.sql) for the class its
/// poi.yaml files the category under; unranked is 1000.
pub fn rank_of(kind: u8) -> u16 {
    let category = kind.checked_sub(1).and_then(|k| KIND_CATEGORIES.get(k as usize)).copied().unwrap_or("");
    match category {
        "hospital" | "clinic" => 20,
        "subway" | "station" => 40,
        "bus_station" => 50,
        "attraction" => 70,
        "university" => 80,
        "school" | "kindergarten" => 85,
        "townhall" => 100,
        "park" => 120,
        "library" => 130,
        "police" => 135,
        "post_office" => 140,
        "mall" | "convenience" => 400,
        "supermarket" | "department_store" => 500,
        "fast_food" => 600,
        "clothes" => 700,
        "bar" => 800,
        _ => 1000,
    }
}

/// Nothing is fetched while places are hidden.
pub fn poi_step() {
    if enabled() {
        PoiTile::pyramid().step(None);
    } else {
        PoiTile::pyramid().idle();
    }
}
