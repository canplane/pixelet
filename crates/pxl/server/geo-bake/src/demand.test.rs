// pxl-geo-bake/demand.test.rs

use super::*;

#[test]
fn a_claimed_cache_drops_what_a_stopped_server_left_and_refuses_a_second_owner() {
    let base = std::env::temp_dir().join(format!("pxl-demand-claim-{}", std::process::id()));
    let orphan = base.join("poi").join("1").join("132110320123012");
    std::fs::create_dir_all(&orphan).unwrap();
    let owner = claim(&base).unwrap();
    assert!(!base.join("poi").exists(), "a generation no running server owns is removed");
    assert!(claim(&base).is_err(), "a running server's cache is not taken from it");
    drop(owner);
    std::fs::remove_dir_all(base).unwrap();
}
