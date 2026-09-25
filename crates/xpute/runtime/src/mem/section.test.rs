// xpute-runtime/mem/section.test.rs

use super::*;

const PAGE: u32 = 1 << 12;

#[test]
fn memory_a_range_ends_at_the_boundary_the_next_one_begins_on() {
    assert_eq!(align_up(0, PAGE), 0);
    assert_eq!(align_up(1, PAGE), PAGE);
    assert_eq!(align_up(PAGE - 1, PAGE), PAGE);
    assert_eq!(align_up(PAGE, PAGE), PAGE, "a boundary is already one");
    assert_eq!(align_up(PAGE + 1, PAGE), 2 * PAGE);
}

#[test]
fn memory_ranges_laid_end_to_end_each_begin_on_the_unit_and_never_overlap() {
    let sizes = [PAGE, 1, 3 * PAGE + 1, 0];
    let base = 8 * PAGE;
    let at: Vec<u32> = (0..sizes.len()).map(|n| nth_at(base, &sizes, PAGE, n)).collect();
    assert_eq!(at, [8 * PAGE, 9 * PAGE, 10 * PAGE, 14 * PAGE]);
    for (i, &a) in at.iter().enumerate() {
        assert!(a.is_multiple_of(PAGE), "{i}");
        assert!(a + sizes[i] <= nth_at(base, &sizes, PAGE, i + 1), "{i}: runs into the next");
    }
    assert_eq!(span_of(&sizes, PAGE), 6 * PAGE, "the last one's rounding is in the span");
    assert_eq!(nth_at(base, &sizes, PAGE, sizes.len()), base + span_of(&sizes, PAGE), "and one past the last is where the span ends");
}

#[test]
fn memory_nothing_laid_spans_nothing() {
    assert_eq!(span_of(&[], PAGE), 0);
    assert_eq!(nth_at(64, &[], PAGE, 0), 64);
}

#[test]
fn memory_a_section_s_words_read_back_as_they_were_written() {
    let mut words = [0u32; 2 * SECTION_WORDS as usize];
    let s = Section {
        offset: 1 << 26,
        bytes: 2 << 26,
        align: PAGE,
    };
    write_section(&mut words, SECTION_WORDS as usize, s);
    assert_eq!(read_section(&words, SECTION_WORDS as usize), s);
}

#[test]
fn memory_a_span_inside_a_section_is_reached_only_where_it_ends_by_the_section_s_end() {
    let s = Section {
        offset: 4 * PAGE,
        bytes: 2 * PAGE,
        align: PAGE,
    };
    assert_eq!(s.end(), 6 * PAGE);
    assert_eq!(s.at(0, 2 * PAGE), Some(4 * PAGE), "the whole of it");
    assert_eq!(s.at(PAGE, PAGE), Some(5 * PAGE), "its last page");
    assert_eq!(s.at(2 * PAGE, 0), Some(6 * PAGE), "nothing, at its end");
    assert_eq!(s.at(PAGE, PAGE + 1), None, "a byte past its end");
    assert_eq!(s.at(u32::MAX, 2), None, "past the address space");
}

#[test]
fn memory_a_range_laid_inside_a_section_ends_where_its_own_size_does() {
    let s = Section {
        offset: 8 * PAGE,
        bytes: 4 * PAGE,
        align: PAGE,
    };
    let sizes = [PAGE + 1, PAGE];
    let second = s.nth(&sizes, PAGE, 1);
    assert_eq!(
        second,
        Section {
            offset: 10 * PAGE,
            bytes: PAGE,
            align: PAGE
        }
    );
    assert_eq!(second.at(0, PAGE + 1), None, "its own end, not the section's");
}
