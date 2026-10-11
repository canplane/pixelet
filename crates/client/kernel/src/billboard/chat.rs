// pxl-kernel/billboard/chat.rs

//! The (dev-only) map chat's latest line and who says it: a billboard label
//! as a marker is, standing over the speaker's head rather than on a cell
//! (`chunk/place.rs` lays both out).

use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
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
    text: String,
    said: f64,
}

#[derive(Default)]
struct Chat {
    line: Option<Line>,
    /// Where the speaker stands, and the point over its head a line hangs
    /// from, as the scene hands them down this turn.
    speaker: Option<(Pvf3, Pvf3)>,
}

static CHAT: Global<Chat> = Global::new();

fn chat() -> &'static mut Chat {
    CHAT.get(Chat::default)
}

pub fn append_chat_line(text: &str) {
    chat().line = Some(Line { text: text.to_string(), said: now() });
}

/// The line to show, none once shown its while; the turn is woken then.
pub fn shown_line() -> Option<Label> {
    let said = chat().line.as_ref()?.said;
    if said + SHOWN_MS <= now() {
        return None;
    }
    wake_at(said + SHOWN_MS);
    Some(Label::Chat { said: said.to_bits() })
}

/// The line said at `said`'s bits; none once another has been.
pub fn line_said(said: u64) -> Option<String> {
    chat().line.as_ref().filter(|l| l.said.to_bits() == said).map(|l| l.text.clone())
}

/// None while there is no one to speak.
pub fn speak_from(speaker: Option<(Pvf3, Pvf3)>) {
    chat().speaker = speaker;
}

pub fn speaker() -> Option<(Pvf3, Pvf3)> {
    chat().speaker
}
