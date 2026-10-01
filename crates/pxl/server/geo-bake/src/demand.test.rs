// pxl-geo-bake/demand.test.rs

use super::*;

/// A restart keeps every page a stopped server baked and clears only what
/// it was killed in the middle of; a running server's cache is not taken
/// from it.
#[test]
fn a_restart_keeps_the_pages_and_clears_what_was_cut_short() {
    let base = std::env::temp_dir().join(format!("pxl-demand-claim-{}", std::process::id()));
    let root = base.join("poi");
    let page = root.join("0").join("132110320123012");
    let work = root.join(".work-0");
    let purge = cache::purged(&root);
    for d in [&page, &work, &purge] {
        std::fs::create_dir_all(d).unwrap();
    }
    let owner = claim(&base).unwrap();
    assert!(claim(&base).is_err(), "a running server's cache is not taken from it");
    prune(&root).unwrap();
    assert!(page.exists(), "a baked page is kept");
    assert!(!work.exists(), "a killed job's staging is removed");
    assert!(!purge.exists(), "a killed purge is finished");
    drop(owner);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn a_terrain_generation_holds_the_ocean_page_before_any_fold_reads_it() {
    use crate::pyramid::PageSet;
    use crate::terrain::leaf::{ocean_samples, Coverage, OCEAN_KEY};
    use pxl_spec::space::pyramid::LevelPage;

    let base = std::env::temp_dir().join(format!("pxl-demand-ocean-{}", std::process::id()));
    let runner = Runner::new(String::new(), 1, Limits::default()).unwrap();
    let data = Dataset::open(&base, Kind::Terrain, &runner).unwrap();
    let ocean = std::fs::read(data.cache.root.join(OCEAN_KEY)).expect("the ocean page is published as the generation opens");

    // One child stands on disk and the other fifteen pages the parent reads
    // are absent, so the fold reads the ocean page for them and nothing else.
    let child = LevelPage::new(0, ocean_samples(&Coverage::of_source())[1]);
    PageSet::new(&data.cache.root).write(child, &ocean).unwrap();
    let out = base.join("out");
    std::fs::create_dir_all(&out).unwrap();
    terrain::execute(child.parent(), &data.cache.root, &out, 1).unwrap();
    assert!(out.join("packet").is_file(), "a parent of an existing child folds to a page");

    drop(data);
    std::fs::remove_dir_all(base).unwrap();
}
