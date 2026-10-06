// pxl-kernel/control/camera/gesture.test.rs

use super::*;

fn drive(anchor: &mut Anchor, now: f64, max: f64) -> f64 {
    let asked = anchor.ask(now, linear(1.0));
    let took = asked.min(max);
    anchor.settle(asked, took, now);
    took
}

#[test]
fn a_refused_share_is_not_paid_back() {
    let mut a = Anchor::new(0.0, 0.0);
    assert_eq!(drive(&mut a, 5.0, 10.0), 5.0);
    assert_eq!(drive(&mut a, 40.0, 10.0), 10.0, "past the limit");
    assert_eq!(drive(&mut a, 39.0, 10.0), 9.0, "the move back was spent on the refused share");
}

#[test]
fn a_gesture_brought_back_asks_what_it_began_at() {
    let mut a = Anchor::new(3.0, 100.0);
    for now in [120.0, 80.0, 150.0, 100.0] {
        drive(&mut a, now, f64::INFINITY);
    }
    assert_eq!(drive(&mut a, 100.0, f64::INFINITY), 3.0);
    let zoom = Anchor::new(300.0, 100.0);
    assert_eq!(zoom.ask(50.0, spread), 600.0);
    assert_eq!(zoom.ask(100.0, spread), 300.0);
}
