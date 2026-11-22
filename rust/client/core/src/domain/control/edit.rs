// pxl-core/domain/control/edit.rs

//! What the pointer picks and edits in the world. What it does to the
//! camera is in input.rs. This module keeps the tool the pointer holds and
//! what is under the pointer. Each tool does the steps of an edit in its own
//! module: what a tap does, the edit it previews and the ghost it shows
//! (pointer.rs, eraser.rs, item.rs). The pointer and the eraser point at what
//! the pick struck alike (`aim`); the item at the corner flush against it.

pub(crate) mod collision;
mod eraser;
pub(crate) mod ghost;
mod item;
pub(crate) mod pick;
mod pointer;
pub(crate) mod write;

use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::control::edit::pick::{Pick, Target, pick_at};
use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::asset::Asset;
use crate::domain::layer::object::store::Entry;
use crate::domain::preview::{Ghost, RenderEdit};
use crate::domain::query::World;
use pxl_global::Global;
use pxl_spec::space::coord::world::{base_of, cube, idx_of};
use pxl_spec::space::coord::zoom::Zoom;

/// The tool the pointer holds, as the UI chose it (pxl:core edit `hold`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Tool {
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
    pub(crate) fn asset(self) -> Option<(Asset, Zoom)> {
        match self {
            Tool::Item { asset, zoom } => Some((asset, zoom)),
            Tool::Pointer | Tool::Eraser => None,
        }
    }

    fn hover(self, pick: &Pick) -> Hover {
        match self {
            Tool::Pointer | Tool::Eraser => aim(pick),
            Tool::Item { zoom, .. } => item::hover(zoom, pick),
        }
    }

    fn tap(self, hover: &Hover) {
        match self {
            Tool::Pointer => pointer::tap(hover),
            Tool::Eraser => eraser::tap(hover),
            Tool::Item { asset, zoom } => item::tap(asset, zoom, hover),
        }
    }

    /// What the tool leaves out of the world as drawn: only the eraser does.
    fn render_edit(self, hover: &Hover) -> RenderEdit {
        match self {
            Tool::Eraser => eraser::render_edit(hover),
            Tool::Pointer | Tool::Item { .. } => RenderEdit::default(),
        }
    }

    fn ghost(self, hover: &Hover) -> Ghost {
        match self {
            Tool::Pointer => pointer::ghost(hover),
            Tool::Eraser => eraser::ghost(hover),
            Tool::Item { asset, zoom } => item::ghost(asset, zoom, hover),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Hover {
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
    /// anything. pointer.rs `ray_px` names the drawn pixel there.
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

pub(crate) fn tool() -> Tool {
    state().tool
}

pub(crate) fn set_tool(tool: Tool) {
    state().tool = tool;
}

pub(crate) fn hover() -> Hover {
    state().hover
}

pub(crate) fn clear_hover() {
    state().hover = Hover::default();
    state().pointer_ndc = None;
}

/// Where the pointer is on the screen, in NDC. None while it is off the
/// screen.
pub(crate) fn pointer_ndc() -> Option<[f64; 2]> {
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

impl Hover {
    pub(crate) fn decor(&self) -> Option<Instance> {
        self.target?.decor
    }

    /// The placed object pointed at, as stored, and the zoom of the cube an
    /// edit there takes: the object's, or else a whole leaf cell, as a carve
    /// of the ground takes it.
    fn object(&self) -> (Option<Entry>, Zoom) {
        let object = self
            .target
            .filter(|t| t.decor.is_none())
            .and_then(|t| World::now().entry_at(t.at));
        let zoom = object.map_or(GROUND.leaf_cell_zoom(), |o| o.zoom);
        (object, zoom)
    }
}

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
        Some(Target { at, .. }) => (base_of(at.zoom(), at.at()), at.zoom()),
        // The corner is on the leaf cell's grid, not the size tier's, because
        // a carve is written at a leaf cell's size.
        None => (
            pick.struck_corner(GROUND.leaf_cell_zoom()),
            GROUND.leaf_cell_zoom(),
        ),
    };
    if !World::now().editable(&cube(zoom, idx_of(zoom, &corner))) {
        return Hover::default();
    }
    Hover {
        coord: Some(corner),
        target,
        approach,
    }
}

pub(crate) fn tap(ndc_x: f64, ndc_y: f64) {
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
