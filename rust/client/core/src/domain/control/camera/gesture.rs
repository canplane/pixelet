// pxl-core/domain/control/camera/gesture.rs

//! A gesture asks the camera for a value computed from where the gesture
//! began and where the hand is now, never for a sum of small moves. When
//! the camera takes less than it was asked, the anchor moves to what it
//! took. The refused part of the motion is then not owed back, and the
//! camera answers at once when the hand reverses.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    val: f64,
    at: f64,
}

impl Anchor {
    pub const fn new(val: f64, at: f64) -> Anchor {
        Anchor { val, at }
    }

    /// The value to ask for with the hand at `now`. `map` is given the
    /// anchor's value, where the hand was at the anchor, and `now`.
    pub fn ask(&self, now: f64, map: impl Fn(f64, f64, f64) -> f64) -> f64 {
        map(self.val, self.at, now)
    }

    /// When the camera took a value other than the one asked, the gesture
    /// goes on from what it took, with the hand at `now`.
    pub fn settle(&mut self, asked: f64, took: f64, now: f64) {
        if took != asked {
            *self = Anchor::new(took, now);
        }
    }
}

pub fn linear(gain: f64) -> impl Fn(f64, f64, f64) -> f64 {
    move |val, at, now| val + (now - at) * gain
}

/// A pinch, where the hand's position is the distance between the fingers.
/// Spreading the fingers divides the camera's distance by the same factor,
/// so the camera comes nearer.
pub fn spread(val: f64, at: f64, now: f64) -> f64 {
    val * at / now
}

#[cfg(test)]
#[path = "gesture.test.rs"]
mod test;
