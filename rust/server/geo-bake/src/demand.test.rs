// pxl-geo-bake/demand.test.rs

use super::*;

#[test]
fn a_restart_keeps_the_tiles_and_clears_what_was_cut_short() {
    let base = std::env::temp_dir().join(format!("pxl-demand-claim-{}", std::process::id()));
    let root = base.join("place");
    let tile = root.join("0").join("010332102301230");
    let work = root.join(".work-0");
    for d in [&tile, &work] {
        std::fs::create_dir_all(d).unwrap();
    }
    let owner = claim(&base).unwrap();
    assert!(
        claim(&base).is_err(),
        "a running server's cache is not taken from it"
    );
    prune(&root).unwrap();
    assert!(tile.exists(), "a baked tile is kept");
    assert!(!work.exists(), "a killed job's staging is removed");
    drop(owner);
    std::fs::remove_dir_all(base).unwrap();
}
