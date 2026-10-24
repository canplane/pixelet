// pxl-kernel/billboard/chat.rs

//! The (dev-only) map chat's latest line and who says it: a billboard label
//! as a marker is, standing over the speaker's head rather than on a cell
//! (`scene/place.rs` lays both out).

use std::rc::Rc;

use pxl_spec::space::coord::q3::Q3;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

use crate::billboard::badge::Label;
use crate::sched::edge::wake_at;

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
pub fn shown_line() -> Option<Label> {
    let line = chat().line.as_ref()?;
    if line.said + SHOWN_MS <= now() {
        return None;
    }
    wake_at(line.said + SHOWN_MS);
    Some(Label::Bubble { text: line.text.clone() })
}

/// None while there is no one to speak.
pub fn speak_from(speaker: Option<(Q3, Q3)>) {
    chat().speaker = speaker;
}

pub fn speaker() -> Option<(Q3, Q3)> {
    chat().speaker
}
