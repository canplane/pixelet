// pxl-kernel/layer/model/occupancy.test.rs

use super::Occupancy;

/// A box from `lo` to `hi`, its twelve triangles.
fn cube(lo: [f32; 3], hi: [f32; 3]) -> (Vec<f32>, Vec<u32>) {
    let positions = (0..8).flat_map(|c| [0, 1, 2].map(|k| if c >> k & 1 == 1 { hi[k] } else { lo[k] })).collect();
    let faces: [[u32; 4]; 6] = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
    let indices = faces.iter().flat_map(|f| [f[0], f[1], f[2], f[0], f[2], f[3]]).collect();
    (positions, indices)
}

fn filled(o: &Occupancy) -> usize {
    o.runs.iter().map(|r| (r[3] - r[2]) as usize).sum()
}

#[test]
fn a_closed_mesh_is_solid_through() {
    let (p, i) = cube([0.0; 3], [2.0; 3]);
    let o = Occupancy::of(&p, &i).unwrap();
    // The faces on the grid's far planes touch the cells past them too.
    let side = (2.0 / o.cell) as usize + 1;
    assert_eq!(filled(&o), side * side * side);
}

#[test]
fn an_open_mesh_is_only_its_shell() {
    // One face of the box: a sheet, nothing enclosed.
    let (p, i) = cube([0.0; 3], [2.0; 3]);
    let o = Occupancy::of(&p, &i[..6]).unwrap();
    let side = (2.0 / o.cell) as usize + 1;
    assert_eq!(filled(&o), side * side);
}
