// pxl-kernel/io/source.test.rs

use super::*;

fn tile(name: &str) -> Tile {
    Tile::of_name(name).expect("a tile's name")
}

/// The formats this client reads, as a declaration names them: the
/// published table's (pxl-spec bake/published.json `FORMAT`), never copied.
fn formats() -> String {
    format!(
        r#"{{ "terrain": {}, "place": {}, "division": {}, "earth": {} }}"#,
        FORMAT.terrain, FORMAT.place, FORMAT.division, FORMAT.earth
    )
}

fn declared() -> String {
    format!(
        r#"{{
  "top_zoom": 4,
  "format": {},
  "version": {{ "terrain": "v10.2026-10-09.1", "place": "v10.2026-10-09.1", "division": "v7.2026-10-09.1", "earth": "v1.2026-10-05.1" }},
  "shard": {{ "0103300": {{}}, "01033210": {{ "version": {{ "place": "v10.2026-10-12.1" }} }} }}
}}"#,
        formats()
    )
}

// Read off the source parsed, never declared: a declaration is the process's
// once, and the world's tests walk ground no shard of this one serves.
#[test]
fn a_source_serves_and_names_versions_by_its_shards() {
    let s = parse(&declared()).expect("a declaration the client reads");
    for served in ["01033", "0103300", "010330012", "010332", "01033210", "010332103"] {
        assert!(s.covers(tile(served)), "{served}");
    }
    for beside in ["01032", "0103301", "01033211", "010332110"] {
        assert!(!s.covers(tile(beside)), "{beside}");
    }
    assert_eq!(s.version_of(BaseLayer::place, tile("010332103")), "v10.2026-10-12.1");
    assert_eq!(s.version_of(BaseLayer::place, tile("010332")), "v10.2026-10-09.1", "over the shard is the source's");
    assert_eq!(s.version_of(BaseLayer::terrain, tile("010332103")), "v10.2026-10-09.1");
    assert_eq!(s.version_of(BaseLayer::earth, tile("01033")), "v1.2026-10-05.1", "the Earth's picture is no shard's");
}

#[test]
fn a_declaration_the_client_cannot_follow_is_refused_whole() {
    let f = formats();
    // A terrain format past the one this client reads, and none for the Earth.
    let (unread, short) = (
        format!(
            r#"{{ "terrain": {}, "place": {}, "division": {}, "earth": {} }}"#,
            FORMAT.terrain + 1,
            FORMAT.place,
            FORMAT.division,
            FORMAT.earth
        ),
        format!(r#"{{ "terrain": {}, "place": {}, "division": {} }}"#, FORMAT.terrain, FORMAT.place, FORMAT.division),
    );
    for bad in [
        format!(r#"{{ "top_zoom": 4, "format": {unread}, "version": {{ "terrain": "t", "place": "p", "division": "d", "earth": "e" }}, "shard": {{}} }}"#),
        format!(r#"{{ "top_zoom": 4, "format": {short}, "version": {{ "terrain": "t", "place": "p", "division": "d", "earth": "e" }}, "shard": {{}} }}"#),
        format!(r#"{{ "top_zoom": 4, "format": {f}, "version": {{ "terrain": "a/b", "place": "p", "division": "d", "earth": "e" }}, "shard": {{}} }}"#),
        format!(r#"{{ "top_zoom": 4, "format": {f}, "version": {{ "terrain": "t", "place": "p", "division": "d" }}, "shard": {{}} }}"#),
        format!(r#"{{ "top_zoom": 4, "format": {f}, "version": {{ "terrain": "t", "place": "p", "division": "d", "earth": "e" }}, "shard": {{ "010330012": {{}} }} }}"#),
        format!(r#"{{ "top_zoom": 3, "format": {f}, "version": {{ "terrain": "t", "place": "p", "division": "d", "earth": "e" }}, "shard": {{}} }}"#),
        format!(r#"{{ "top_zoom": 4, "format": {f}, "version": {{ "terrain": "t", "place": "p", "division": "d", "earth": "e" }} }}"#),
    ] {
        assert_eq!(parse(&bad).map(|_| ()), None, "{bad}");
    }
}
