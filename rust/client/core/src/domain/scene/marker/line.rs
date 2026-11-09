// pxl-core/domain/scene/marker/line.rs

//! The chat's line (`chat.rs`) as the arrangement takes it: a bubble that its
//! speaker hangs over its head while the speaker shows it
//! (domain/scene/character/companion.rs `line_hangs`). A tap on the bubble
//! goes nowhere.

use super::arrangement::{Instance, Key};
use crate::domain::scene::character::companion::line_hangs;
use crate::domain::scene::marker::chat::shown_line;
use crate::domain::scene::marker::design::Design;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;

/// What the arrangement compares of the line, by value: its bubble while the
/// line is said, and where the bubble hangs while it shows, seen from the
/// eye.
#[derive(Clone, PartialEq)]
pub(super) struct Bubble {
    line: Option<Design>,
    pos: Option<Q3>,
}

/// The line as it is now, seen from `eye`.
pub(super) fn bubble(eye: V3) -> Bubble {
    Bubble {
        line: shown_line(),
        pos: line_hangs(eye),
    }
}

impl Bubble {
    /// The line's instance, or None while it does not show.
    pub(super) fn instance(&self) -> Option<Instance> {
        let (design, pos) = (self.line.clone()?, self.pos?);
        Some(Instance {
            key: Key::Line,
            design,
            pos,
            tap: None,
        })
    }

    /// The line's bubble while the line is said, wherever its speaker is and
    /// whether or not the bubble shows.
    pub(super) fn design(&self) -> Option<Design> {
        self.line.clone()
    }
}
