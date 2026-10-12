// pxl-kernel/io/source.test.rs

use super::*;

fn tile(name: &str) -> Tile {
    Tile::of_name(name).expect("a tile's name")
}

const DECLARED: &str = r#"{
  "top_zoom": 5,
  "format": { "terrain": 8, "place": 10, "division": 7, "earth": 1 },
  "version": { "terrain": "v8.2026-10-09.1", "place": "v10.2026-10-09.1", "division": "v7.2026-10-09.1", "earth": "v1.2026-10-05.1" },
  "shard": { "0103300": {}, "01033210": { "version": { "place": "v10.2026-10-12.1" } } }
}"#;

// Read off the source parsed, never declared: a declaration is the process's
// once, and the world's tests walk ground no shard of this one serves.
#[test]
fn a_source_serves_and_names_versions_by_its_shards() {
    let s = parse(DECLARED).expect("a declaration the client reads");
    for served in ["01033", "0103300", "010330012", "010332", "01033210", "010332103"] {
        assert!(s.covers(tile(served)), "{served}");
    }
    for beside in ["01032", "0103301", "01033211", "010332110"] {
        assert!(!s.covers(tile(beside)), "{beside}");
    }
    assert_eq!(s.version_of(BaseLayer::place, tile("010332103")), "v10.2026-10-12.1");
    assert_eq!(s.version_of(BaseLayer::place, tile("010332")), "v10.2026-10-09.1", "over the shard is the source's");
    assert_eq!(s.version_of(BaseLayer::terrain, tile("010332103")), "v8.2026-10-09.1");
    assert_eq!(s.version_of(BaseLayer::earth, tile("01033")), "v1.2026-10-05.1", "the Earth's picture is no shard's");
}

#[test]
fn a_declaration_the_client_cannot_follow_is_refused_whole() {
    for bad in [
        r#"{ "top_zoom": 5, "format": { "terrain": 7, "place": 10, "division": 7, "earth": 1 }, "version": { "terrain": "t", "place": "p", "division": "d", "earth": "e" }, "shard": {} }"#,
        r#"{ "top_zoom": 5, "format": { "terrain": 8, "place": 10, "division": 7 }, "version": { "terrain": "t", "place": "p", "division": "d", "earth": "e" }, "shard": {} }"#,
        r#"{ "top_zoom": 5, "format": { "terrain": 8, "place": 10, "division": 7, "earth": 1 }, "version": { "terrain": "a/b", "place": "p", "division": "d", "earth": "e" }, "shard": {} }"#,
        r#"{ "top_zoom": 5, "format": { "terrain": 8, "place": 10, "division": 7, "earth": 1 }, "version": { "terrain": "t", "place": "p", "division": "d" }, "shard": {} }"#,
        r#"{ "top_zoom": 5, "format": { "terrain": 8, "place": 10, "division": 7, "earth": 1 }, "version": { "terrain": "t", "place": "p", "division": "d", "earth": "e" }, "shard": { "010330012": {} } }"#,
        r#"{ "top_zoom": 4, "format": { "terrain": 8, "place": 10, "division": 7, "earth": 1 }, "version": { "terrain": "t", "place": "p", "division": "d", "earth": "e" }, "shard": {} }"#,
        r#"{ "top_zoom": 5, "format": { "terrain": 8, "place": 10, "division": 7, "earth": 1 }, "version": { "terrain": "t", "place": "p", "division": "d", "earth": "e" } }"#,
    ] {
        assert_eq!(parse(bad).map(|_| ()), None, "{bad}");
    }
}
