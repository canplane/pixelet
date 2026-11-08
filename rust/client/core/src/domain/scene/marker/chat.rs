// pxl-core/domain/scene/marker/chat.rs

//! The latest line of the map chat, a feature for development only: what was
//! said, when, and its text on a bubble. Its speaker is the person, who hangs
//! the bubble over its head and decides whether it shows
//! (domain/scene/character/companion.rs `line_hangs`). `line.rs` hands the
//! line to the arrangement.

use std::rc::Rc;

use xpute_core::clock::now;
use xpute_core::global::Global;

use crate::domain::scene::marker::design::Design;
use crate::sys::sched::edge::wake_at;

/// How long a line shows, in ms. The chat has no expiry of its own.
const SHOWN_MS: f64 = 8000.0;

/// A line is stamped with the runtime's clock, not the frame's. A line said
/// after a still spell would otherwise take the last frame's time and read
/// as long gone. The stamp is the line's own, so a swap of character cannot
/// make a dismissed line look new.
struct Line {
    text: Rc<str>,
    said: f64,
}

#[derive(Default)]
struct Chat {
    line: Option<Line>,
}

static CHAT: Global<Chat> = Global::new();

fn chat() -> &'static mut Chat {
    CHAT.get(Chat::default)
}

pub fn append_chat_line(text: &str) {
    chat().line = Some(Line {
        text: text.into(),
        said: now(),
    });
}

/// The line to show, in its bubble, or None once it has shown for
/// `SHOWN_MS`. While it shows, the turn is woken for the moment it expires.
pub(super) fn shown_line() -> Option<Design> {
    let line = chat().line.as_ref()?;
    if line.said + SHOWN_MS <= now() {
        return None;
    }
    wake_at(line.said + SHOWN_MS);
    Some(Design::Bubble {
        text: line.text.clone(),
    })
}
