// pxl-tool/asset/symbol.test.rs

use super::*;

/// The sheet's ratio here is the spec's `RATIO_MAX`, read off its source.
#[test]
fn the_sheet_s_ratio_is_the_spec_s() {
    let grid = crate::root().join("rust/spec/src/space/camera/grid.rs");
    let text = std::fs::read_to_string(&grid).expect("pxl-spec's camera grid");
    let ratio = regex::Regex::new(r"pub const RATIO_MAX: f64 = ([0-9.]+);")
        .expect("a pattern")
        .captures(&text)
        .and_then(|c| c[1].parse::<f64>().ok())
        .expect("`RATIO_MAX` written as a literal");
    assert_eq!(f64::from(SHEET_RATIO), ratio);
}
