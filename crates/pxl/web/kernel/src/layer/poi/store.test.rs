// pxl-kernel/layer/poi/store.test.rs

use super::*;

fn summary(key: u64, levels: u32) -> Summary {
    let b = xy::bounds(key);
    Summary {
        key,
        count: 100 * 4u64.pow(levels),
        bounds: [b[0], b[1], 0.0, b[2] - 0.000001, b[3] - 0.000001, 100.0],
        refinable: levels > 0,
    }
}
fn node(key: u64, levels: u32) -> Node {
    Node {
        revision: 1,
        summary: summary(key, levels),
        children: if levels == 0 {
            vec![]
        } else {
            xy::children(key).into_iter().map(|k| summary(k, levels - 1)).collect()
        },
    }
}
fn root(levels: u32) -> Node {
    node(xy::parse("132110320000000").unwrap(), levels)
}
fn all(_: u64, _: [f64; 6]) -> Option<Relevance> {
    Some(Relevance { distance: 0.0, refine: true })
}
fn limits(candidates: usize) -> Limits {
    Limits { candidates, ..LIMITS }
}
fn loaded(levels: u32) -> Store {
    let mut s = Store::default();
    s.select(true, &[(root(0).summary.key, 0.0)], all, LIMITS);
    s.adopt(root(0).summary.key, root(levels), LIMITS.resident_bytes).unwrap();
    s
}
fn subjects(s: &Store) -> Vec<u64> {
    s.wanted.iter().map(|r| r.subject).collect()
}
fn focus(key: u64) -> impl Fn(u64, [f64; 6]) -> Option<Relevance> {
    move |subject, _| (xy::contains(subject, key) || xy::contains(key, subject)).then_some(Relevance { distance: 0.0, refine: true })
}
fn assert_cut(s: &Store, limits: Limits) {
    assert!(s.cut.len() <= limits.candidates);
    assert!(s.bytes() + s.wanted.len() * NODE_RESERVATION <= limits.resident_bytes);
    for (i, a) in s.cut.iter().enumerate() {
        for b in &s.cut[i + 1..] {
            assert!(!xy::contains(a.key, b.key) && !xy::contains(b.key, a.key));
        }
    }
}

#[test]
fn delayed_invalid_or_over_budget_children_keep_the_whole_parent() {
    let mut s = loaded(2);
    s.select(true, &[(root(0).summary.key, 0.0)], all, limits(4));
    let parent = s.cut[0];
    assert_eq!(s.cut, root(2).children);
    assert!(subjects(&s).contains(&parent.key));
    let mut bad = node(parent.key, 1);
    bad.summary.bounds[2] += 1.0;
    assert!(s.adopt(parent.key, bad, LIMITS.resident_bytes).is_err());
    let mut incomplete = node(parent.key, 1);
    incomplete.children.pop();
    assert!(s.adopt(parent.key, incomplete, LIMITS.resident_bytes).is_err());
    s.select(true, &[(root(0).summary.key, 0.0)], all, limits(4));
    assert_eq!(s.cut, root(2).children);
    s.adopt(parent.key, node(parent.key, 1), LIMITS.resident_bytes).unwrap();
    s.select(true, &[(root(0).summary.key, 0.0)], all, limits(4));
    assert_eq!(s.cut, root(2).children, "no partial replacement to fill a spare slot");
    s.select(true, &[(root(0).summary.key, 0.0)], all, limits(7));
    assert_eq!(s.cut.iter().map(|s| s.count).sum::<u64>(), root(2).summary.count);
    assert!(!s.cut.contains(&parent));
    assert_cut(&s, limits(7));
}

