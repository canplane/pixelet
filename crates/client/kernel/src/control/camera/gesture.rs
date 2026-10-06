// pxl-kernel/control/camera/gesture.rs

//! A control a gesture drives asks a function of where the gesture began and
//! where the hand is now, never a sum of moves. Where the camera takes less
//! than asked, the anchor moves to what it took, so a refusal is never owed
//! back and a reversal answers at once.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    value: f64,
    at: f64,
}

impl Anchor {
    pub const fn new(value: f64, at: f64) -> Anchor {
        Anchor { value, at }
    }

    /// `map(value, at, now)`.
    pub fn ask(&self, now: f64, map: impl Fn(f64, f64, f64) -> f64) -> f64 {
        map(self.value, self.at, now)
    }

    /// Where it took less than asked, the gesture goes on from there.
    pub fn settle(&mut self, asked: f64, took: f64, now: f64) {
        if took != asked {
            *self = Anchor::new(took, now);
        }
    }
}

/// The value moved by the hand's travel times `gain`.
pub fn linear(gain: f64) -> impl Fn(f64, f64, f64) -> f64 {
    move |value, at, now| value + (now - at) * gain
}

/// Spreading the fingers brings the camera nearer.
pub fn spread(value: f64, at: f64, now: f64) -> f64 {
    value * at / now
}

#[cfg(test)]
#[path = "gesture.test.rs"]
mod test;
