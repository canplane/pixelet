// pxl-kernel/domain/scene/marker/chat.rs

//! The (dev-only) map chat's latest line and who says it: a bubble hung from
//! the point over its speaker's head that the scene hands over
//! (`speak_from`), and shown by its own rule (`hangs`), not read from the
//! chunks as a marker someone put up is. `layout.rs` lays both out.

use std::rc::Rc;

use pxl_spec::block::object::marker_shows;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

use crate::domain::query::cell_at;
use crate::domain::scene::marker::design::Design;
use crate::domain::view::frame::coord::to_world;
use crate::sys::sched::edge::wake_at;

/// Chat has no expiry of its own; a line shows this long.
const SHOWN_MS: f64 = 8000.0;

/// Stamped with the host's clock, not the frame's: a line said after a still
/// spell would take the last frame's time and read as long gone. A
/// character swap cannot make a dismissed line look new.
struct Line {
    text: Rc<str>,
    said: f64,
}

#[derive(Default)]
struct Chat {
    line: Option<Line>,
    /// Where the speaker stands, and the point over its head a line hangs
    /// from, as the scene hands them down this turn.
    speaker: Option<(Q3, Q3)>,
}

static CHAT: Global<Chat> = Global::new();

fn chat() -> &'static mut Chat {
    CHAT.get(Chat::default)
}

pub fn append_chat_line(text: &str) {
    chat().line = Some(Line { text: text.into(), said: now() });
}

/// The line to show, in its bubble, none once shown its while; the turn is
/// woken then.
pub fn shown_line() -> Option<Design> {
    let line = chat().line.as_ref()?;
    if line.said + SHOWN_MS <= now() {
        return None;
    }
    wake_at(line.said + SHOWN_MS);
    Some(Design::Bubble { words: line.text.clone() })
}

/// None while there is no one to speak.
pub fn speak_from(speaker: Option<(Q3, Q3)>) {
    chat().speaker = speaker;
}

/// Where a line hangs, over its speaker's head, while it shows there seen
/// from `eye` at `tau`: as a marker shows (`marker_shows`), so a line reads
/// where the markers about its speaker do. A marker someone put up is
/// judged by the chunk the markers are read from; a line, which no chunk
/// holds, by the camera grid's cell where its speaker stands, the zoom such
/// a chunk would be drawn at there.
pub fn hangs(eye: V3, tau: Tau) -> Option<Q3> {
    let (feet, over) = chat().speaker?;
    marker_shows(cell_at(eye, tau, to_world(&feet))).then_some(over)
}
