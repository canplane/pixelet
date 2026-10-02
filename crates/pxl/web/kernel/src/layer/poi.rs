// pxl-kernel/layer/poi.rs

//! Places as a layer of the pyramid: a page counts the places in each of
//! its cells, drawn as badges (placement.rs).

pub mod badge;
pub mod placement;

use pxl_spec::bake::poi_badge::KIND_CATEGORIES;
use pxl_spec::bake::poi_page::{self, PoiPage};
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

use crate::global::Global;
use crate::io::demand::{Category, Phases};
use crate::sched::profiler::record::Failure;
use crate::shell::setting::store::settings_store;
use crate::world::pyramid::{Layer, Pyramid};

static POI: Global<Pyramid<PoiPage>> = Global::new();

impl Layer for PoiPage {
    const CATEGORY: Category = Category::poi;
    const COARSEST: u32 = crate::param::POI_MAJOR_LEVEL;

    fn pyramid() -> &'static mut Pyramid<PoiPage> {
        POI.get(Pyramid::default)
    }

    /// Small, so held on the heap; the record is only a count slot.
    fn receive(_record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<PoiPage, Failure> {
        Ok(poi_page::decode(pkt)?)
    }

    /// At the top, a page with no places; under it, one the page above settles.
    fn absent(p: LevelPage) {
        if p.level < TOP_LEVEL {
            PoiPage::pyramid().answer_above(p);
        } else {
            PoiPage::pyramid().set(p, PoiPage::default());
        }
    }
}

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

/// Nothing is fetched while places are hidden.
pub fn poi_step() {
    if enabled() {
        PoiPage::pyramid().step(None, &[]);
        crate::world::pyramid::cover::step::<PoiPage>();
    } else {
        PoiPage::pyramid().idle();
    }
}
