// pxl-geo-bake/demand/poi.rs

use super::job;
use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::space::pyramid::Page;

pub(super) fn execute(p: Page, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    super::pyramid::execute::<PoiPage>(
        p,
        cache,
        out,
        || Ok(crate::poi::leaves(&mut job::connect(&[&crate::poi::READS], seconds)?.1, &crate::coverage::Coverage::of_source()?, Some(p.page))?.remove(&p.page)),
        &PoiPage::default(),
    )
}
