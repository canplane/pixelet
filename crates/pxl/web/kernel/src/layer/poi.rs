// pxl-kernel/layer/poi.rs

//! Places as a layer of the pyramid (world/pyramid.rs): a page counts the
//! places in each of its cells (pxl-spec bake/poi_page.rs), and a count is
//! drawn as a badge at the center of the block of cells the ground under it
//! is drawn at (placement.rs). Which pages are held and fetched is the
//! pyramid's, for the same view as the ground's.

pub mod badge;
pub mod placement;

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

    fn pyramid() -> &'static mut Pyramid<PoiPage> {
        POI.get(Pyramid::default)
    }

    /// A page is small and held on the heap; its record is only its room in
    /// the count the pyramid bounds.
    fn receive(_record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<PoiPage, Failure> {
        Ok(poi_page::decode(pkt)?)
    }

    /// A page the bake has no object for: at the top, one holding no places;
    /// under it, one the page over it settles, whose ground that page says.
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

/// The layer's turn: its pyramid's while places are shown, and nothing asked
/// for while they are not.
pub fn poi_step() {
    if enabled() {
        PoiPage::pyramid().step(None, &[]);
        crate::world::pyramid::cover::step::<PoiPage>();
    } else {
        PoiPage::pyramid().idle();
    }
}
