// pxl-geo-spec/overture/category.rs

use crate::overture::category_spec::OVERTURE_CATEGORY_MAP;

/// Overture's `categories.primary` translated into the vocabulary the POI
/// badge's theme keys off, or none when the table has no entry for it.
///
/// The table itself is generated from spec/pxl/server/overture/category.json,
/// as is the shell's copy, and the generator fails if either drifts from the
/// theme they both have to land in.
pub fn map(primary: &str) -> Option<&'static str> {
    OVERTURE_CATEGORY_MAP.binary_search_by_key(&primary, |(key, _)| *key).ok().map(|at| OVERTURE_CATEGORY_MAP[at].1)
}

#[cfg(test)]
#[path = "category.test.rs"]
mod test;
