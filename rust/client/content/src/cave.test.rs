use super::{ROOT, ROOT_BLOCKS, blocks, hollow};
use pxl_engine::seed::node_seed;
use pxl_spec::share::FULL;
use pxl_spec::space::coord::world::SEA_LEVEL;
use pxl_spec::space::coord::zoom::LEAF_CELL_ZOOM;
use pxl_spec::space::hierarchy::tree::child_of;

fn children(zoom: u32, c: [i32; 3]) -> impl Iterator<Item = (u32, [i32; 3])> {
    (0..8).map(move |k| (zoom + 1, child_of(c, k)))
}

/// Every node holds exactly what its eight do, from a root to the leaves.
#[test]
fn a_node_is_its_children() {
    let mut h = 0x9e37_79b9u32;
    let mut next = || {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        h
    };
    for _ in 0..300 {
        let r = [
            (next() % 64) as i32 - 32,
            (next() % 64) as i32 - 32,
            (next() % 8) as i32 - 4,
        ];
        // Down a line from the root, checking each node on it and a sibling.
        let mut node = (ROOT, r);
        while node.0 < LEAF_CELL_ZOOM {
            let sum: u32 = children(node.0, node.1).map(blocks_or_leaves).sum();
            assert_eq!(blocks_or_leaves(node), sum, "node {node:?}");
            let k = next() % 8;
            node = children(node.0, node.1).nth(k as usize).unwrap();
        }
    }
}

/// A node's caves counted in leaves: each block counts as 8 leaves, and at the leaf zoom a
/// leaf counts as its block's one.
fn blocks_or_leaves(node: (u32, [i32; 3])) -> u32 {
    if node.0 == LEAF_CELL_ZOOM {
        blocks(node)
    } else {
        blocks(node) * 8
    }
}

/// The caves take some of a root, not most, and the same share of every root.
#[test]
fn every_root_alike() {
    let a = hollow((ROOT, [0, 0, 0]));
    assert!(a > 0 && a < FULL / 20, "{a}");
    for c in [[5, -3, 1], [100, 7, -2], [-40, 40, 0]] {
        assert_eq!(hollow((ROOT, c)), a);
    }
    assert_eq!(hollow((ROOT - 3, [1, 2, 0])), a);
    // The constant a coarser node reads is what a root's descent holds.
    for c in [[0, 0, 0], [7, -9, 3], [-31, 12, -1]] {
        assert_eq!(blocks((ROOT, c)), ROOT_BLOCKS);
    }
}

/// A picture of one root's caves, for a look only: `cargo test --release
/// -p pxl-content picture -- --ignored`, written where `CAVE_PICTURE` says.
#[test]
#[ignore]
fn picture() {
    use super::{PIPE, holds, hollow_from};
    let Ok(path) = std::env::var("CAVE_PICTURE") else {
        return;
    };
    let side = 128usize; // The side in blocks, each two units, which makes four roots across.
    let mut hollow = vec![false; side * side * side];
    for cx in 0..side as i32 / 4 {
        for cy in 0..side as i32 / 4 {
            for cz in 0..side as i32 / 4 {
                let p = holds((PIPE, [cx, cy, cz])).piece;
                if p.blocks == 0 {
                    continue;
                }
                let h = node_seed(super::BLOCKS_TAG, (PIPE, [cx, cy, cz]));
                for b in 0..64u32 {
                    if hollow_from(h, &p, [b, 0, 0], 0) == 1 {
                        let (x, y, z) = (
                            cx as usize * 4 + (b & 3) as usize,
                            cy as usize * 4 + (b >> 2 & 3) as usize,
                            cz as usize * 4 + (b >> 4 & 3) as usize,
                        );
                        hollow[(x * side + y) * side + z] = true;
                    }
                }
            }
        }
    }
    let g = |x: usize, y: usize, z: usize| hollow[(x * side + y) * side + z];
    let w = side * 2 + 4;
    let mut img = vec![245u8; w * w];
    for (ox, oy, q) in [
        (0, 0, 0usize),
        (side + 4, 0, 1),
        (0, side + 4, 2),
        (side + 4, side + 4, 3),
    ] {
        for a in 0..side {
            for b in 0..side {
                let first = |f: &dyn Fn(usize) -> bool| (0..side).find(|&k| f(k));
                let hit = match q {
                    0 => {
                        if g(a, side / 2, side - 1 - b) {
                            Some(0)
                        } else {
                            None
                        }
                    }
                    1 => first(&|k| g(a, b, side - 1 - k)),
                    2 => first(&|k| g(a, k, side - 1 - b)),
                    _ => first(&|k| g(k, a, side - 1 - b)),
                };
                if let Some(d) = hit {
                    img[(oy + b) * w + ox + a] = (30 + d * 170 / side) as u8;
                }
            }
        }
    }
    let mut out = format!("P5 {w} {w} 255\n").into_bytes();
    out.extend(img);
    std::fs::write(path, out).unwrap();
}

