// pxl-geo-bake/page/rekey.rs

//! A pyramid object's quadkey name given its `<level>/<page>` one
//! (pxl-spec bake/quadkey.rs).

use pxl_spec::bake::quadkey::page_name;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

const SHARED: [&str; 1] = ["_ocean"];

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
    if SHARED.contains(&rest) {
        return Ok(rest.to_string());
    }
    let (level, page) = rest.split_once('/').ok_or("a pyramid page is a level and a page")?;
    let level = level.parse::<u32>().map_err(|e| format!("{level}: {e}"))?;
    if level > TOP_LEVEL {
        return Err(format!("level {level} is past the pyramid's top"));
    }
    let page = page.parse::<IVec>().map_err(|e| format!("{page}: {e}"))?;
    Ok(page_name(LevelPage::new(level, page)).to_string())
}

#[cfg(test)]
#[path = "rekey.test.rs"]
mod test;
