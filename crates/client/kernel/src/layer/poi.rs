// pxl-kernel/layer/poi.rs

//! Places as a layer of the pyramid: a tile counts the places in each of
//! its cells, drawn as badges (`world/chunk/place.rs`).

use pxl_spec::bake::poi_badge::KIND_CATEGORIES;
use pxl_spec::bake::poi_tile::{self, PoiTile, POI_LATTICE};
use pxl_spec::space::layer::lattice::Lattice;

use crate::io::demand::{Category, Phases};
use crate::io::failure::Failure;
use crate::layer::residency::Residency;
use crate::layer::Layer;
use crate::layer::{Answer, Bound, Query};
use crate::param::{POI_ALL_ZOOM, POI_MAJOR_ZOOM};
use crate::shell::setting::store::settings_store;
use pxl_spec::bake::format::Occupancy;
use pxl_spec::space::coord::adapter::epsg3857::cell_zoom_of;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;

static POI: Global<Residency<PoiTile>> = Global::new();

impl Layer for PoiTile {
    const CATEGORY: Category = Category::poi;
    const LATTICE: Lattice = POI_LATTICE;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
    fn residency() -> &'static mut Residency<PoiTile> {
        POI.get(Residency::default)
    }

    /// Small, so held on the heap; the record is only a count slot.
    fn receive(_record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<PoiTile, Failure> {
        Ok(poi_tile::decode(pkt)?)
    }

    fn empty() -> Option<PoiTile> {
        Some(PoiTile::default())
    }
}

/// A node's places: the tile that says them and that tile's name.
#[derive(Clone, Copy)]
pub struct Places<'a> {
    pub tile: &'a PoiTile,
    pub from: Tile,
}

impl Query for Residency<PoiTile> {
    type Record<'a> = Places<'a>;

    /// A place fills nothing: it stands on the surface drawn there.
    fn bounds<const D: usize>(&self, _node: Node<D>, _cell: Zoom) -> Answer<Option<Bound>> {
        Answer::Exact(None)
    }

    /// From the resident tile that says the node's tile exactly, a parent
    /// settling it included; a miss is nothing, never a coarser count.
    fn read(&self, (zoom, [x, y, _]): Node<3>, cell: Zoom) -> Answer<Places<'_>> {
        let p = POI_LATTICE.read_tile((zoom, [x, y]), cell);
        match self.source(p).and_then(|src| Some(Places { tile: self.get(src)?, from: src })) {
            Some(places) => Answer::Exact(places),
            None => Answer::Nothing(p),
        }
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
