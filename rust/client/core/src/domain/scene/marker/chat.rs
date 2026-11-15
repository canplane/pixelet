// pxl-core/domain/scene/marker/chat.rs

//! The latest line of the map chat, a feature for development only: what was
//! said, when, and its text on a bubble. Its speaker is the person, who hangs
//! the bubble over its head and decides whether it shows
//! (domain/scene/character/companion.rs `line_hangs`). `line.rs` hands the
//! line to the arrangement.

use std::rc::Rc;

use pxl_global::Global;
use pxl_spec::asset::marker::{CHAT_TEXT_CHARS, text_fits};
use xpute_guest::clock::now;
use xpute_kit::status::errno::Errno;

use crate::domain::scene::marker::design::Design;
use crate::domain::scene::marker::param::SHOWN_MS;
use crate::sys::sched::invalidate_at;

/// A line is stamped with the host's clock, not the frame's. A line said
/// after a still spell would otherwise take the last frame's time and read
/// as long gone. The stamp is the line's own, so a swap of character cannot
/// make a dismissed line look new.
struct Line {
    text: Rc<str>,
    at_ms: f64,
}

#[derive(Default)]
struct Chat {
    line: Option<Line>,
}

static CHAT: Global<Chat> = Global::new();

fn chat() -> &'static mut Chat {
    CHAT.get(Chat::default)
}

/// Says the line `text`; a line past `CHAT_TEXT_CHARS` is refused whole
/// (pxl-spec asset/marker.rs `text_fits`), EMSGSIZE, and the last line stays.
pub(crate) fn append_chat_line(text: &str) -> Result<(), Errno> {
    if !text_fits(text, CHAT_TEXT_CHARS) {
        return Err(Errno::EMSGSIZE);
    }
    chat().line = Some(Line {
        text: text.into(),
        at_ms: now(),
    });
    Ok(())
}

/// The line to show, in its bubble, or None once it has shown for
/// `SHOWN_MS`. While it shows, the frame is woken for the moment it expires.
pub(super) fn shown_line() -> Option<Design> {
    let line = chat().line.as_ref()?;
    if line.at_ms + SHOWN_MS <= now() {
        return None;
    }
    invalidate_at(line.at_ms + SHOWN_MS);
    Some(Design::Bubble {
        text: line.text.clone(),
    })
}
