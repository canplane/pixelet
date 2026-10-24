// pxl-kernel/scene/place/line.rs

//! The chat's line as the layout takes it (`billboard/chat.rs`): its bubble,
//! hung from the point its speaker hands over, while its own rule shows it;
//! no place a tap goes to.

use super::{Instance, Key};
use crate::billboard::badge::Label;
use crate::billboard::chat::{hangs, shown_line};
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;

/// What the layout compares of the line, by value: its bubble while it is
/// said, and where it hangs while it shows, seen from the eye.
#[derive(Clone, PartialEq)]
pub(super) struct Said {
    line: Option<Label>,
    hangs: Option<Q3>,
}

/// The line as it is now, seen from `eye` at `tau`.
pub(super) fn said(eye: V3, tau: Tau) -> Said {
    Said {
        line: shown_line(),
        hangs: hangs(eye, tau),
    }
}

impl Said {
    /// Its instance, where it shows.
    pub(super) fn shown(&self) -> Option<Instance> {
        let (label, stands) = (self.line.clone()?, self.hangs?);
        Some(Instance {
            key: Key::Line,
            label,
            stands,
            tap: None,
        })
    }

    /// Its bubble while it is said, kept wherever its speaker is.
    pub(super) fn kept(&self) -> Option<Label> {
        self.line.clone()
    }
}
