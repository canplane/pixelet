// pxl-geo-bake/util.test.rs

use std::io::Write;

use super::*;

fn tile() -> Vec<u8> {
    (0..100_000u32).map(|i| (i / 700) as u8).collect()
}

fn gzip(raw: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(raw).unwrap();
    enc.finish().unwrap()
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pxl-geo-bake-util-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_packed_tile_is_labeled_what_it_is_and_unpacks_to_itself() {
    let packed = pack(&tile(), Level::Stored).unwrap();
    assert_eq!(encoding_of(&packed), Some(CONTENT_ENCODING));
    assert_eq!(unpack(&packed).unwrap(), tile());
}

#[test]
fn a_tile_stored_before_zstd_still_unpacks() {
    let old = gzip(&tile());
    assert_eq!(encoding_of(&old), Some("gzip"));
    assert_eq!(unpack(&old).unwrap(), tile());
    assert!(unpack(b"not a tile").is_err());
}

#[test]
fn a_bake_will_not_add_to_a_directory_of_another_encoding() {
    let old = scratch("old");
    std::fs::write(old.join("1"), gzip(&tile())).unwrap();
    assert!(refuse_mixed(&old).is_err());

    let same = scratch("same");
    std::fs::write(same.join("1.empty"), b"").unwrap();
    std::fs::write(same.join("1"), pack(&tile(), Level::Stored).unwrap()).unwrap();
    assert!(refuse_mixed(&same).is_ok());
    assert!(refuse_mixed(&same.join("absent")).is_ok());
    let _ = std::fs::remove_dir_all(old);
    let _ = std::fs::remove_dir_all(same);
}
