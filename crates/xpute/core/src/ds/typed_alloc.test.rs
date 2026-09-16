// xpute_core/ds/typed_alloc.test.rs

use super::*;

#[test]
fn typed_alloc_the_heap_allocator_is_new_ctor_n() {
    let mut heap = HEAP_ALLOC;
    let view = heap.take::<f64>(8);
    assert_eq!(view.length(), 8);
    assert_eq!(view.as_slice(), &[0.0; 8]);
}

