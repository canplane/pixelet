// pxl-core/domain/layer/object/asset.rs

//! What an asset id names. This is the one place an id is read, whether it
//! was stored or handed in. An id names a cube, a decor or a marker from the
//! catalog. Any other id is `Unknown`, an id from another version's catalog
//! among them: it is taken for nothing here, drawn as the catalog's
//! `unknown`, and left as it is where it is stored. What a tool does with an
//! asset is defined beside the tool (`control/edit`).

use pxl_spec::asset::cube::cube_of;
use pxl_spec::asset::decor::Decor;
use pxl_spec::asset::id::{kind_of, Kind};
use pxl_spec::asset::AssetId;
use pxl_spec::space::coord::zoom::{Zoom, UNIT_ZOOM};

use crate::domain::layer::object::size::object_zoom_of;

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

    /// The asset's kind, which decides what it may stand on and what may
    /// stand on it. None for an unknown asset.
    pub fn kind(self) -> Option<Kind> {
        match self {
            Asset::Cube(_) => Some(Kind::cube),
            Asset::Decor(_) => Some(Kind::decor),
            Asset::Marker => Some(Kind::marker),
            Asset::Unknown => None,
        }
    }

    /// The zoom of the cube the asset takes. A cube's zoom comes from the
    /// chosen scale step; anything else fills one unit cell.
    pub fn zoom(self, scale_steps: i32) -> Zoom {
        match self {
            Asset::Cube(_) => object_zoom_of(scale_steps),
            Asset::Decor(_) | Asset::Marker | Asset::Unknown => UNIT_ZOOM,
        }
    }
}

#[cfg(test)]
#[path = "asset.test.rs"]
mod test;
