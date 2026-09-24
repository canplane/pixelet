// pxl-kernel/layer/poi/store.test.rs

use super::*;

fn summary(key: u64, levels: u32) -> Summary {
    let b = xy::bounds(key);
    Summary {
        key,
        count: 100 * 4u64.pow(levels),
        bounds: [b[0], b[1], 0.0, b[2] - 0.01, b[3] - 0.01, 100.0],
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
fn all(_: &Summary) -> Option<Relevance> {
    Some(Relevance { distance: 0.0, refine: true })
}
fn limits(candidates: usize) -> Limits {
    Limits { candidates, ..LIMITS }
}
fn loaded(levels: u32) -> Store {
    let mut s = Store::default();
    s.select(&[(root(0).summary.key, 0.0)], all, LIMITS);
    s.adopt(root(0).summary.key, root(levels), LIMITS.resident_bytes).unwrap();
    s
}
fn subjects(s: &Store) -> Vec<u64> {
    s.wanted.iter().map(|r| r.subject).collect()
}
fn focus(key: u64) -> impl Fn(&Summary) -> Option<Relevance> {
    move |s| (xy::contains(s.key, key) || xy::contains(key, s.key)).then_some(Relevance { distance: 0.0, refine: true })
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
    s.select(&[(root(0).summary.key, 0.0)], all, limits(4));
    let parent = s.cut[0];
    assert_eq!(s.cut, root(2).children);
    assert!(subjects(&s).contains(&parent.key));
    let mut bad = node(parent.key, 1);
    bad.summary.bounds[2] += 1.0;
    assert!(s.adopt(parent.key, bad, LIMITS.resident_bytes).is_err());
    let mut incomplete = node(parent.key, 1);
    incomplete.children.pop();
    assert!(s.adopt(parent.key, incomplete, LIMITS.resident_bytes).is_err());
    s.select(&[(root(0).summary.key, 0.0)], all, limits(4));
    assert_eq!(s.cut, root(2).children);
    s.adopt(parent.key, node(parent.key, 1), LIMITS.resident_bytes).unwrap();
    s.select(&[(root(0).summary.key, 0.0)], all, limits(4));
    assert_eq!(s.cut, root(2).children, "no partial replacement to fill a spare slot");
    s.select(&[(root(0).summary.key, 0.0)], all, limits(7));
    assert_eq!(s.cut.iter().map(|s| s.count).sum::<u64>(), root(2).summary.count);
    assert!(!s.cut.contains(&parent));
    assert_cut(&s, limits(7));
}

#[test]
fn a_view_can_refine_to_transport_resolution_with_one_candidate() {
    let mut s = loaded(8);
    let leaf = (0..8).fold(root(8).summary.key, |key, _| key * 4 + 3);
    let policy = limits(1);
    for _ in 0..9 {
        s.select(&[(root(0).summary.key, 0.0)], focus(leaf), policy);
        assert_cut(&s, policy);
        for key in subjects(&s) {
            let levels = xy::MAX_DEPTH - xy::depth(key).unwrap();
            s.adopt(key, node(key, levels), policy.resident_bytes).unwrap();
        }
    }
    s.select(&[(root(0).summary.key, 0.0)], focus(leaf), policy);
    assert_eq!(s.cut, vec![summary(leaf, 0)]);
    assert_eq!(s.cut[0].count, 100, "terminal membership is not expanded into entities");
    assert!(s.wanted.is_empty());
    assert_cut(&s, policy);
    s.select(&[], |_| None, policy);
    assert!(s.cut.is_empty());
    assert!(s.wanted.is_empty());
    let resident = s.node_count();
    s.select(&[(root(0).summary.key, 0.0)], focus(leaf), policy);
    assert_eq!(s.cut, vec![summary(leaf, 0)]);
    assert!(s.wanted.is_empty(), "returning to resident summaries needs no fetch");
    assert_eq!(s.node_count(), resident);
}

#[test]
fn irrelevant_children_use_no_slots_and_relevant_children_replace_together() {
    let mut s = loaded(1);
    let children = root(1).children;
    let assess = |s: &Summary| (s.key != children[0].key).then_some(Relevance { distance: 0.0, refine: true });
    s.select(&[(root(0).summary.key, 0.0)], assess, limits(2));
    assert_eq!(s.cut, vec![root(1).summary]);
    s.select(&[(root(0).summary.key, 0.0)], assess, limits(3));
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
    let assess = |s: &Summary| {
        if xy::contains(hidden, s.key) {
            return None;
        }
        Some(Relevance {
            distance: if xy::contains(near, s.key) { 1.0 } else { 100.0 },
            refine: true,
        })
    };
    s.select(&[(root(0).summary.key, 0.0)], assess, policy);
    assert_eq!(subjects(&s), vec![near]);
    s.adopt(near, node(near, 1), policy.resident_bytes).unwrap();
    s.select(&[(root(0).summary.key, 0.0)], assess, Limits { candidates: 6, ..policy });
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
        s.select(&[(root(0).summary.key, 0.0)], focus(parent.key), policy);
        assert_eq!(s.cut, vec![parent]);
        assert_eq!(subjects(&s), vec![parent.key]);
        assert_cut(&s, policy);
        s.adopt(parent.key, node(parent.key, 1), policy.resident_bytes).unwrap();
        s.select(&[(root(0).summary.key, 0.0)], focus(parent.key), policy);
        assert_eq!(s.cut, node(parent.key, 1).children);
        assert_cut(&s, policy);
    }
}

#[test]
fn current_view_withdraws_obsolete_requests_and_late_packets_cannot_repopulate_it() {
    let mut s = loaded(2);
    let children = root(2).children;
    s.select(&[(root(0).summary.key, 0.0)], focus(children[0].key), limits(4));
    assert_eq!(subjects(&s), vec![children[0].key]);
    s.select(&[(root(0).summary.key, 0.0)], focus(children[3].key), limits(4));
    assert_eq!(subjects(&s), vec![children[3].key]);
    let before = s.bytes();
    s.adopt(children[0].key, node(children[0].key, 1), LIMITS.resident_bytes).unwrap();
    assert_eq!(s.bytes(), before);
    assert_eq!(s.cut, vec![children[3]]);
    s.select(&[], |_| None, LIMITS);
    assert!(s.cut.is_empty() && s.wanted.is_empty());
    s.select(&[(root(0).summary.key, 0.0)], all, limits(0));
    assert!(s.cut.is_empty() && s.wanted.is_empty());
}

#[test]
fn a_complete_cut_reconsiders_capacity_after_other_branches_disappear() {
    let mut s = loaded(2);
    let children = root(2).children;
    s.nodes.extend(children.iter().map(|c| node(c.key, 1)));
    let kept = node(children[0].key, 1).children;
    let assess = |s: &Summary| {
        let relevant = s.key == root(2).summary.key || children.iter().any(|c| c.key == s.key) || kept.iter().any(|c| c.key == s.key);
        relevant.then_some(Relevance { distance: s.key as f64, refine: true })
    };
    s.select(&[(root(0).summary.key, 0.0)], assess, limits(4));
    assert_eq!(s.cut, kept, "a formerly blocked branch must use the slots the current view freed");
    assert_cut(&s, limits(4));
}

#[test]
fn region_discovery_rejects_wrong_addresses_and_revision_changes_resolve_only_that_region_again() {
    let mut s = Store::default();
    let a = root(2);
    let b = node(a.summary.key + 1, 2);
    let roots = [(a.summary.key, 0.0), (b.summary.key, 1.0)];
    s.select(&roots, all, LIMITS);
    assert!(s.adopt(a.summary.key, b.clone(), LIMITS.resident_bytes).is_err());
    s.adopt(a.summary.key, a.clone(), LIMITS.resident_bytes).unwrap();
    s.adopt(b.summary.key, b.clone(), LIMITS.resident_bytes).unwrap();
    s.select(&roots, all, LIMITS);
    let key = subjects(&s)[0];
    let mut updated = node(key, 1);
    updated.revision = 2;
    assert!(s.adopt(key, updated, LIMITS.resident_bytes).is_err());
    assert!(s.nodes.iter().all(|n| !xy::contains(a.summary.key, n.summary.key)));
    assert!(s.nodes.iter().any(|n| n.summary.key == b.summary.key));
    s.select(&roots, all, LIMITS);
    assert!(subjects(&s).contains(&a.summary.key));
    assert!(!subjects(&s).contains(&b.summary.key));
    assert_cut(&s, LIMITS);
}
