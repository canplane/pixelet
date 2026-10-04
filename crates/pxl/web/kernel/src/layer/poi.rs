// pxl-kernel/layer/poi.rs

//! Places as a layer of the pyramid: a page counts the places in each of
//! its cells, drawn as badges (placement.rs).

pub mod badge;
pub mod placement;

use pxl_spec::bake::poi_badge::KIND_CATEGORIES;
use pxl_spec::bake::poi_page::{self, PoiPage};

use crate::io::demand::{Category, Phases};
use crate::sched::profiler::record::Failure;
use crate::shell::setting::store::settings_store;
use crate::world::pyramid::{Layer, Pyramid};
use xpute_runtime::global::Global;

static POI: Global<Pyramid<PoiPage>> = Global::new();

impl Layer for PoiPage {
    const CATEGORY: Category = Category::poi;

    fn pyramid() -> &'static mut Pyramid<PoiPage> {
        POI.get(Pyramid::default)
    }

    /// Small, so held on the heap; the record is only a count slot.
    fn receive(_record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<PoiPage, Failure> {
        Ok(poi_page::decode(pkt)?)
    }

    fn empty() -> Option<PoiPage> {
        Some(PoiPage::default())
    }
}

pub fn enabled() -> bool {
    settings_store().poi
}

/// How prominent a place of kind `kind` is, the more the lower: the
/// reader's choice, never the page's (pxl-spec bake/poi_page.rs), so a change
/// of it is no new bake. OpenMapTiles' `poi_class_rank` (layers/poi/class.sql)
/// of the class its `poi` layer files the category under
/// (layers/poi/poi.yaml): a clinic is a hospital, a subway a railway, a
/// kindergarten a school, a mall or a convenience store a shop, a department
/// store a grocery; what it does not rank, 1000.
pub fn rank_of(kind: u8) -> u16 {
    let category = kind.checked_sub(1).and_then(|k| KIND_CATEGORIES.get(k as usize)).copied().unwrap_or("");
    match category {
        "hospital" | "clinic" => 20,
        "subway" | "station" => 40,
        "bus_station" => 50,
        "attraction" => 70,
        "university" | "college" => 80,
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

/// The layer's turn: its pyramid's while places are shown, out to where the
/// ground is drawn coarser than the coarsest level a place is shown at
/// (param.rs `POI_MAJOR_LEVEL`) and no farther, and nothing asked for while
/// they are not.
pub fn poi_step() {
    if enabled() {
        PoiPage::pyramid().step(None);
    } else {
        PoiPage::pyramid().idle();
    }
}
