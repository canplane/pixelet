// pxl-core/link/export/input.test.rs

use super::*;
use crate::link::export::{life, Core};

/// A UI that attaches, the first or one a reload makes on the same core,
/// starts with no pointer pressed and no text taken: a press it makes is a
/// press, not one spent while the old UI typed.
#[test]
fn a_new_ui_starts_with_no_pointers_and_not_typing() {
    assert_eq!(typing(true), Ok(()));
    assert_eq!(phase_of(PointerPhase::Down, 1), PointerPhase::Down);
    assert_eq!(pointers().spent, [1], "pressed while typing");
    assert_eq!(life::Guest::attached(&Core), Ok(()));
    let p = pointers();
    assert!(!p.typing && p.spent.is_empty());
    assert_eq!(
        phase_of(PointerPhase::Up, 1),
        PointerPhase::Up,
        "a lift taps"
    );
}