#[test]
fn a_view_can_refine_to_transport_resolution_with_one_candidate() {
    let levels = xy::MAX_DEPTH - xy::ROOT_DEPTH;
    let mut s = loaded(levels);
    let leaf = (0..levels).fold(root(levels).summary.key, |key, _| key * 4 + 3);
    let policy = limits(1);
    for _ in 0..=levels {
        s.select(true, &[(root(0).summary.key, 0.0)], focus(leaf), policy);
        assert_cut(&s, policy);
        for key in subjects(&s) {
            let levels = xy::MAX_DEPTH - xy::depth(key).unwrap();
            s.adopt(key, node(key, levels), policy.resident_bytes).unwrap();
        }
    }
    s.select(true, &[(root(0).summary.key, 0.0)], focus(leaf), policy);
    assert_eq!(s.cut, vec![summary(leaf, 0)]);
    assert_eq!(s.cut[0].count, 100, "terminal membership is not expanded into entities");
    assert!(s.wanted.is_empty());
    assert_cut(&s, policy);
    s.select(true, &[], |_, _| None, policy);
    assert!(s.cut.is_empty());
    assert!(s.wanted.is_empty());
    let resident = s.node_count();
    s.select(true, &[(root(0).summary.key, 0.0)], focus(leaf), policy);
    assert_eq!(s.cut, vec![summary(leaf, 0)]);
    assert!(s.wanted.is_empty(), "returning to resident summaries needs no fetch");
    assert_eq!(s.node_count(), resident);
}

#[test]
fn irrelevant_children_use_no_slots_and_relevant_children_replace_together() {
    let mut s = loaded(1);
    let children = root(1).children;
    let assess = |key, _| (key != children[0].key).then_some(Relevance { distance: 0.0, refine: true });
    s.select(true, &[(root(0).summary.key, 0.0)], assess, limits(2));
    assert_eq!(s.cut, vec![root(1).summary]);
    s.select(true, &[(root(0).summary.key, 0.0)], assess, limits(3));
    assert_eq!(s.cut, children[1..]);
    assert_eq!(s.cut.iter().map(|s| s.count).sum::<u64>(), 300);
    assert_cut(&s, limits(3));
}

#[test]
fn relevance_is_hard_and_proximity_only_orders_relevant_refinement() {
    let mut s = loaded(2);
    let children = root(2).children;
    let hidden = children[0].key;
    let near = children[3].key;
    let policy = Limits {
        candidates: 5,
        resident_bytes: NODE_RESERVATION * 2,
    };
    let assess = |key, _| {
        if xy::contains(hidden, key) {
            return None;
        }
        Some(Relevance {
            distance: if xy::contains(near, key) { 1.0 } else { 100.0 },
            refine: true,
        })
    };
    s.select(true, &[(root(0).summary.key, 0.0)], assess, policy);
    assert_eq!(subjects(&s), vec![near]);
    s.adopt(near, node(near, 1), policy.resident_bytes).unwrap();
    s.select(true, &[(root(0).summary.key, 0.0)], assess, Limits { candidates: 6, ..policy });
    assert!(!s.cut.contains(&children[3]));
    assert!(s.cut.iter().all(|c| !xy::contains(hidden, c.key)));
    assert_cut(&s, Limits { candidates: 6, ..policy });
}

#[test]
fn new_view_evicts_old_branches_and_re_resolves_them_when_needed() {
    let mut s = loaded(2);
    let children = root(2).children;
    let policy = Limits {
        candidates: 4,
        resident_bytes: NODE_RESERVATION * 2,
    };
    for parent in [children[0], children[3], children[0]] {
        s.select(true, &[(root(0).summary.key, 0.0)], focus(parent.key), policy);
        assert_eq!(s.cut, vec![parent]);
        assert_eq!(subjects(&s), vec![parent.key]);
        assert_cut(&s, policy);
        s.adopt(parent.key, node(parent.key, 1), policy.resident_bytes).unwrap();
        s.select(true, &[(root(0).summary.key, 0.0)], focus(parent.key), policy);
        assert_eq!(s.cut, node(parent.key, 1).children);
        assert_cut(&s, policy);
    }
}

#[test]
fn current_view_withdraws_obsolete_requests_and_late_packets_cannot_repopulate_it() {
    let mut s = loaded(2);
    let children = root(2).children;
    s.select(true, &[(root(0).summary.key, 0.0)], focus(children[0].key), limits(4));
    assert_eq!(subjects(&s), vec![children[0].key]);
    s.select(true, &[(root(0).summary.key, 0.0)], focus(children[3].key), limits(4));
    assert_eq!(subjects(&s), vec![children[3].key]);
    let before = s.bytes();
    s.adopt(children[0].key, node(children[0].key, 1), LIMITS.resident_bytes).unwrap();
    assert_eq!(s.bytes(), before);
    assert_eq!(s.cut, vec![children[3]]);
    s.select(true, &[], |_, _| None, LIMITS);
    assert!(s.cut.is_empty() && s.wanted.is_empty());
    s.select(true, &[(root(0).summary.key, 0.0)], all, limits(0));
    assert!(s.cut.is_empty() && s.wanted.is_empty());
}

