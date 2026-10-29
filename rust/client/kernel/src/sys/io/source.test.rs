// pxl-kernel/sys/io/source.test.rs

use super::*;

fn tile(name: &str) -> Tile {
    Tile::of_name(name).expect("a tile's name")
}

/// The formats this client reads, by each kind's bake number: the published
/// table's (pxl-spec bake/published.json `FORMAT`), never copied.
fn formats() -> Vec<u32> {
    BaseLayer::ALL.into_iter().map(|l| FORMAT.of(l)).collect()
}

// Read off the source parsed, never declared: a declaration is the process's
// once, and the world's tests walk ground no shard of this one serves.
#[test]
fn src_serves_by_shard() {
    let s = parse(4, &formats(), ["0103300", "01033210"]).expect("a declaration the client reads");
    for served in [
        "01033",
        "0103300",
        "010330012",
        "010332",
        "01033210",
        "010332103",
    ] {
        assert!(s.covers(tile(served)), "{served}");
    }
    for beside in ["01032", "0103301", "01033211", "010332110"] {
        assert!(!s.covers(tile(beside)), "{beside}");
    }
}

#[test]
fn unfollowable_declaration_refused() {
    let f = formats();
    // A terrain format past the one this client reads, and none for the Earth.
    let mut unread = f.clone();
    unread[BaseLayer::terrain as usize] += 1;
    let short = &f[..f.len() - 1];
    assert!(parse(4, &unread, []).is_none(), "an unread format");
    assert!(parse(4, short, []).is_none(), "a kind with no format");
    assert!(
        parse(4, &f, ["010330012"]).is_none(),
        "a name finer than a shard"
    );
    assert!(parse(3, &f, []).is_none(), "a zoom above the pyramid");
    assert!(parse(16, &f, []).is_none(), "a zoom below the leaves");
}
