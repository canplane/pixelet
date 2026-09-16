// pxl_core/bake/terrain.test.rs

use super::*;

#[test]
fn rounds_as_javascript_does() {
    // Math.round takes a tie toward +∞; f64::round takes it away from zero.
    assert_eq!(js_round(2.5), 3.0);
    assert_eq!(js_round(-2.5), -2.0);
    assert_eq!(js_round(-2.4), -2.0);
    assert_eq!(js_round(-2.6), -3.0);
    assert_eq!(js_round(-0.5), 0.0);
}
