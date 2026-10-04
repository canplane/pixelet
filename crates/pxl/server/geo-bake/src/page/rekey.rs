// pxl-geo-bake/page/rekey.rs

//! An object's quadkey name given its old one (pxl-spec bake/quadkey.rs).

use pxl_spec::bake::place_page::{DICT_OBJECT, GROUP_ZOOM};
use pxl_spec::bake::quadkey::{page_name, quadkey_of};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

const SHARED: [&str; 2] = ["_ocean", DICT_OBJECT];

pub enum Bake {
    Pyramid,
    Flat(u32),
}

impl Bake {
    pub fn of(prefix: &str) -> Result<Self, String> {
        match prefix.split('/').next().unwrap_or_default() {
            "terrain" | "poi" => Ok(Bake::Pyramid),
            "geocode" => Ok(Bake::Flat(GROUP_ZOOM)),
            other => Err(format!("{other}: not a bake this renames")),
        }
    }

    /// An unreadable name is an error, never a skip: nothing downstream would
    /// notice a missing object.
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
