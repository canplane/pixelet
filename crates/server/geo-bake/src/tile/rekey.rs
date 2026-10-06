// pxl-geo-bake/tile/rekey.rs

//! A pyramid object's quadkey name given its `z<zoom>/<tile>` one
//! (pxl-spec bake/quadkey.rs).

use pxl_spec::bake::quadkey::tile_name;
use pxl_spec::space::coord::pvf3::ivec::IVec;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

/// The bakes held as a pyramid; geocode writes its quadkeys itself.
pub fn of(prefix: &str) -> Result<(), String> {
    match prefix.split('/').next().unwrap_or_default() {
        "terrain" | "poi" => Ok(()),
        other => Err(format!("{other}: not a bake this renames")),
    }
}

/// An unreadable name is an error, never a skip: nothing downstream would
/// notice a missing object.
pub fn rename(rest: &str) -> Result<String, String> {
    let (zoom, tile) = rest.split_once('/').ok_or("a pyramid tile is a zoom and a tile")?;
    let z = zoom.strip_prefix('z').and_then(|z| z.parse::<Zoom>().ok()).ok_or_else(|| format!("{zoom}: not a zoom"))?;
    if !(GROUND.top()..=LEAF_ZOOM).contains(&z) {
        return Err(format!("z{z} is past the pyramid"));
    }
    let tile = tile.parse::<IVec>().map_err(|e| format!("{tile}: {e}"))?;
    Ok(tile_name(Tile::new(z, tile)).to_string())
}

#[cfg(test)]
#[path = "rekey.test.rs"]
mod test;
