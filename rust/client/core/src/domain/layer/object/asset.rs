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
pub(crate) enum Asset {
    Cube(AssetId),
    Decor(AssetId),
    Marker,
    Unknown,
}

impl Asset {
    pub(crate) fn of(id: AssetId) -> Asset {
        match kind_of(id) {
            Some(Kind::Cube) => cube_of(id).map_or(Asset::Unknown, Asset::Cube),
            Some(Kind::Decor) => Decor::find(id).map_or(Asset::Unknown, |d| Asset::Decor(d.id)),
            Some(Kind::Marker) => Asset::Marker,
            _ => Asset::Unknown,
        }
    }

    pub(crate) fn cube(self) -> Option<AssetId> {
        match self {
            Asset::Cube(id) => Some(id),
            _ => None,
        }
    }

    pub(crate) fn decor(self) -> Option<&'static Decor> {
        match self {
            Asset::Decor(id) => Decor::find(id),
            _ => None,
        }
    }

    /// The asset's kind, which decides what it may stand on and what may
    /// stand on it. None for an unknown asset.
    pub(crate) fn kind(self) -> Option<Kind> {
        match self {
            Asset::Cube(_) => Some(Kind::Cube),
            Asset::Decor(_) => Some(Kind::Decor),
            Asset::Marker => Some(Kind::Marker),
            Asset::Unknown => None,
        }
    }

    /// The zoom of the cube the asset takes. A cube's zoom comes from the
    /// chosen scale step; anything else fills one unit cell.
    pub(crate) fn zoom(self, scale_step: i32) -> Zoom {
        match self {
            Asset::Cube(_) => object_zoom_of(scale_step),
            Asset::Decor(_) | Asset::Marker | Asset::Unknown => UNIT_ZOOM,
        }
    }
}

#[cfg(test)]
#[path = "asset.test.rs"]
mod test;
