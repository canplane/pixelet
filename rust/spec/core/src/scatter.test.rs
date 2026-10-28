// pxl-spec-core/scatter.test.rs

use super::*;

/// A patch's contest drawn once for all its candidates (`beaten`) answers
/// each as `stands` does, over every decor of the catalog and the patches of
/// squares at the leaves and coarser.
#[test]
fn a_patch_contest_is_each_candidate_standing() {
    let (mut compared, mut lost) = (0, 0);
    for decor in &DECORS {
        for zoom in [LEAF_ZOOM, LEAF_ZOOM - 3, TOP_ZOOM + 1] {
            let side = 1i32 << (SQUARE_SHIFT + LEAF_ZOOM - zoom);
            for q in [[7, -3], [-12, 40]] {
                let region = [
                    [q[0] * side, q[1] * side],
                    [(q[0] + 1) * side, (q[1] + 1) * side],
                ];
                let mut p = 0;
                loop {
                    let (born, b, real) = patch_of(zoom, region, p);
                    if !real {
                        break;
                    }
                    let mut cs = [Candidate::NONE; COUNTS];
                    let n = each_in_patch(decor, born, b, |i, c| cs[i as usize] = c);
                    let mut asked = [0u32; MARKS];
                    for i in 0..n as usize {
                        mark(&mut asked, i);
                    }
                    let beaten = beaten(decor, zoom, &cs, n, &asked);
                    for i in 0..n as usize {
                        assert_eq!(
                            !is_marked(&beaten, i),
                            stands(decor, zoom, &cs[i]),
                            "decor {} at {zoom}, patch {p}, candidate {i}",
                            decor.value
                        );
                        compared += 1;
                        lost += usize::from(is_marked(&beaten, i));
                    }
                    p += 1;
                }
            }
        }
    }
    assert!(compared > 1000, "only {compared} candidates compared");
    assert!(
        lost > 0,
        "no candidate was beaten, so the contest was not tried"
    );
}

/// Decor is drawn where the rays ask the leaf cell's zoom or finer at its
/// distance, and nowhere they ask coarser.
#[test]
fn decor_is_drawn_at_the_leaves_or_finer_and_never_coarser() {
    let detail = Detail {
        range: [0, crate::FINEST_ZOOM],
        eye: [0.0; 3],
        spacing: crate::cast::Spacing::NONE,
    };
    let mut d = 0.25f32;
    let mut seen = [false; 2];
    while d < 1.0e6 {
        let d2 = d * d;
        let at = detail.at(d2);
        assert_eq!(drawn_at(detail, d2), at >= LEAF_CELL, "at {d}: z{at}");
        seen[usize::from(at >= LEAF_CELL)] = true;
        d *= 1.5;
    }
    assert_eq!(seen, [true, true], "both sides of the leaves are asked");
}
