// pxl-geo-bake/page/rekey.rs

//! A pyramid object's quadkey name given its `z<zoom>/<page>` one
//! (pxl-spec bake/quadkey.rs).

use pxl_spec::bake::quadkey::page_name;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{Page, LEAF_Z, TOP_Z};

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
    let (zoom, page) = rest.split_once('/').ok_or("a pyramid page is a zoom and a page")?;
    let z = zoom.strip_prefix('z').and_then(|z| z.parse::<u32>().ok()).ok_or_else(|| format!("{zoom}: not a zoom"))?;
    if !(TOP_Z..=LEAF_Z).contains(&z) {
        return Err(format!("z{z} is past the pyramid"));
    }
    let page = page.parse::<IVec>().map_err(|e| format!("{page}: {e}"))?;
    Ok(page_name(Page::new(z, page)).to_string())
}

#[cfg(test)]
#[path = "rekey.test.rs"]
mod test;
