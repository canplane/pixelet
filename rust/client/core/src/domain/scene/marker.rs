// pxl-core/domain/scene/marker.rs

//! Markers: billboards drawn flat to the camera over the world, each from its
//! own design on the one atlas. Three sources feed them: the places, as Carto
//! lays them out; the pins people put up, which are objects of the marker kind;
//! and the chat's line. Each source is read on its own, and one arrangement
//! draws them all together (`arrangement.rs`). What a source hands the
//! arrangement, which both speak, is written here.

mod arrangement;
mod atlas;
mod candidate;
mod chat;
mod design;
mod line;
mod param;
mod pin;
mod place;
mod selection;
mod set;

pub(crate) use arrangement::{draw, on_changed, step, tap};
pub(crate) use atlas::atlas;
pub(crate) use chat::append_chat_line;
pub(crate) use design::text_raster;

use pxl_spec::brick::BRICK_SHIFT;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::Cell;
use pxl_spec::space::layer::octant::Octant;

use design::Design;

/// A brick of the eye's clip levels that markers are read over, named by its
/// zoom and place. Its X runs on round the world without wrapping, as the
/// view's frame does (domain/view/observation.rs `bricks`).
type Brick = Node<2>;

/// The zoom of a brick's cells: the camera grid's zoom at the brick's
/// nearest point, the finest the brick draws.
fn cell_zoom((zoom, _): Brick) -> Zoom {
    zoom + BRICK_SHIFT
}

/// The name a holder gives what it shows, stable from one run to the next
/// and independent of what is painted (`design.rs`). A pin is named by its
/// entry, a pin's cell whose count it shows by the cell (`pin.rs`), a place
/// by the ground leaf cell it stands in, which is its own whatever tile it
/// was read from (pxl-spec bake/tile/place.rs: a leaf cell keeps one place),
/// and the chat's line by its speaker, of whom there is one. The arrangement
/// uses the key only to tell instances apart, so it can keep an instance's
/// old drawing until its new one is painted. Where a tap goes is the
/// instance's own (`Instance::tap`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Pin(Octant),
    PinCell((Zoom, [i32; 3])),
    Place(Cell),
    Line,
}

/// What a source hands the arrangement: its holder's key, the design it
/// paints, where it stands, and where a tap on it goes, or None where it
/// takes no tap.
#[derive(Clone)]
struct Instance {
    key: Key,
    design: Design,
    pos: Q3,
    tap: Option<MarkerHit>,
}

/// What a tap on a scene marker hit: a place to go to, or a pin's entry,
/// which the tool held decides what to do with.
#[derive(Clone, Copy)]
pub(crate) enum MarkerHit {
    To(Q3),
    Pin(Octant, Q3),
}