#[test]
fn a_complete_cut_reconsiders_capacity_after_other_branches_disappear() {
    let mut s = loaded(2);
    let children = root(2).children;
    s.nodes.extend(children.iter().map(|c| node(c.key, 1)));
    let kept = node(children[0].key, 1).children;
    let assess = |key, _| {
        let relevant = key == root(2).summary.key || children.iter().any(|c| c.key == key) || kept.iter().any(|c| c.key == key);
        relevant.then_some(Relevance { distance: key as f64, refine: true })
    };
    s.select(true, &[(root(0).summary.key, 0.0)], assess, limits(4));
    assert_eq!(s.cut, kept, "a formerly blocked branch must use the slots the current view freed");
    assert_cut(&s, limits(4));
}

#[test]
fn region_discovery_rejects_wrong_addresses_and_revision_changes_resolve_only_that_region_again() {
    let mut s = Store::default();
    let a = root(2);
    let b = node(a.summary.key + 1, 2);
    let roots = [(a.summary.key, 0.0), (b.summary.key, 1.0)];
    s.select(true, &roots, all, LIMITS);
    assert!(s.adopt(a.summary.key, b.clone(), LIMITS.resident_bytes).is_err());
    s.adopt(a.summary.key, a.clone(), LIMITS.resident_bytes).unwrap();
    s.adopt(b.summary.key, b.clone(), LIMITS.resident_bytes).unwrap();
    s.select(true, &roots, all, LIMITS);
    let key = subjects(&s)[0];
    let mut updated = node(key, 1);
    updated.revision = 2;
    assert!(s.adopt(key, updated, LIMITS.resident_bytes).is_err());
    assert!(s.nodes.iter().all(|n| !xy::contains(a.summary.key, n.summary.key)));
    assert!(s.nodes.iter().any(|n| n.summary.key == b.summary.key));
    s.select(true, &roots, all, LIMITS);
    assert!(subjects(&s).contains(&a.summary.key));
    assert!(!subjects(&s).contains(&b.summary.key));
    assert_cut(&s, LIMITS);
}

fn refine_shared_corner(half_width: f64, expected: usize) {
    let levels = xy::MAX_DEPTH - xy::ROOT_DEPTH;
    let base = root(levels).summary.key;
    let unit = (xy::bounds(base)[2] - xy::bounds(base)[0]) / (1u64 << levels) as f64;
    let half_width = half_width * unit / 2.0;
    let roots: Vec<_> = (0..4).map(|i| (base + i, i as f64)).collect();
    let corner = xy::bounds(base);
    let (x, y) = (corner[2], corner[1]);
    let visible = |_: u64, b: [f64; 6]| (b[0] <= x + half_width && b[3] >= x - half_width && b[1] <= y + unit * 0.55 && b[4] >= y - unit * 0.55).then_some(Relevance { distance: 0.0, refine: true });
    let mut s = Store::default();
    // Deliver just one response each turn: eviction must not cancel a deeper
    // request merely because its ancestors no longer reside in the cache.
    for _ in 0..100 {
        s.select(true, &roots, visible, LIMITS);
        assert_cut(&s, LIMITS);
        let Some(key) = subjects(&s).first().copied() else { break };
        s.adopt(key, node(key, xy::MAX_DEPTH - xy::depth(key).unwrap()), LIMITS.resident_bytes).unwrap();
    }
    s.select(true, &roots, visible, LIMITS);
    assert_eq!(s.cut.len(), expected);
    assert!(s.cut.iter().all(|n| xy::depth(n.key) == Some(xy::MAX_DEPTH)));
    assert!(s.wanted.is_empty());
    assert!(s.node_count() < 4 * 8, "whole ancestor paths must not be resident");
    assert_cut(&s, LIMITS);
    // Zooming out must restore coarse membership, not keep a partial fine cut.
    s.select(true, &roots, |_, _| Some(Relevance { distance: 0.0, refine: false }), LIMITS);
    assert_eq!(s.cut, roots.iter().map(|(k, _)| summary(*k, levels)).collect::<Vec<_>>());
}

#[test]
fn four_regions_refine_at_their_shared_corner_without_pinning_ancestry() {
    refine_shared_corner(1.1, 4);
}

