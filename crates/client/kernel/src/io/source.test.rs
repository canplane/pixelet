// pxl-kernel/io/source.test.rs

use super::*;

fn tile(name: &str) -> Tile {
    Tile::of_name(name).expect("a tile's name")
}

const DECLARED: &str = r#"{
  "top_zoom": 5,
  "format": { "terrain": 8, "place": 10 },
  "path": { "terrain": "v8.2026-10-09.1", "place": "v10.2026-10-09.1", "earth": "v1.2026-10-05.1" },
  "shard": { "0103300": {}, "01033210": { "path": { "place": "v10.2026-10-12.1" } } }
}"#;

#[test]
fn a_tile_is_served_over_or_under_a_shard_and_nowhere_else() {
    declare(DECLARED).expect("a declaration the client reads");
    for served in ["01033", "0103300", "010330012", "010332", "01033210", "010332103"] {
        assert!(serves(BaseLayer::terrain, tile(served)), "{served}");
    }
    for beside in ["01032", "0103301", "01033211", "010332110"] {
        assert!(!serves(BaseLayer::terrain, tile(beside)), "{beside}");
    }
    assert!(serves(BaseLayer::earth, tile("01032")), "the Earth's picture is no shard's");
}

#[test]
fn a_shard_names_its_own_path_at_and_under_it_and_the_source_the_rest() {
    declare(DECLARED).expect("a declaration the client reads");
    assert_eq!(path(BaseLayer::place, tile("010332103")), Some("v10.2026-10-12.1"));
    assert_eq!(path(BaseLayer::place, tile("010332")), Some("v10.2026-10-09.1"), "over the shard is the source's");
    assert_eq!(path(BaseLayer::terrain, tile("010332103")), Some("v8.2026-10-09.1"));
    assert_eq!(path(BaseLayer::division, tile("010332103")), None, "a kind the source names no path for");
}

#[test]
fn a_declaration_the_client_cannot_follow_is_refused_whole() {
    for bad in [
        r#"{ "top_zoom": 5, "format": { "terrain": 7 }, "shard": {} }"#,
        r#"{ "top_zoom": 5, "path": { "terrain": "a/b" }, "shard": {} }"#,
        r#"{ "top_zoom": 5, "shard": { "010330012": {} } }"#,
        r#"{ "top_zoom": 4, "shard": {} }"#,
        r#"{ "top_zoom": 5 }"#,
    ] {
        assert_eq!(parse(bad).map(|_| ()), None, "{bad}");
    }
}
