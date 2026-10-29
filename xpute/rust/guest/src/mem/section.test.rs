// xpute-guest/mem/section.test.rs

use super::*;

const PAGE: u32 = 1 << 12;

#[test]
fn ranges_aligned_disjoint() {
    let sizes = [PAGE, 1, 3 * PAGE + 1, 0];
    let base = 8 * PAGE;
    let at: Vec<u32> = (0..sizes.len())
        .map(|n| nth_at(base, &sizes, PAGE, n))
        .collect();
    assert_eq!(at, [8 * PAGE, 9 * PAGE, 10 * PAGE, 14 * PAGE]);
    for (i, &a) in at.iter().enumerate() {
        assert!(a.is_multiple_of(PAGE), "{i}");
        assert!(
            a + sizes[i] <= nth_at(base, &sizes, PAGE, i + 1),
            "{i}: runs into the next"
        );
    }
    assert_eq!(
        span_of(&sizes, PAGE),
        6 * PAGE,
        "the last one's rounding is in the span"
    );
    assert_eq!(
        nth_at(base, &sizes, PAGE, sizes.len()),
        base + span_of(&sizes, PAGE),
        "and one past the last is where the span ends"
    );
}

#[test]
fn span_within_section() {
    let s = Section {
        off: 4 * PAGE,
        bytes: 2 * PAGE,
    };
    assert_eq!(s.end(), 6 * PAGE);
    assert_eq!(s.at(0, 2 * PAGE), Some(4 * PAGE), "the whole of it");
    assert_eq!(s.at(PAGE, PAGE), Some(5 * PAGE), "its last page");
    assert_eq!(s.at(2 * PAGE, 0), Some(6 * PAGE), "nothing, at its end");
    assert_eq!(s.at(PAGE, PAGE + 1), None, "a byte past its end");
    assert_eq!(s.at(u32::MAX, 2), None, "past the address space");
}

#[test]
fn range_ends_by_sz() {
    let s = Section {
        off: 8 * PAGE,
        bytes: 4 * PAGE,
    };
    let sizes = [PAGE + 1, PAGE];
    let second = s.nth(&sizes, PAGE, 1);
    assert_eq!(
        second,
        Section {
            off: 10 * PAGE,
            bytes: PAGE
        }
    );
    assert_eq!(
        second.at(0, PAGE + 1),
        None,
        "its own end, not the section's"
    );
}