#[test]
fn skipping_a_missing_ancestor_cannot_hide_a_newly_visible_sibling() {
    let mut s = loaded(3);
    let parent = root(3).children[0];
    let deep = node(parent.key, 2).children[0];
    s.cut = vec![deep];
    s.nodes.push(node(deep.key, 1));
    // All children are visible again. A deep cache hit cannot stand for the
    // unknown rest of its ancestor; keep the parent and resolve its packet.
    s.select(true, &[(root(0).summary.key, 0.0)], all, limits(4));
    assert_eq!(s.cut, root(3).children);
    assert!(subjects(&s).contains(&parent.key));
    assert_cut(&s, limits(4));
}

#[test]
fn resolving_an_evicted_root_discards_descendants_of_its_old_revision() {
    let mut s = loaded(2);
    let root_key = root(2).summary.key;
    let child = node(root(2).children[0].key, 1);
    s.nodes = vec![child];
    s.select(true, &[(root_key, 0.0)], all, LIMITS);
    let mut fresh = root(2);
    fresh.revision = 2;
    s.adopt(root_key, fresh, LIMITS.resident_bytes).unwrap();
    assert!(s.nodes.iter().all(|n| n.revision == 2));
}

#[test]
fn a_complete_visible_frontier_can_skip_ancestors_without_partial_replacement() {
    refine_shared_corner(2.5, 8);
}

#[test]
fn a_resident_far_branch_cannot_spend_the_slots_of_a_pending_near_refinement() {
    let mut s = loaded(2);
    let root = root(2);
    let near = root.children[0].key;
    let far = root.children[3].key;
    s.nodes.push(node(far, 1));
    let assess = |key, _| {
        Some(Relevance {
            distance: if xy::contains(near, key) { 0.0 } else { 100.0 },
            refine: true,
        })
    };
    let policy = limits(7);
    s.select(true, &[(root.summary.key, 0.0)], assess, policy);
    assert_eq!(s.cut, root.children);
    assert_eq!(subjects(&s), vec![near]);
    s.adopt(near, node(near, 1), policy.resident_bytes).unwrap();
    s.select(false, &[(root.summary.key, 0.0)], assess, policy);
    assert!(node(near, 1).children.iter().all(|child| s.cut.contains(child)));
    assert!(s.cut.iter().any(|summary| summary.key == far));
    assert_eq!(s.cut.iter().map(|summary| summary.count).sum::<u64>(), root.summary.count);
    assert_cut(&s, policy);
}

#[test]
fn delayed_region_roots_have_candidate_slots_before_their_responses_arrive() {
    let a = root(2);
    let b = node(a.summary.key + 1, 2);
    let roots = [(a.summary.key, 0.0), (b.summary.key, 1.0)];
    let policy = limits(4);
    let mut s = Store::default();
    s.select(true, &roots, all, policy);
    s.adopt(a.summary.key, a.clone(), policy.resident_bytes).unwrap();
    s.select(false, &roots, all, policy);
    assert_eq!(s.cut, vec![a.summary]);
    assert_cut(&s, policy);
    s.adopt(b.summary.key, b.clone(), policy.resident_bytes).unwrap();
    for _ in 0..4 {
        s.select(false, &roots, all, policy);
        assert_eq!(s.cut, vec![a.summary, b.summary]);
        assert_cut(&s, policy);
        assert!(s.wanted.is_empty());
    }
}

#[test]
fn a_resolved_region_with_no_visible_children_is_not_reintroduced_as_a_coarse_candidate() {
    let a = root(2);
    let b = node(a.summary.key + 1, 2);
    let roots = [(a.summary.key, 0.0), (b.summary.key, 1.0)];
    let policy = limits(4);
    let assess = |key, _| {
        if key != b.summary.key && xy::contains(b.summary.key, key) {
            return None;
        }
        Some(Relevance {
            distance: if key == b.summary.key { 100.0 } else { 0.0 },
            refine: true,
        })
    };
    let mut s = Store::default();
    s.select(true, &roots, assess, policy);
    s.adopt(a.summary.key, a.clone(), policy.resident_bytes).unwrap();
    s.adopt(b.summary.key, b.clone(), policy.resident_bytes).unwrap();
    for _ in 0..8 {
        s.select(false, &roots, assess, policy);
        assert_eq!(s.cut, a.children);
        assert_cut(&s, policy);
    }
}
