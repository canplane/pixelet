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
    assert_eq!(fs::read_dir(completed.join("tile")).unwrap().count(), 2);
    let page = pxl_spec::poi::detail::decode(&crate::util::unpack(&fs::read(completed.join("tile").join(pxl_spec::poi::detail::name(root, 0))).unwrap()).unwrap()).unwrap();
    assert_eq!(page.records, vec![entity]);
    assert!(completed.join("source.jsonl").is_file());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn coincident_details_page_without_truncating_membership_or_original_text() {
    use pxl_spec::poi::detail;
    let dir = std::env::temp_dir().join(format!("pxl-detail-pages-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let root = xy::parse("132110320123012").unwrap();
    let b = xy::bounds(root);
    write(&dir, root, 9, "test", Level::Live, |accept| {
        for i in 0..37 {
            accept(
                Entity {
                    id: format!("{i:04}"),
                    position: [b[0], b[1], i as f64],
                    text: format!("강남 장소 {i} {}", "가".repeat(1500)),
                },
                "source",
            )?;
        }
        Ok(())
    })
    .unwrap();
    let mut offset = 0;
    let mut ids = Vec::new();
    loop {
        let bytes = crate::util::unpack(&fs::read(dir.join("tile").join(detail::name(root, offset))).unwrap()).unwrap();
        assert!(bytes.len() <= detail::MAX_PACKET_BYTES);
        let page = detail::decode(&bytes).unwrap();
        assert_eq!(page.total, 37);
        assert_eq!(page.offset, offset);
        assert!(page.records.len() <= detail::PAGE_RECORDS);
        for record in &page.records {
            assert!(record.text.ends_with(&"가".repeat(1500)));
            ids.push(record.id.clone());
        }
        let Some(next) = page.next() else { break };
        offset = next;
    }
    assert_eq!(ids, (0..37).map(|i| format!("{i:04}")).collect::<Vec<_>>());
    fs::remove_dir_all(dir).unwrap();
}
