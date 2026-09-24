// pxl-geo-bake/demand/poi.rs

use super::job;
use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::space::pyramid::LevelPage;

/// A POI page: a leaf counted from the database, a page above folded by
/// adding; a page with no places is none.
pub(super) fn execute(p: LevelPage, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    super::pyramid::execute::<PoiPage>(
        p,
        cache,
        out,
        || Ok(crate::poi::leaves(&mut job::connect(seconds)?, Some(p.page))?.remove(&p.page)),
        || Ok(PoiPage::default()),
    )
}
