// pxl-spec/space/lod.test.rs

use super::*;

/// Feet on a page's corners, edges and inside, either side of the origin.
fn feet(level: u32) -> Vec<[f64; 2]> {
    let span = span_of(level);
    let fractions = [0.0, 1e-9, 0.25, 0.5, 0.75, 1.0 - 1e-9];
    let mut out = Vec::new();
    for [px, py] in [[0.0, 0.0], [-3.0, 5.0], [17.0, -11.0]] {
        for fx in fractions {
            for fy in fractions {
                out.push([(px + fx) * span, (py + fy) * span]);
            }
        }
    }
    out
}

fn faults(level: u32, foot: [f64; 2]) -> Vec<LevelPage> {
    pages_within(level, foot, fault_radius(level)).collect()
}

#[test]
fn a_level_faults_on_at_most_three_by_three_pages_wherever_the_foot_stands() {
    for level in 0..=TOP_LEVEL {
        for foot in feet(level) {
            let n = faults(level, foot).len() as u32;
            assert!(n <= FAULT_PAGES && n <= 9, "level {level} at {foot:?}: {n} pages");
        }
    }
}

#[test]
fn the_parents_of_a_levels_pages_lie_in_the_next_levels() {
    for level in 0..TOP_LEVEL {
        for foot in feet(level) {
            let above = faults(level + 1, foot);
            for p in faults(level, foot) {
                assert!(above.contains(&p.parent()), "level {level} at {foot:?}: {p:?}'s parent outside");
            }
        }
    }
}

/// Wherever an observation hits, the page it asks for is one of that level's fault pages.
#[test]
fn a_hit_asks_for_a_page_inside_the_fault_bound() {
    for level in 0..=TOP_LEVEL {
        for foot in feet(level) {
            let pages = faults(level, foot);
            for height in [0.0, 3.0, 1000.0] {
                for k in 0..64 {
                    let angle = k as f64 * 0.37;
                    for run in [0.5, 7.0, 100.0, 383.9, 900.0, 5_000.0, 40_000.0, 300_000.0] {
                        let at = [foot[0] + run * libm::cos(angle), foot[1] + run * libm::sin(angle)];
                        let asked = super::level(libm::hypot(run, height)).max(0);
                        if asked != level as i32 {
                            continue;
                        }
                        let [x, y] = [at[0] / span_of(level), at[1] / span_of(level)].map(|c| c.floor() as i64);
                        assert!(pages.contains(&page_at(level, [x, y])), "level {level}: a hit {run} along the ground from {foot:?}");
                    }
                }
            }
        }
    }
}

/// The fault radius is exactly where a level stops being asked for.
#[test]
fn the_fault_bound_is_where_a_level_ends_and_holds_no_chunk() {
    for l in 0..=TOP_LEVEL {
        let r = fault_radius(l);
        assert_eq!(level(r * (1.0 - 1e-12)), l as i32, "level {l} is asked for up to its radius");
        assert_eq!(level(r), l as i32 + 1, "and not at it");
    }
}

#[test]
fn a_level_is_the_distance_alone_and_the_same_ratio_at_every_level() {
    let mut last = i32::MIN;
    for k in 0..4000 {
        let r = 0.01 * 1.01f64.powi(k);
        let l = level(r);
        assert!(l >= last, "{r}: level {l} after {last}");
        last = l;
        for n in [-3, 1, 5] {
            assert_eq!(level(r * 2f64.powi(n)), l + n, "{r} scaled by 2^{n}");
        }
    }
    for l in -3..=TOP_LEVEL as i32 {
        assert_eq!(level(split_distance(l)), l, "a level begins at its split distance");
        assert_eq!(level(split_distance(l + 1) * (1.0 - 1e-12)), l, "and ends where the next begins");
    }
}