/// Every node of a root, from the root to its pipe cells, holds exactly what
/// its eight do; no pipe cell holds past its 64; every piece lies in its
/// node; and the pipe cells together hold the root's blocks.
#[test]
fn every_node_of_a_root_is_its_children() {
    use super::{PIPE, bounds, holds};
    for r in [[0, 0, 0], [1, 2, 3], [-7, 5, -1], [33, -40, 2]] {
        let mut level = vec![(ROOT, r)];
        while level[0].0 < PIPE {
            let mut next = vec![];
            for &node in &level {
                let kids: Vec<_> = children(node.0, node.1).collect();
                let sum: u32 = kids.iter().map(|&k| blocks(k)).sum();
                assert_eq!(blocks(node), sum, "node {node:?}");
                let p = holds(node).piece;
                if p.blocks > 0 {
                    let (o, s) = bounds(node);
                    let inside = |v: [i32; 3]| (0..3).all(|j| v[j] >= o[j] && v[j] < o[j] + s);
                    assert!(
                        inside(p.enter) && inside(p.leave),
                        "piece {p:?} out of {node:?}"
                    );
                }
                next.extend(kids);
            }
            level = next;
        }
        let mut all = 0;
        for &cell in &level {
            let n = blocks(cell);
            assert!(n <= 64, "{cell:?} {n}");
            all += n;
        }
        assert_eq!(all, ROOT_BLOCKS, "root {r:?}");
    }
}

/// Finer than a leaf, a node reads its leaf.
#[test]
fn finer_than_a_leaf_is_its_leaf() {
    for c in [[5, 9, 3], [-11, 40, -2]] {
        let leaf = hollow((LEAF_CELL_ZOOM, c));
        assert_eq!(
            hollow((LEAF_CELL_ZOOM + 2, [c[0] * 4 + 1, c[1] * 4 + 3, c[2] * 4])),
            leaf
        );
    }
}

/// How often a leaf is a cave in the band a cave may take under a face near
/// sea level, for a look only: `cargo test --release -p pxl-content band --
/// --ignored --nocapture`.
#[test]
#[ignore]
fn band() {
    let sea = SEA_LEVEL;
    let mut hollow_n = 0u64;
    let mut all = 0u64;
    let mut cols_with = 0u64;
    for y in 0..128 {
        for x in 0..128 {
            let mut any = false;
            for z in sea - 64..sea {
                all += 1;
                // Columns 37 units apart, over 18 roots across.
                if hollow((LEAF_CELL_ZOOM, [x * 37, y * 37, z])) >= FULL {
                    hollow_n += 1;
                    any = true;
                }
            }
            cols_with += any as u64;
        }
    }
    println!(
        "band: {hollow_n} of {all} leaves ({:.3}%), columns with any {cols_with} of {}",
        hollow_n as f64 * 100.0 / all as f64,
        128 * 128
    );
    let mut deep = 0u64;
    for d in [0, 8, 16, 32, 48, 63] {
        let mut n = 0;
        for y in 0..128 {
            for x in 0..128 {
                n += (hollow((LEAF_CELL_ZOOM, [x * 37, y * 37, sea - 1 - d])) >= FULL) as u64;
            }
        }
        deep += n;
        println!("depth {d}: {n} of {}", 128 * 128);
    }
    let _ = deep;
}
