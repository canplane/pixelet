// pxl-geo-bake/poi/snapshot.test.rs

use super::*;

#[test]
fn backing_and_nodes_are_published_together_and_source_failure_publishes_nothing() {
    let dir = std::env::temp_dir().join(format!("pxl-snapshot-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let root = xy::parse("132110320123012").unwrap();
    let b = xy::bounds(root);
    let entity = Entity {
        id: "one".into(),
        position: [b[0] + 1.0, b[1] + 1.0, 123.0],
        text: "original text".into(),
    };
    let failed = dir.join("failed");
    assert!(write(&failed, root, 1, "test", Level::Live, |accept| {
        accept(entity.clone(), "provider-id")?;
        Err("source interrupted".into())
    })
    .is_err());
    assert!(!failed.exists());
    let completed = dir.join("completed");
    write(&completed, root, 2, "test", Level::Live, |accept| {
        accept(entity.clone(), "provider-id")?;
        accept(entity.clone(), "provider-id")
    })
    .unwrap();
    let backing = fs::read_to_string(completed.join("canonical.jsonl")).unwrap();
    assert_eq!(backing.lines().count(), 1);
    assert_eq!(serde_json::from_str::<Entity>(backing.trim()).unwrap(), entity);
    let node = wire::decode(&crate::util::unpack(&fs::read(completed.join("tile").join(xy::name(root))).unwrap()).unwrap()).unwrap();
    assert_eq!(node.summary.count, 1);
    assert_eq!(node.summary.bounds[2], 123.0);
    assert!(!node.summary.refinable);
    assert_eq!(fs::read_dir(completed.join("tile")).unwrap().count(), 1);
    assert!(completed.join("source.jsonl").is_file());
    fs::remove_dir_all(dir).unwrap();
}
