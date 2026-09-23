// pxl-geo-bake/page/rekey.rs

//! What a published object of one bake is called once pages are named by
//! their quadkeys, given what it was called before (pxl-spec bake/quadkey.rs).
//!
//! It computes and does nothing: a rename inside a bucket is a server-side
//! copy, and a rename on disk is a hard link, so what has to be right here is
//! only the arithmetic — which is why it is in the one crate that compiles
//! `pxl-spec` rather than in a script that would be a second copy of the
//! packed-lane maths.

use pxl_spec::bake::constant::{zoom_of_level, LEVEL0_ZOOM, PAGE_TOP_LEVEL};
use pxl_spec::bake::place_page::{DICT_OBJECT, GROUP_SHAMT};
use pxl_spec::bake::quadkey::quadkey_of;
use pxl_spec::space::pvf3::ivec::IVec;

/// The objects a bake keeps beside its pages, which are named rather than
/// keyed and so are carried across unchanged.
const SHARED: [&str; 2] = ["_ocean", DICT_OBJECT];

/// What the name after a bake's prefix is made of. The three bakes differ only
/// in the zoom they hold a page at, and terrain in that the zoom is per page.
pub enum Bake {
    /// A page at every level of the pyramid, the level written before it.
    Terrain,
    /// A page at one zoom, and nothing before it.
    Flat(u32),
}

impl Bake {
    /// Which bake a prefix belongs to. `asset` is not one: its objects are
    /// named by their content and there is nothing to rename.
    pub fn of(prefix: &str) -> Result<Self, String> {
        match prefix.split('/').next().unwrap_or_default() {
            "terrain" => Ok(Bake::Terrain),
            "poi" => Ok(Bake::Flat(LEVEL0_ZOOM)),
            "geocode" => Ok(Bake::Flat(LEVEL0_ZOOM - GROUP_SHAMT)),
            other => Err(format!("{other}: not a bake this renames")),
        }
    }

    /// The new name for what followed the old prefix.
    ///
    /// An unreadable name is an error and never a skip: a skipped object is an
    /// object the new prefix does not have, and nothing downstream would say
    /// so.
    pub fn rename(&self, rest: &str) -> Result<String, String> {
        if SHARED.contains(&rest) {
            return Ok(rest.to_string());
        }
        let (zoom, raw) = match self {
            Bake::Flat(zoom) => (*zoom, rest),
            Bake::Terrain => {
                let (level, page) = rest.split_once('/').ok_or("a terrain page is a level and a page")?;
                let level = level.parse::<u32>().map_err(|e| format!("{level}: {e}"))?;
                if level > PAGE_TOP_LEVEL {
                    return Err(format!("level {level} is past the pyramid's top"));
                }
                (zoom_of_level(level), page)
            }
        };
        let page = raw.parse::<IVec>().map_err(|e| format!("{raw}: {e}"))?;
        Ok(quadkey_of(zoom, page).to_string())
    }
}

#[cfg(test)]
#[path = "rekey.test.rs"]
mod test;
