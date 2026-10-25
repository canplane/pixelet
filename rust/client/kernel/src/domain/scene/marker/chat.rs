// pxl-kernel/domain/scene/marker/chat.rs

//! The (dev-only) map chat's latest line: what was said and when, its words
//! on a bubble. Its speaker is the person, who hangs the bubble over
//! its head and says whether it shows (pxl-kernel
//! domain/scene/character/companion.rs `line_hangs`); `line.rs` hands it to
//! the arrangement.

use std::rc::Rc;

use xpute_guest::clock::now;
use xpute_guest::global::Global;

use crate::domain::scene::marker::design::Design;
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
