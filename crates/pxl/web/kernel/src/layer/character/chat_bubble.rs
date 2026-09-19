// pxl-kernel/layer/character/chat_bubble.rs

//! A billboarded speech bubble over the companion's head, showing the most
//! recent line from the (currently dev-only) map chat. World-anchored-billboard
//! tier (see issue/20260907-world-rendering-tiers.md); the render is a
//! billboard set standing on the companion's actor, drawn from the sheet POI
//! badges share. What this owns is only what's chat-bubble-specific: reading
//! the chat lines, the show/expire timer, and laying out the notched
//! speech-bubble shape the shell paints.

use pxl_render::asset::atlas::{DrawSpec, NO_SLOT};
use pxl_render::gpu::surface::DPR_MAX;

use crate::global::Global;
use crate::host::{measure_text, wall_now};
use crate::layer::billboard::canvas_scale::{FONT_FAMILY, FONT_WEIGHT_BOLD};
use crate::layer::billboard::canvas_text::wrap_text;
use crate::layer::billboard::sheet::{billboard_sheet, follow_billboard_set, new_billboard_set, remove_billboard_set, write_billboard_set, BILLBOARD_ORDER};

// ts (wall clock at append) lets the bubble derive its own remaining show
// time from wall-clock, not from when it happened to start showing — without
// it, a character swap resets its state and it has no way to tell an old,
// already-dismissed line apart from a brand new one.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatLine {
    pub text: String,
    pub ts: f64,
}

/// The last line the shell sent; the chat log itself is the shell's.
static CHAT_LINE: Global<Option<ChatLine>> = Global::new();

/// A line the shell's chat appended.
pub fn append_chat_line(text: &str) {
    *CHAT_LINE.get(|| None) = Some(ChatLine {
        text: text.to_string(),
        ts: wall_now(),
    });
}

// A new chat line shows for this long, then the bubble disappears — chat itself
// has no such expiry, so this is tracked here, not in the shell's chat log.
const SHOW_DURATION_MS: f64 = 8000.0;

fn font() -> String {
    format!("{FONT_WEIGHT_BOLD} 14px {FONT_FAMILY}")
}

const PADDING_X: f64 = 11.0;
const PADDING_Y: f64 = 8.0;
const LINE_H: f64 = 17.0;
const TAIL_H: f64 = 10.0;

// A long message wraps onto more lines instead of growing the bubble
// arbitrarily wide.
const MAX_TEXT_WIDTH_PX: f64 = 220.0;

// Air gap between the character model's own top and the bubble's tail tip: a
// fixed clearance added on top of wherever the model's bounding box actually
// ends, rather than an arbitrary multiplier of its height.
const HEAD_CLEARANCE_U: f64 = 0.22;

/// A message laid out: its wrapped lines and the bubble's logical size.
#[derive(Clone, Debug, PartialEq)]
pub struct BubbleLayout {
    pub lines: Vec<String>,
    pub box_w: f64,
    pub box_h: f64,
    pub logical_h: f64,
}

pub fn layout_bubble(text: &str) -> BubbleLayout {
    let f = font();
    let measure = |s: &str| measure_text(&f, s);
    let lines = wrap_text(&measure, text, MAX_TEXT_WIDTH_PX);
    let text_w = lines.iter().map(|l| measure(l)).fold(f64::NEG_INFINITY, f64::max).ceil();
    let box_w = text_w + PADDING_X * 2.0;
    let box_h = lines.len() as f64 * LINE_H + PADDING_Y * 2.0;
    BubbleLayout {
        lines,
        box_w,
        box_h,
        logical_h: box_h + TAIL_H,
    }
}

struct Held {
    set: u32,
    slot: u32,
    layout: BubbleLayout,
}

struct Bubble {
    /// What the bubble is set up for: the actor and the text.
    shown: Option<(u32, String)>,
    held: Option<Held>,
    /// What was last written into the set, so an unchanged frame writes nothing.
    written: Option<(u32, f64, u32)>,
}

static BUBBLE: Global<Bubble> = Global::new();

/// The bubble's turn, for the character with `actor` whose model's top is at
/// `anchor_z`: which line is showing, the set that shows it, and the instance
/// written again when the sheet is replaced, which moves its rectangle.
pub fn chat_bubble_step(actor: Option<u32>, anchor_z: f64) {
    let b = BUBBLE.get(|| Bubble {
        shown: None,
        held: None,
        written: None,
    });

    // Driven by the line's wall-clock ts, so this stays correct across a
    // character swap, which would otherwise make an old, already-dismissed line
    // look brand new again.
    let visible = CHAT_LINE.get(|| None).as_ref().filter(|l| l.ts + SHOW_DURATION_MS - wall_now() > 0.0).map(|l| l.text.clone());
    let want = actor.zip(visible);

    if want != b.shown {
        if let Some(held) = b.held.take() {
            remove_billboard_set(held.set);
            billboard_sheet().release(held.slot);
        }
        b.written = None;
        b.shown = want.clone();
        if let Some((_, text)) = want {
            let layout = layout_bubble(&text);
            let slot = billboard_sheet().add(
                layout.box_w * DPR_MAX,
                layout.logical_h * DPR_MAX,
                DrawSpec::Bubble {
                    lines: layout.lines.clone(),
                    box_w: layout.box_w,
                    box_h: layout.box_h,
                },
            );
            if slot != NO_SLOT {
                b.held = Some(Held {
                    set: new_billboard_set(BILLBOARD_ORDER::chat),
                    slot: slot as u32,
                    layout,
                });
            }
        }
    }

    let (Some(held), Some(actor)) = (&b.held, actor) else { return };
    // The tail tip is the anchor, at the model's top plus the clearance.
    let tip_z = anchor_z + HEAD_CLEARANCE_U;
    let sheet = billboard_sheet();
    let stamp = (actor, tip_z, sheet.sheet_generation());
    if b.written == Some(stamp) {
        return;
    }
    b.written = Some(stamp);
    let s = held.slot;
    // The tail tip is the anchor, at the bottom of the sprite.
    let lane = [
        0.0,
        0.0,
        0.0,
        held.layout.box_w as f32,
        held.layout.logical_h as f32,
        sheet.u0(s) as f32,
        sheet.v0(s) as f32,
        sheet.u1(s) as f32,
        sheet.v1(s) as f32,
        1.0,
    ];
    follow_billboard_set(held.set, actor, tip_z);
    write_billboard_set(held.set, &lane);
}
