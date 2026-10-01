// pxl-kernel/control/camera/gesture.rs

//! A control a gesture drives. What it asks is a function of where the
//! gesture began — the control's value then and where the hand was — and
//! where the hand is now, never a sum of its moves: a gesture brought back
//! asks what it began at. Where the camera takes less than was asked — a
//! limit, the terrain — the anchor moves to what it took and where the hand
//! is, so what was refused is not kept to be paid back, and a move the other
//! way is answered at once. Every gesture that drives a control does it
//! through this, so none keeps a debt of its own.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    value: f64,
    at: f64,
}

impl Anchor {
    /// A gesture begun with the control at `value` and the hand at `at`.
    pub const fn new(value: f64, at: f64) -> Anchor {
        Anchor { value, at }
    }

    /// What the gesture asks with the hand at `now`: `map(value, at, now)`.
    pub fn ask(&self, now: f64, map: impl Fn(f64, f64, f64) -> f64) -> f64 {
        map(self.value, self.at, now)
    }

    /// The camera took `took` where `asked` was asked, the hand at `now`:
    /// where it took less, the gesture goes on from there.
    pub fn settle(&mut self, asked: f64, took: f64, now: f64) {
        if took != asked {
            *self = Anchor::new(took, now);
        }
    }
}

/// A tilt or a turn: the value moved by the hand's travel times `gain`.
pub fn linear(gain: f64) -> impl Fn(f64, f64, f64) -> f64 {
    move |value, at, now| value + (now - at) * gain
}

/// A pinch's zoom: the distance scaled as the fingers' spread closes,
/// spreading them nearer.
pub fn spread(value: f64, at: f64, now: f64) -> f64 {
    value * at / now
}

#[cfg(test)]
#[path = "gesture.test.rs"]
mod test;
