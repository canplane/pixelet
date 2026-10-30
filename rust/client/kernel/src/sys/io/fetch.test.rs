// pxl-kernel/sys/io/fetch.test.rs

use crate::sys::io::host_spec;
use pxl_spec::bake::published::TILE_SPHERE;
use pxl_spec::space::layer::tile::Tile;
use xpute_kit::codec::json::read_json;

/// The host names a tile by its subject as the kernel once
/// named it itself,
/// recorded in this fixture, which its test reads too (tile.test.ts):
/// each name there is its subject's, as pxl-spec writes it.
const FIXTURE: &str = include_str!(concat!(
    env!("PXL_REPO"),
    "/ts/client/src/driver/namespace/tile.fixture.json"
));

#[test]
fn fixture_tile_names() {
    let fixture = read_json(FIXTURE).expect("the fixture is JSON");
    let tiles = fixture.get("tiles").expect("its tiles");
    let mut n = 0;
    for row in tiles.items() {
        let subject: u64 = row
            .get("subject")
            .and_then(|s| s.as_str())
            .and_then(|s| s.as_plain())
            .and_then(|s| s.parse().ok())
            .expect("a subject");
        let object = row
            .get("object")
            .and_then(|o| o.as_str())
            .and_then(|o| o.as_plain())
            .expect("an object");
        let (_, name) = object.split_once('/').expect("a version, then a name");
        assert_eq!(name, Tile::of_subject(subject).name(), "{subject}");
        assert_ne!(
            subject,
            u64::from(host_spec::SPHERE),
            "no tile's number is the sphere's"
        );
        n += 1;
    }
    assert!(n > 0);
    let sphere = fixture
        .get("sphere")
        .and_then(|s| s.as_str())
        .and_then(|s| s.as_plain())
        .expect("the sphere");
    assert_eq!(
        sphere.split_once('/').map(|(_, name)| name),
        Some(TILE_SPHERE)
    );
}
