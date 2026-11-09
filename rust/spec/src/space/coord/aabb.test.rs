// pxl-spec/space/coord/aabb.test.rs

use super::*;

fn tall() -> Aabb {
    Aabb {
        min: [0.0, 0.0, 0.0],
        max: [2.0, 2.0, 3.0],
    }
}

#[test]
fn enters_top() {
    let b = tall();
    assert_eq!(
        b.enters([1.5, 0.5, 10.0], [-0.1, 0.05, -1.0]),
        Some((7.0, 2))
    );
    assert_eq!(b.enters([1.0, 1.0, 10.0], [0.0, 0.0, -1.0]), Some((7.0, 2)));
}

#[test]
fn enters_side() {
    let b = tall();
    assert_eq!(
        b.enters([-5.0, 1.0, 1.5], [1.0, 0.1, 0.05]).map(|e| e.1),
        Some(0)
    );
    assert_eq!(
        b.enters([1.0, 9.0, 1.5], [0.05, -1.0, -0.1]).map(|e| e.1),
        Some(1)
    );
}

#[test]
fn grazing_enters_last_plane() {
    // Over the top's edge, down onto it: the side's plane is reached first.
    let b = tall();
    assert_eq!(
        b.enters([2.5, 1.0, 3.4], [-1.0, 0.0, -0.5]).map(|e| e.1),
        Some(2)
    );
    // Under the top's edge, into the side.
    assert_eq!(
        b.enters([2.5, 1.0, 2.9], [-1.0, 0.0, 0.0]).map(|e| e.1),
        Some(0)
    );
}

#[test]
fn miss_or_inside_enters_none() {
    let b = tall();
    // Down past its side, as over what a model holds out past the box.
    assert_eq!(b.enters([2.1, 1.0, 10.0], [0.0, 0.0, -1.0]), None);
    // Over the top's corner, out past the side before it reaches the top.
    assert_eq!(b.enters([2.5, 1.0, 3.4], [-1.0, 0.0, 0.5]), None);
    // Away from it.
    assert_eq!(b.enters([1.0, 1.0, 10.0], [0.0, 0.0, 1.0]), None);
    // From inside.
    assert_eq!(b.enters([1.0, 1.0, 1.0], [0.0, 0.0, -1.0]), None);
}
