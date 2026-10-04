// pxl-spec/space/sparse_tree/tree.test.rs

use super::*;
use crate::space::pvf3::pvf3;
use crate::space::sparse_tree::tree as sparse_tree;

const LAYOUT: SparseTreeLayout = SparseTreeLayout {
    dim: 3,
    min: Scale::of(-3),
    max: Scale::of(6),
};

fn at(x: f64, y: f64, z: f64) -> Pvf3 {
    let mut out = Pvf3::default();
    *pvf3::gather(&mut out, pvf3::from_real(x), pvf3::from_real(y), pvf3::from_real(z))
}

#[test]
fn sparse_tree_retype_widening_the_leaf_lane_carries_every_value_across() {
    let mut tree = sparse_tree::alloc::<u8>(LAYOUT, 64);
    sparse_tree::write(&mut tree, &at(3.0, 4.0, 5.0), 200, Scale::of(0));
    sparse_tree::write(&mut tree, &at(10.125, 1.0, 0.0), 7, LAYOUT.min);
    let mut tree = sparse_tree::retype::<u8, u16>(tree);
    assert_eq!(sparse_tree::read(&tree, &at(3.5, 4.5, 5.5)), 200);
    assert_eq!(sparse_tree::read(&tree, &at(10.125, 1.0, 0.0)), 7);
    sparse_tree::write(&mut tree, &at(20.0, 20.0, 20.0), 300, Scale::of(0));
    assert_eq!(sparse_tree::read(&tree, &at(20.0, 20.0, 20.0)), 300);
}

#[test]
#[should_panic(expected = "EOVERFLOW")]
fn sparse_tree_decode_refuses_a_blob_whose_count_overflows_u32_rather_than_wrapping_under_it() {
    let mut blob = vec![0u64; 4];
    blob[0] = 0xffff_ffff;
    let _ = decode::<u16>(
        &blob,
        SparseTreeLayout {
            dim: 3,
            min: Scale::of(-3),
            max: Scale::of(3),
        },
    );
}

/// xorshift64: the same writes on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Every slot `walk_to` reports at each depth — a branch's folded `data` —
/// is what the fold makes of every leaf in its block, here their maximum.
fn folds_hold(tree: &SparseTree<u8>) {
    let mut leaves = Vec::new();
    st_mem::walk(&tree.mem, &tree.layout, &mut |x, y, z, s, v| leaves.push(([x, y, z], s, v)), 0);
    for depth in LAYOUT.min.shamt()..LAYOUT.max.shamt() {
        st_mem::walk_to(
            &tree.mem,
            &tree.layout,
            Scale::of(depth),
            &mut |x, y, z, s, v| {
                let n = LAYOUT.min.count_in(s) as i32;
                let within = |[lx, ly, lz]: [i32; 3], ls: Scale| {
                    let m = LAYOUT.min.count_in(ls) as i32;
                    [(x, lx), (y, ly), (z, lz)].iter().all(|&(a, b)| b < a + n && a < b + m)
                };
                let most = leaves.iter().filter(|&&(at, ls, _)| within(at, ls)).map(|&(_, _, lv)| lv).max().unwrap_or(0);
                assert_eq!(v, most, "the block at ({x}, {y}, {z}) of side {s:?} folds to {v}, its leaves to {most}");
            },
            0,
        );
    }
}

