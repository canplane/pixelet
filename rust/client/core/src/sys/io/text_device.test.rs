// pxl-core/sys/io/text_device.test.rs

use super::line_ends;

/// Each character is 10 wide.
fn ends(text: &str, max_width: f64) -> Vec<usize> {
    line_ends(text, max_width, &mut |s| s.chars().count() as f64 * 10.0)
}

#[test]
fn one_line_without_a_width() {
    assert_eq!(ends("hello world", 0.0), vec![11]);
    assert_eq!(ends("", 100.0), vec![0]);
    assert_eq!(ends("   ", 100.0), vec![3]);
}

#[test]
fn words_fill_each_line() {
    assert_eq!(ends("aaa bbb ccc", 70.0), vec![7, 11]);
}

#[test]
fn a_space_stays_at_the_head_of_the_next_line() {
    let text = "aaa bbb";
    let e = ends(text, 30.0);
    assert_eq!(e, vec![3, 7]);
    assert_eq!(&text[e[0]..e[1]], " bbb");
}

#[test]
fn a_word_too_wide_breaks_between_characters() {
    assert_eq!(ends("가나다라마", 20.0), vec![6, 12, 15]);
}

#[test]
fn no_line_starts_with_a_closing_mark() {
    // The break before 。 moves back a character.
    assert_eq!(ends("가나다。라", 30.0), vec![6, 15]);
}

#[test]
fn no_line_ends_with_an_opening_mark() {
    assert_eq!(ends("가나「다라", 30.0), vec![6, 15]);
}

#[test]
fn an_astral_character_stays_whole() {
    assert_eq!(ends("😀😀😀", 20.0), vec![8, 12]);
}
