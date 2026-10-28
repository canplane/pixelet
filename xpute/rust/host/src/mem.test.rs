// xpute-host/mem.test.rs

use super::*;

#[test]
fn little_endian() {
    let mut bytes = [0u8; 12];
    let mut m = Memory::of(&mut bytes);
    m.set_word(5, 0x0403_0201);
    assert_eq!(m.word(5), 0x0403_0201);
    assert_eq!(m.read(5, 4), &[1, 2, 3, 4]);
    assert_eq!(m.bytes(), 12);
}

#[test]
fn write_in_place() {
    let mut bytes = [9u8; 8];
    Memory::of(&mut bytes).write(2, &[1, 2]);
    assert_eq!(bytes, [9, 9, 1, 2, 9, 9, 9, 9]);
}

#[test]
#[should_panic(expected = "run past the memory")]
fn past_end_refused() {
    let mut bytes = [0u8; 8];
    Memory::of(&mut bytes).word(6);
}

#[test]
#[should_panic(expected = "run past the memory")]
fn wrap_refused() {
    let mut bytes = [0u8; 8];
    Memory::of(&mut bytes).read(usize::MAX, 2);
}
