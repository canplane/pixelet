// pxl-core/sys/io/source.test.rs

use super::*;

fn tile(name: &str) -> Quadrant {
    Quadrant::of_key(name).expect("a tile's name")
}

/// The formats this client reads, by each kind's bake number, as the
/// published table gives them (pxl-spec bake/publication.json `FORMAT`).
fn formats() -> Vec<u32> {
    BaseLayer::ALL.into_iter().map(BaseLayer::format).collect()
}

// The test reads a parsed source and never declares it. A declaration is
// made once in a process, and the world's tests walk ground that no shard of
// this source serves.
#[test]
fn src_serves_by_shard() {
    crate::on_core(|| {
        let s =
            parse(4, &formats(), ["0103300", "01033210"]).expect("a declaration the client reads");
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
    });
}

#[test]
fn unfollowable_decl_refused() {
    crate::on_core(|| {
        let f = formats();
        // One declaration gives terrain a format past the one this client reads,
        // and another gives the Earth no format.
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
    });
}
