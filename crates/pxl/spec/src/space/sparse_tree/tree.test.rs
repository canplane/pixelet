// pxl-spec/space/sparse_tree/tree.test.rs

use xpute_core::kit::array::TypedArray;

use super::*;
use crate::space::pvf3::pvf3;
use crate::space::sparse_tree::tree as sparse_tree;

const LAYOUT: SparseTreeLayout = SparseTreeLayout { dim: 3, min: -3, max: 6 };

fn at(x: f64, y: f64, z: f64) -> PVF3 {
    let mut out = PVF3::default();
    *pvf3::gather(&mut out, pvf3::from_real(x), pvf3::from_real(y), pvf3::from_real(z))
}

#[test]
fn sparse_tree_retype_widening_the_leaf_lane_carries_every_value_across() {
    let mut tree = sparse_tree::alloc::<u8>(LAYOUT, 64);
    sparse_tree::write(&mut tree, &at(3.0, 4.0, 5.0), 200, 0);
    sparse_tree::write(&mut tree, &at(10.125, 1.0, 0.0), 7, LAYOUT.min);
    let mut tree = sparse_tree::retype::<u8, u16>(tree);
    assert_eq!(sparse_tree::read(&tree, &at(3.5, 4.5, 5.5)), 200);
    assert_eq!(sparse_tree::read(&tree, &at(10.125, 1.0, 0.0)), 7);
    sparse_tree::write(&mut tree, &at(20.0, 20.0, 20.0), 300, 0);
    assert_eq!(sparse_tree::read(&tree, &at(20.0, 20.0, 20.0)), 300);
}

/// What the TypeScript computed, recorded: the generator that wrote it
/// is gone and the vector stands as it is (xpute-core/sys/vector.rs).
const VECTOR: &str = include_str!("../../../vector/spec/space/sparse_tree/tree.tsv");

#[test]
fn sparse_tree_computes_what_the_typescript_computes() {
    let v = xpute_core::sys::vector::Vector::load(VECTOR);
    v.each("", |k| {
        let layout = SparseTreeLayout {
            dim: v.u32(&format!("{k}.dim")),
            min: v.i32(&format!("{k}.min")),
            max: v.i32(&format!("{k}.max")),
        };
        let coord = |base: &str| {
            let mut p = PVF3::default();
            *pvf3::gather(&mut p, v.i64(&format!("{base}.x")), v.i64(&format!("{base}.y")), v.i64(&format!("{base}.z")))
        };
        let mut tree = sparse_tree::alloc::<u16>(layout, 8);
        v.each(&format!("{k}.writes"), |w| {
            sparse_tree::write(&mut tree, &coord(w), v.u32(&format!("{w}.val")) as u16, v.i32(&format!("{w}.level")));
        });
        v.each(&format!("{k}.probes"), |p| {
            assert_eq!(sparse_tree::read(&tree, &coord(p)) as u32, v.u32(&format!("{p}.val")), "{p}");
        });
        let mut walked = Vec::new();
        sparse_tree::walk(&tree, &mut |x, y, z, level, val| walked.push(format!("{x},{y},{z},{level},{val}")));
        let want: Vec<&str> = (0..v.len(&format!("{k}.walked"))).map(|j| v.s(&format!("{k}.walked.{j}"))).collect();
        assert_eq!(walked, want, "{k}.walked");
        assert_eq!(tree.mem.head, v.u32(&format!("{k}.head")), "{k}.head");
        sparse_tree::compact(&mut tree);
        assert_eq!(tree.mem.head, v.u32(&format!("{k}.compacted_head")), "{k}.compacted_head");
        let blob = sparse_tree::encode(&tree);
        let hex: String = TypedArray::<u8>::over(blob.buffer().clone(), blob.byte_offset(), blob.byte_length())
            .as_slice()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(hex, v.s(&format!("{k}.encoded")), "{k}.encoded");
    });
}

#[test]
#[should_panic(expected = "does not align within u32")]
fn sparse_tree_decode_refuses_a_blob_whose_count_overflows_u32_rather_than_wrapping_under_it() {
    let blob: U64Array = TypedArray::new(4);
    blob.put(0, 0xffff_ffff);
    let _ = decode::<u16>(&blob, SparseTreeLayout { dim: 3, min: -3, max: 3 });
}
