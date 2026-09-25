// pxl-geo-bake/page/rekey.rs

//! What a published object of one bake is called once pages are named by
//! their quadkeys, given what it was called before (pxl-spec bake/quadkey.rs).
//!
//! It computes and does nothing: a rename inside a bucket is a server-side
//! copy, and a rename on disk is a hard link, so what has to be right here is
//! only the arithmetic — which is why it is in the one crate that compiles
//! `pxl-spec` rather than in a script that would be a second copy of the
//! packed-lane maths.

use pxl_spec::bake::place_page::{DICT_OBJECT, GROUP_ZOOM};
use pxl_spec::bake::quadkey::{page_name, quadkey_of};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

/// The objects a bake keeps beside its pages, which are named rather than
/// keyed and so are carried across unchanged.
const SHARED: [&str; 2] = ["_ocean", DICT_OBJECT];

/// What the name after a bake's prefix is made of. A layer held as a pyramid
/// writes a page at every level, the level before it; geocode holds a page at
/// one zoom.
pub enum Bake {
    /// A page at every level of the pyramid, the level written before it.
    Pyramid,
    /// A page at one zoom, and nothing before it.
    Flat(u32),
}

impl Bake {
    /// Which bake a prefix belongs to. `asset` is not one: its objects are
    /// named by their content and there is nothing to rename.
    pub fn of(prefix: &str) -> Result<Self, String> {
        match prefix.split('/').next().unwrap_or_default() {
            "terrain" | "poi" => Ok(Bake::Pyramid),
            "geocode" => Ok(Bake::Flat(GROUP_ZOOM)),
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
            Bake::Pyramid => {
                let (level, page) = rest.split_once('/').ok_or("a pyramid page is a level and a page")?;
                let level = level.parse::<u32>().map_err(|e| format!("{level}: {e}"))?;
                if level > TOP_LEVEL {
                    return Err(format!("level {level} is past the pyramid's top"));
                }
                let page = page.parse::<IVec>().map_err(|e| format!("{page}: {e}"))?;
                return Ok(page_name(LevelPage::new(level, page)).to_string());
            }
        };
        let page = raw.parse::<IVec>().map_err(|e| format!("{raw}: {e}"))?;
        Ok(quadkey_of(zoom, page).to_string())
    }
}

#[cfg(test)]
#[path = "rekey.test.rs"]
mod test;
