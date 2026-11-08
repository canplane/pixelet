// pxl-core/domain/layer/object/asset.rs

//! What an id names, the one reading of one, stored or handed in, and what each
//! kind does: a cube, a decor or a marker the catalog gives out. Anything else is
//! `Unknown`, of another version's catalog among them: taken for nothing here,
//! drawn as the catalog's `unknown`, and left as it is where it is stored. What
//! a tool does with one is beside the tool (`control/edit`).

use pxl_spec::asset::cube::cube_of;
use pxl_spec::asset::decor::Decor;
use pxl_spec::asset::id::{kind_of, Kind};
use pxl_spec::asset::AssetId;
use pxl_spec::space::coord::zoom::{Zoom, UNIT_ZOOM};
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::layer::object::size::{object_zoom_of, MARKER_ZOOM};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asset {
    Cube(AssetId),
    Decor(AssetId),
    Marker,
    Unknown,
}

impl Asset {
    pub fn of(id: AssetId) -> Asset {
        match kind_of(id) {
            Some(Kind::cube) => cube_of(id).map_or(Asset::Unknown, Asset::Cube),
            Some(Kind::decor) => Decor::find(id).map_or(Asset::Unknown, |d| Asset::Decor(d.id)),
            Some(Kind::marker) => Asset::Marker,
            _ => Asset::Unknown,
        }
    }

    pub fn cube(self) -> Option<AssetId> {
        match self {
            Asset::Cube(id) => Some(id),
            _ => None,
        }
    }

    pub fn decor(self) -> Option<&'static Decor> {
        match self {
            Asset::Decor(id) => Decor::find(id),
            _ => None,
        }
    }

    /// The kind it is of, as it stands under and on others.
    pub fn kind(self) -> Option<Kind> {
        match self {
            Asset::Cube(_) => Some(Kind::cube),
            Asset::Decor(_) => Some(Kind::decor),
            Asset::Marker => Some(Kind::marker),
            Asset::Unknown => None,
        }
    }

    /// The zoom of the cube it takes: a cube's comes of the scale chosen; a
    /// decor's and a marker's are their own.
    pub fn zoom(self, scale_steps: i32) -> Zoom {
        match self {
            Asset::Cube(_) => object_zoom_of(scale_steps),
            Asset::Decor(_) => UNIT_ZOOM,
            Asset::Marker => MARKER_ZOOM,
            Asset::Unknown => GROUND.leaf_cell_zoom(),
        }
    }
}

#[cfg(test)]
#[path = "asset.test.rs"]
mod test;
