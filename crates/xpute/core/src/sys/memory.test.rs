// xpute_core/sys/memory.test.rs

use super::*;

const R: u32 = 1 << 26;

#[test]
fn memory_sections_are_whole_regions_in_address_order_past_the_front_the_last_taking_what_is_left() {
    let mut out = [Section::default(); 3];
    lay_out(8 * R + 123, 32 << 20, R, 16 << 20, &[1, 3], &mut out);
    assert_eq!(
        out,
        [
            Section {
                offset: 32 << 20,
                bytes: 32 << 20,
                region_bytes: 16 << 20
            },
            Section {
                offset: R,
                bytes: 3 * R,
                region_bytes: R
            },
            Section {
                offset: 4 * R,
                bytes: 4 * R,
                region_bytes: R
            },
        ]
    );
}

#[test]
fn memory_a_section_s_words_read_back_as_they_were_written() {
    let mut words = [0u32; 2 * SECTION_WORDS as usize];
    let s = Section {
        offset: R,
        bytes: 2 * R,
        region_bytes: R,
    };
    write_section(&mut words, SECTION_WORDS as usize, s);
    assert_eq!(read_section(&words, SECTION_WORDS as usize), s);
}

#[test]
#[should_panic]
fn memory_too_few_regions_for_the_sections_is_refused() {
    let mut out = [Section::default(); 3];
    lay_out(2 * R, 32 << 20, R, 16 << 20, &[1, 1], &mut out);
}
