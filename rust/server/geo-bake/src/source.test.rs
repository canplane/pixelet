// pxl-geo-bake/source.test.rs

use super::*;

fn kind(names: &[&str]) -> Kind {
    Kind {
        format: 1,
        version: "v1".into(),
        shard: Some(tiles(names.iter().copied()).expect("shard names")),
    }
}

fn kinds(each: impl Fn(BaseKind) -> Kind) -> KindTable {
    try_kinds(|l| Ok(each(l))).expect("a kind each")
}

#[test]
fn shard_served_when_all_kinds() {
    let all = ["01033000", "01033001", "01033002", "01033003", "01033010"];
    let kinds = kinds(|l| match l {
        BaseKind::Division => kind(&all[..4]),
        BaseKind::Earth => Kind {
            shard: None,
            ..kind(&[])
        },
        _ => kind(&all),
    });
    let shard = &decl(MIN_ZOOM, &kinds, &OverrideMap::new()).expect("a declaration")["shard"];
    let keys: Vec<&str> = shard
        .as_object()
        .expect("a map")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["0103300"]);
}

#[test]
fn unread_keys_unwritten() {
    assert!(tiles(["010330001"]).is_err(), "finer than a shard");
    let shards = || Some(tiles(["01033000"]).expect("a shard"));
    let kinds = kinds(|l| Kind {
        format: 1,
        version: if l == BaseKind::Terrain { "a/b" } else { "v1" }.into(),
        shard: shards().filter(|_| l != BaseKind::Earth),
    });
    assert!(
        decl(MIN_ZOOM, &kinds, &OverrideMap::new()).is_err(),
        "a version no segment"
    );
    let earth_sharded = try_kinds(|_| {
        Ok(Kind {
            format: 1,
            version: "v1".into(),
            shard: shards(),
        })
    })
    .expect("a kind each");
    assert!(
        decl(MIN_ZOOM, &earth_sharded, &OverrideMap::new()).is_err(),
        "the Earth's picture held by shards"
    );
}

#[test]
fn override_names_shard() {
    let kinds = kinds(|l| match l {
        BaseKind::Earth => Kind {
            shard: None,
            ..kind(&[])
        },
        _ => kind(&["01033000", "01033001", "01033002", "01033003"]),
    });
    let at = tiles(["01033001"])
        .expect("a name")
        .pop_first()
        .expect("one");
    let overrides =
        OverrideMap::from([(at, vec![(BaseKind::Place, "v10.2026-10-12.1".to_string())])]);
    let shard = &decl(MIN_ZOOM, &kinds, &overrides).expect("a declaration")["shard"];
    assert_eq!(shard["0103300"], json!({}));
    assert_eq!(
        shard["01033001"],
        json!({ "version": { "place": "v10.2026-10-12.1" } })
    );
}

#[test]
fn partial_shard_override_refused() {
    let kinds = kinds(|l| match l {
        BaseKind::Earth => Kind {
            shard: None,
            ..kind(&[])
        },
        BaseKind::Division => kind(&["01033000"]),
        _ => kind(&["01033000", "01033001"]),
    });
    for name in ["01033001", "0103300"] {
        let at = key(name).expect("a key");
        let overrides =
            OverrideMap::from([(at, vec![(BaseKind::Place, "v10.2026-10-12.1".to_string())])]);
        assert!(decl(MIN_ZOOM, &kinds, &overrides).is_err(), "{name}");
    }
}
