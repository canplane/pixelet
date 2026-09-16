// pxl_core/world/decor.test.rs

#[test]
fn a_draw_rounds_the_word_to_a_double_first_as_javascript_does() {
    // A word past 2^53 whose low bits differ from its double's.
    let z: u64 = 0xffff_ffff_ffff_fdff;
    assert_eq!(((z as f64) % 4_294_967_296.0) as u32, 0, "rounds up to 2^64, which is 0 mod 2^32");
    assert_ne!(z as u32, 0);
}
