use super::{blocks, hollow, ROOT, ROOT_BLOCKS};
use pxl_engine::answer::FULL;
use pxl_engine::LEAF_CELL;

fn children(zoom: u32, c: [i32; 3]) -> impl Iterator<Item = (u32, [i32; 3])> {
    (0..8).map(move |k| {
        (
            zoom + 1,
            [
                2 * c[0] + (k & 1),
                2 * c[1] + (k >> 1 & 1),
                2 * c[2] + (k >> 2 & 1),
            ],
        )
    })
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
        while node.0 < LEAF_CELL {
            let sum: u32 = children(node.0, node.1).map(blocks_or_leaves).sum();
            assert_eq!(blocks_or_leaves(node), sum, "node {node:?}");
            let k = next() % 8;
            node = children(node.0, node.1).nth(k as usize).unwrap();
        }
    }
}

/// Its leaves of caves: a block is 8, a leaf its block's one.
fn blocks_or_leaves(node: (u32, [i32; 3])) -> u32 {
    if node.0 == LEAF_CELL {
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
    extern crate std;
    use super::{block_hollow, holds, PIPE};
    use std::format;
    use std::vec;
    let Ok(path) = std::env::var("CAVE_PICTURE") else {
        return;
    };
    let side = 128usize; // blocks, 2 units each: four roots across
    let mut hollow = vec![false; side * side * side];
    for cx in 0..side as i32 / 4 {
        for cy in 0..side as i32 / 4 {
            for cz in 0..side as i32 / 4 {
                let p = holds((PIPE, [cx, cy, cz])).piece;
                if p.blocks == 0 {
                    continue;
                }
                for b in 0..64u32 {
                    if block_hollow([cx, cy, cz], &p, b) {
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
