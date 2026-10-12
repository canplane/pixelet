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

// One test declares: a declaration is the kernel's once.
#[test]
fn a_declared_source_serves_and_names_paths_by_its_shards_and_is_not_declared_again() {
    declare(DECLARED).expect("a declaration the client reads");
    for served in ["01033", "0103300", "010330012", "010332", "01033210", "010332103"] {
        assert!(serves(BaseLayer::terrain, tile(served)), "{served}");
    }
    for beside in ["01032", "0103301", "01033211", "010332110"] {
        assert!(!serves(BaseLayer::terrain, tile(beside)), "{beside}");
    }
    assert!(serves(BaseLayer::earth, tile("01032")), "the Earth's picture is no shard's");
    assert_eq!(version(BaseLayer::place, tile("010332103")), "v10.2026-10-12.1");
    assert_eq!(version(BaseLayer::place, tile("010332")), "v10.2026-10-09.1", "over the shard is the source's");
    assert_eq!(version(BaseLayer::terrain, tile("010332103")), "v8.2026-10-09.1");
    assert_eq!(declare(DECLARED), Err(Errno::EEXIST));
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
