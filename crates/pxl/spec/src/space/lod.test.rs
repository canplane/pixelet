// pxl-spec/space/lod.test.rs

use super::*;

/// The ground's zoom a distance draws, past the pyramid's ends too.
fn zoom_at(r: f64) -> i32 {
    GROUND.z_of(cell_at(r))
}

/// Feet on a page's corners, edges and inside, either side of the origin.
fn feet(z: u32) -> Vec<[f64; 2]> {
    let span = span_of(z);
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

fn faults(z: u32, foot: [f64; 2]) -> Vec<Page> {
    pages_within(z, foot, fault_radius(z)).collect()
}

#[test]
fn a_zoom_faults_on_at_most_three_by_three_pages_wherever_the_foot_stands() {
    for z in TOP_Z..=LEAF_Z {
        for foot in feet(z) {
            let n = faults(z, foot).len() as u32;
            assert!(n <= FAULT_PAGES && n <= 9, "z{z} at {foot:?}: {n} pages");
        }
    }
}

#[test]
fn the_parents_of_a_zooms_pages_lie_in_the_next_coarser_ones() {
    for z in TOP_Z + 1..=LEAF_Z {
        for foot in feet(z) {
            let above = faults(z - 1, foot);
            for p in faults(z, foot) {
                assert!(above.contains(&p.parent()), "z{z} at {foot:?}: {p:?}'s parent outside");
            }
        }
    }
}

/// Wherever an observation hits, the page it asks for is one of that zoom's fault pages.
#[test]
fn a_hit_asks_for_a_page_inside_the_fault_bound() {
    for z in TOP_Z..=LEAF_Z {
        for foot in feet(z) {
            let pages = faults(z, foot);
            for height in [0.0, 3.0, 1000.0] {
                for k in 0..64 {
                    let angle = k as f64 * 0.37;
                    for run in [0.5, 7.0, 100.0, 383.9, 900.0, 5_000.0, 40_000.0, 300_000.0] {
                        let at = [foot[0] + run * libm::cos(angle), foot[1] + run * libm::sin(angle)];
                        let asked = zoom_at(libm::hypot(run, height)).min(LEAF_Z as i32);
                        if asked != z as i32 {
                            continue;
                        }
                        let [x, y] = [at[0] / span_of(z), at[1] / span_of(z)].map(|c| c.floor() as i64);
                        assert!(pages.contains(&page_at(z, [x, y])), "z{z}: a hit {run} along the ground from {foot:?}");
                    }
                }
            }
        }
    }
}

/// The fault radius is exactly where a zoom stops being asked for.
#[test]
fn the_fault_bound_is_where_a_zoom_ends_and_holds_no_chunk() {
    for z in TOP_Z..=LEAF_Z {
        let r = fault_radius(z);
        assert_eq!(zoom_at(r * (1.0 - 1e-12)), z as i32, "z{z} is asked for up to its radius");
        assert_eq!(zoom_at(r), z as i32 - 1, "and not at it");
    }
}

#[test]
fn a_zoom_is_the_distance_alone_and_the_same_ratio_at_every_zoom() {
    let mut last = i32::MAX;
    for k in 0..4000 {
        let r = 0.01 * 1.01f64.powi(k);
        let z = zoom_at(r);
        assert!(z <= last, "{r}: z{z} after z{last}");
        last = z;
        for n in [-3, 1, 5] {
            assert_eq!(zoom_at(r * 2f64.powi(n)), z - n, "{r} scaled by 2^{n}");
        }
    }
    for z in TOP_Z as i32..=LEAF_Z as i32 + 3 {
        assert_eq!(zoom_at(split_distance(GROUND.side_at(z as u32))), z, "a zoom begins at its split distance");
        assert_eq!(zoom_at(split_distance(GROUND.side_at(z as u32 - 1)) * (1.0 - 1e-12)), z, "and ends where the next coarser begins");
    }
}
