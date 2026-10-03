// pxl-geo-bake/demand/poi.rs

use super::job;
use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::space::pyramid::LevelPage;

pub(super) fn execute(p: LevelPage, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    super::pyramid::execute::<PoiPage>(
        p,
        cache,
        out,
        || Ok(crate::poi::leaves(&mut job::connect(&[&crate::poi::READS], seconds)?, &crate::coverage::Coverage::of_source()?, Some(p.page))?.remove(&p.page)),
        || Ok(PoiPage::default()),
    )
}
