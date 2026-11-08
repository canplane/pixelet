// pxl-core/domain/control/edit.rs

//! What the pointer picks and edits in the world. What it does to the
//! camera is in input.rs. This module keeps the tool the pointer holds and
//! what is under the pointer. Each tool does every step of an edit in its own
//! module: what it points at, what a tap does, the edit it previews and the
//! ghost it shows (pointer.rs, eraser.rs, item.rs).

pub mod collision;
mod eraser;
pub mod ghost;
mod item;
pub mod pick;
mod pointer;
pub mod write;

use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::control::edit::pick::{pick_at, Pick};
use crate::domain::control::edit::write::Preview;
use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::asset::Asset;
use crate::domain::layer::object::store::Entry;
use crate::domain::preview::Ghost;
use crate::domain::query::World;
use pxl_spec::space::coord::zoom::{base_of, cube, idx_of, Zoom};
use pxl_spec::space::layer::octant::Octant;
use xpute_core::global::Global;

/// The tool the pointer holds, as the UI chose it (`EDIT_TOOL`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Tool {
    #[default]
    Pointer,
    Eraser,
    /// An item to place, at the zoom its scale comes to (`Asset::zoom`).
    Item {
        asset: Asset,
        zoom: Zoom,
    },
}

impl Tool {
    /// What the tool would place, and at which zoom.
    pub fn asset(self) -> Option<(Asset, Zoom)> {
        match self {
            Tool::Item { asset, zoom } => Some((asset, zoom)),
            Tool::Pointer | Tool::Eraser => None,
        }
    }

    /// What the tool points at where the pick struck.
    fn hover(self, pick: &Pick) -> Hover {
        match self {
            Tool::Pointer | Tool::Eraser => aim(pick),
            Tool::Item { zoom, .. } => item::hover(zoom, pick),
        }
    }

    /// What a tap does where the tool points (`hover`).
    fn tap(self, hover: &Hover) {
        match self {
            Tool::Pointer => pointer::tap(hover),
            Tool::Eraser => eraser::tap(hover),
            Tool::Item { asset, zoom } => item::tap(asset, zoom, hover),
        }
    }

    /// The edit a tap would make where the tool points, this turn; `was` the
    /// last turn's.
    fn preview(self, hover: &Hover, was: &Preview) -> Preview {
        match self {
            Tool::Pointer => pointer::preview(hover),
            Tool::Eraser => eraser::preview(hover),
            Tool::Item { asset, zoom } => item::preview(asset, zoom, hover, was),
        }
    }

    /// The ghost of the edit `preview` previews.
    fn ghost(self, hover: &Hover, preview: &Preview) -> Ghost {
        match self {
            Tool::Pointer => pointer::ghost(hover, preview),
            Tool::Eraser => eraser::ghost(hover, preview),
            Tool::Item { asset, zoom } => item::ghost(asset, zoom, hover, preview),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hover {
    pub coord: Option<Q3>,
    pub target: Option<Target>,
    /// The leaf cell across the struck face, where a pivot sent to the
    /// hover stands (`tap`).
    pub approach: Option<Q3>,
}

struct State {
    tool: Tool,
    hover: Hover,
    /// Where the pointer is on the screen, in NDC, whether or not it strikes
    /// anything. The gauge casts its own ray there.
    pointer_ndc: Option<[f64; 2]>,
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State {
        tool: Tool::default(),
        hover: Hover::default(),
        pointer_ndc: None,
    })
}

pub fn tool() -> Tool {
    state().tool
}

pub(crate) fn set_tool(tool: Tool) {
    state().tool = tool;
}

pub fn hover() -> Hover {
    state().hover
}

pub(crate) fn clear_hover() {
    state().hover = Hover::default();
    state().pointer_ndc = None;
}

/// Where the pointer is on the screen, in NDC. None while it is off the
/// screen.
pub fn pointer_ndc() -> Option<[f64; 2]> {
    state().pointer_ndc
}

/// The pointer is at `ndc_x`, `ndc_y` over something the world's pick does
/// not reach (pxl-core pointer.rs), so nothing in the world is picked.
pub(crate) fn point_at(ndc_x: f64, ndc_y: f64) {
    state().pointer_ndc = Some([ndc_x, ndc_y]);
    state().hover = Hover::default();
}

/// While a gesture is under way, the pointer points at nothing to place.
pub(crate) fn clear_hover_coord() {
    state().hover.coord = None;
}

/// What the pick struck, as a tool reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// A placed object.
    Object(Octant),
    /// A decor the ground generated, and the leaf cell it stands in. It has
    /// no entry in the store.
    Prop([i32; 3], Instance),
}

impl Hover {
    pub fn object_at(&self) -> Option<Octant> {
        match self.target {
            Some(Target::Object(at)) => Some(at),
            _ => None,
        }
    }

    pub fn prop(&self) -> Option<Instance> {
        match self.target {
            Some(Target::Prop(_, i)) => Some(i),
            _ => None,
        }
    }

    /// The placed object pointed at, as stored, and the zoom of the cube an
    /// edit there takes: the object's, or else a whole leaf cell, as a carve
    /// of the ground takes it.
    fn pointed(&self) -> (Option<Entry>, Zoom) {
        let object = self.object_at().and_then(|at| World::now().entry_at(at));
        let zoom = object.map_or(GROUND.leaf_cell_zoom(), |o| o.zoom);
        (object, zoom)
    }
}

/// Finds what is under the pointer at `ndc_x`, `ndc_y` for the tool held,
/// and keeps it as the hover.
pub(crate) fn hover_at(ndc_x: f64, ndc_y: f64) {
    state().pointer_ndc = Some([ndc_x, ndc_y]);
    state().hover = match pick_at(ndc_x, ndc_y).filter(|p| !p.is_folded()) {
        Some(p) => state().tool.hover(&p),
        None => Hover::default(),
    };
}

/// What the pointer and the eraser point at where `pick` struck: the placed
/// object or the generated decor struck, or else the leaf cell of the
/// ground; and the leaf cell across the struck face, where a pivot sent
/// there stands. Where the world takes no edit, nothing.
fn aim(pick: &Pick) -> Hover {
    let target = pick.target();
    let approach = Some(pick.adjacent_corner(GROUND.leaf_cell_zoom()));
    let (corner, zoom) = match target {
        Some(Target::Object(at)) => (base_of(at.zoom(), at.at()), at.zoom()),
        Some(Target::Prop(cell, _)) => (
            base_of(GROUND.leaf_cell_zoom(), cell),
            GROUND.leaf_cell_zoom(),
        ),
        // The corner is on the leaf cell's grid, not the size tier's, because
        // a carve is written at a leaf cell's size.
        None => (
            pick.struck_corner(GROUND.leaf_cell_zoom()),
            GROUND.leaf_cell_zoom(),
        ),
    };
    // Where the world takes no edit, nothing is pointed at.
    if !World::now().editable(&cube(zoom, idx_of(zoom, &corner))) {
        return Hover::default();
    }
    Hover {
        coord: Some(corner),
        target,
        approach,
    }
}

/// Carries out a tap at `ndc_x`, `ndc_y` with the tool held.
pub fn tap(ndc_x: f64, ndc_y: f64) {
    hover_at(ndc_x, ndc_y);
    let hover = state().hover;
    if hover.coord.is_none() {
        return;
    }
    state().tool.tap(&hover);
    // What the tap wrote is under the pointer, and the pointer may not move
    // again, so the world is picked again there now.
    hover_at(ndc_x, ndc_y);
}
