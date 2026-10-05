// pxl-geo-bake/demand.test.rs

use super::*;

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
