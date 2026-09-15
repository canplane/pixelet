// xpute_core/kit/text.test.rs

use core::fmt::Write;

use super::*;

#[test]
fn text_writes_into_its_bytes_and_refuses_what_does_not_fit_whole() {
    let mut buf = [0u8; 8];
    let mut t = Text::new(&mut buf);
    write!(t, "ab{}", 12).unwrap();
    assert_eq!(t.as_str(), "ab12");
    assert!(t.write_str("héllo").is_err(), "six bytes past the four left");
    assert_eq!(t.as_str(), "ab12", "and nothing of it written");
    t.clear();
    assert!(t.is_empty());
}

#[test]
fn quoted_writes_a_json_string_of_whatever_displays() {
    let mut buf = [0u8; 64];
    let mut t = Text::new(&mut buf);
    write!(t, "{}", Quoted("a\"b\\c\nd\u{1}é")).unwrap();
    assert_eq!(t.as_str(), "\"a\\\"b\\\\c\\nd\\u0001é\"");
    t.clear();
    write!(t, "{}", Quoted(3.5)).unwrap();
    assert_eq!(t.as_str(), "\"3.5\"");
}

#[test]
fn a_fixed_string_is_cut_at_the_last_whole_character_that_fits() {
    assert_eq!(FixStr::<5>::of("abcdé").as_str(), "abcd");
    let mut s = FixStr::<5>::new();
    assert!(s.push('é') && s.push('é'));
    assert!(!s.push('é'), "two bytes past the one left");
    assert_eq!(&*s, "éé");
    assert_eq!(s, FixStr::<5>::of("éé"));
    assert!(write!(s, "z").is_ok());
    assert!(write!(s, "z").is_err());
    assert_eq!(s, "ééz");
}
