// pxl-geo-bake/poi/bake.test.rs

use super::*;
use pxl_spec::poi::wire;

#[test]
fn coincident_population_is_preserved_with_constant_hierarchy_size() {
    let root = xy::parse("132110320000000").unwrap();
    let b = xy::bounds(root);
    let mut tree = Hierarchy::new(root, 7).unwrap();
    for i in 0..100_000 {
        let entity = Entity {
            id: format!("{i:08}"),
            position: [b[0] + 12.0, b[1] + 23.0, 700.0],
            text: "annotation".into(),
        };
        assert!(tree.add(entity.clone()).unwrap());
        assert!(!tree.add(entity).unwrap());
    }
    let nodes = tree.finish();
    assert_eq!(nodes.len(), 1);
    assert!(!nodes[0].summary.refinable);
    for n in &nodes {
        assert_eq!(n.summary.count, 100_000);
        assert_eq!(wire::decode(&wire::encode(n).unwrap()).unwrap(), *n);
    }
}

#[test]
fn complete_children_partition_the_population_and_empty_root_is_explicit() {
    let root = xy::parse("132110320000000").unwrap();
    let mut tree = Hierarchy::new(root, 1).unwrap();
    for (i, key) in xy::children(root).into_iter().enumerate() {
        let b = xy::bounds(key);
        tree.add(Entity {
            id: i.to_string(),
            position: [b[0] + 1.0, b[1] + 1.0, i as f64 * 100.0],
            text: String::new(),
        })
        .unwrap();
    }
    let nodes = tree.finish();
    assert_eq!(nodes[0].summary.count, 4);
    assert_eq!(nodes[0].children.iter().map(|s| s.count).sum::<u64>(), 4);
    for node in nodes {
        wire::validate(&node).unwrap();
    }
    let empty = Hierarchy::new(root, 2).unwrap().finish();
    assert_eq!(empty.len(), 1);
    assert_eq!(empty[0].summary.count, 0);
    wire::validate(&empty[0]).unwrap();
}

#[test]
fn conflicting_identity_and_unresolved_position_refuse_publication() {
    let root = xy::parse("132110320000000").unwrap();
    let b = xy::bounds(root);
    let mut tree = Hierarchy::new(root, 1).unwrap();
    let e = Entity {
        id: "a".into(),
        position: [b[0], b[1], 1.0],
        text: "one".into(),
    };
    tree.add(e.clone()).unwrap();
    assert!(tree.add(Entity { text: "two".into(), ..e }).is_err());
    assert!(tree
        .add(Entity {
            id: "b".into(),
            position: [b[0], b[1], f64::NAN],
            text: String::new()
        })
        .is_err());
}

#[test]
fn spatial_resolution_reaches_beyond_two_levels_without_detail_expansion() {
    let root = xy::parse("132110320000000").unwrap();
    let b = xy::bounds(root);
    let mut tree = Hierarchy::new(root, 1).unwrap();
    for (i, x) in [0.25, 0.5, 2.25].into_iter().enumerate() {
        tree.add(Entity {
            id: i.to_string(),
            position: [b[0] + x, b[1] + 0.25, 50.0],
            text: String::new(),
        })
        .unwrap();
    }
    let nodes = tree.finish();
    assert!(nodes.iter().any(|n| xy::depth(n.summary.key) == Some(xy::MAX_DEPTH)));
    let terminals: Vec<_> = nodes.iter().filter(|n| !n.summary.refinable).collect();
    assert_eq!(terminals.iter().map(|n| n.summary.count).sum::<u64>(), 3);
    assert!(terminals.iter().any(|n| n.summary.count == 2), "transport resolution does not bound population");
    for n in &nodes {
        wire::validate(n).unwrap();
        assert!(wire::encode(n).unwrap().len() <= wire::MAX_PACKET_BYTES);
    }
}

#[test]
fn a_vertical_stack_keeps_its_full_count_and_xyz_bounds() {
    let root = xy::parse("132110320000000").unwrap();
    let b = xy::bounds(root);
    let mut tree = Hierarchy::new(root, 1).unwrap();
    for (i, z) in [-500.0, 0.0, 500.0].into_iter().enumerate() {
        tree.add(Entity {
            id: i.to_string(),
            position: [b[0], b[1], z],
            text: String::new(),
        })
        .unwrap();
    }
    let nodes = tree.finish();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].summary.count, 3);
    assert_eq!(nodes[0].summary.bounds, [b[0], b[1], -500.0, b[0], b[1], 500.0]);
    assert!(!nodes[0].summary.refinable);
    wire::validate(&nodes[0]).unwrap();
}
