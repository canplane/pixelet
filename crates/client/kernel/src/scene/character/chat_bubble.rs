// pxl-kernel/scene/character/chat_bubble.rs

//! A billboarded speech bubble over the companion's head, showing the most
//! recent line of the (dev-only) map chat for a while.

use pxl_render::asset::atlas::{DrawSpec, NO_SLOT};
use pxl_render::gpu::pass::badges::BUBBLE_FONT;
use pxl_render::gpu::surface::DPR_MAX;

use crate::scene::billboard::field::billboard_instance;
use crate::scene::billboard::sheet::{billboard_sheet, follow_billboard_set, new_billboard_set, remove_billboard_set, write_billboard_set, BillboardOrder};
use crate::text::text_lines;
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;

// `ts` lets the bubble time itself from when the line was said, so a
// character swap cannot make a dismissed line look new.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatLine {
    pub text: String,
    pub ts: f64,
}

static CHAT_LINE: Global<Option<ChatLine>> = Global::new();

pub fn append_chat_line(text: &str) {
    *CHAT_LINE.get(|| None) = Some(ChatLine { text: text.to_string(), ts: now() });
}

// Chat has no expiry of its own; the bubble's is tracked here.
const SHOW_DURATION_MS: f64 = 8000.0;

const PADDING_X: f64 = 11.0;
const PADDING_Y: f64 = 8.0;
const LINE_H: f64 = 17.0;
const TAIL_H: f64 = 10.0;

const MAX_TEXT_WIDTH_PX: f64 = 220.0;

// A fixed clearance over the model's bounding box, not a multiplier of its height.
const HEAD_CLEARANCE_U: f64 = 0.22;

#[derive(Clone, Debug, PartialEq)]
pub struct BubbleLayout {
    pub lines: Vec<String>,
    pub box_w: f64,
    pub box_h: f64,
    pub logical_h: f64,
}

/// None until `text` is laid out, which a later turn has.
pub fn layout_bubble(text: &str) -> Option<BubbleLayout> {
    let laid = text_lines(BUBBLE_FONT, MAX_TEXT_WIDTH_PX, text)?;
    let text_w = laid.iter().map(|&(_, w)| w).fold(f64::NEG_INFINITY, f64::max).ceil();
    let lines: Vec<String> = laid.iter().map(|&(line, _)| line.trim().to_owned()).collect();
    let box_w = text_w + PADDING_X * 2.0;
    let box_h = lines.len() as f64 * LINE_H + PADDING_Y * 2.0;
    Some(BubbleLayout {
        lines,
        box_w,
        box_h,
        logical_h: box_h + TAIL_H,
    })
}

struct Held {
    set: u32,
    slot: u32,
    layout: BubbleLayout,
}

struct Bubble {
    /// The actor and the text.
    shown: Option<(u32, String)>,
    held: Option<Held>,
    /// What was last written into the set, so an unchanged frame writes nothing.
    written: Option<(u32, f64, u32)>,
}

static BUBBLE: Global<Bubble> = Global::new();

pub fn chat_bubble_step(actor: Option<u32>, anchor_z: f64) {
    let b = BUBBLE.get(|| Bubble {
        shown: None,
        held: None,
        written: None,
    });

    // The host's clock, not the frame's wall clock: a line said after a still
    // spell would be stamped with the last frame's time and read as long gone.
    let visible = CHAT_LINE.get(|| None).as_ref().filter(|l| l.ts + SHOW_DURATION_MS - now() > 0.0).map(|l| l.text.clone());
    let want = actor.zip(visible);

    if want != b.shown {
        if let Some(held) = b.held.take() {
            remove_billboard_set(held.set);
            billboard_sheet().release(held.slot);
        }
        b.written = None;
        b.shown = want.clone();
        if let Some((_, text)) = want {
            let Some(layout) = layout_bubble(&text) else {
                b.shown = None;
                return;
            };
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
                    set: new_billboard_set(BillboardOrder::Chat),
                    slot: slot as u32,
                    layout,
                });
            }
        }
    }

    let (Some(held), Some(actor)) = (&b.held, actor) else { return };
    let tip_z = anchor_z + HEAD_CLEARANCE_U;
    let sheet = billboard_sheet();
    let stamp = (actor, tip_z, sheet.sheet_generation());
    if b.written == Some(stamp) {
        return;
    }
    b.written = Some(stamp);
    let lane = billboard_instance([0.0; 3], [held.layout.box_w, held.layout.logical_h], held.slot, 1.0);
    follow_billboard_set(held.set, actor, tip_z);
    write_billboard_set(held.set, &lane);
}
