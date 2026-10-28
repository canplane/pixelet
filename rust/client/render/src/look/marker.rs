// pxl-render/look/marker.rs

//! How each marker's design is laid out, in logical pixels: its fonts, its
//! sizes and its colors, and where its anchor stands down from its top. The
//! pass paints by them (`gpu/pass/marker.rs`), and the kernel sizes a
//! marker's sprite by them before it is painted (pxl-kernel
//! domain/scene/marker/design.rs).

use pxl_spec::asset::symbol::SYMBOL_PX;

use crate::gpu::sheet::{Halo, SHEET_SCALE};
use crate::gpu::text::Font;

// Regular and semibold, not canvas's "bold" (700); the system faces carry a real
// semibold, so neither is synthetic.

const REGULAR: u32 = 400;
const SEMIBOLD: u32 = 600;

pub const BLACK: u32 = 0x000000ff;
pub const WHITE: u32 = 0xffffffff;

/// A marker's words on a pill, the dot under it at its cell's center.
pub const PILL_FONT: Font = Font {
    weight: REGULAR,
    px: 12.0,
};
const PILL_INNER: f32 = 20.0;
const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 7.0;
pub const DOT_DIAM: f32 = 6.0;
const DOT_GAP: f32 = 4.0;
pub const DOT_BORDER: f32 = 1.0;
pub const PILL_H: f32 = PILL_INNER + PAD_Y;
pub const PILL_R: f32 = PILL_H / 2.0;
pub const DOT_CY: f32 = PILL_H + DOT_GAP + DOT_DIAM / 2.0;
/// A dark pill, its words white.
pub const PILL_FILL: u32 = 0x14141ab8;
pub const PILL_INK: u32 = WHITE;
pub const DOT_FILL: u32 = 0xffffff99;
pub const DOT_RING: u32 = 0xffffff40;
/// A pill's colors turned over, a light pill, its words dark, so a number
/// on it does not read as words that are a number.
pub const COUNT_FILL: u32 = 0xffffffe6;
pub const COUNT_INK: u32 = 0x14141aff;

/// A pill's sprite for words as wide as `text_w`: its width, height down to
/// the dot's foot, and where what it marks is, at the dot's.
pub fn pill_size(text_w: f64) -> [f64; 3] {
    [
        (text_w + 2.0 * f64::from(PAD_X)).max(f64::from(PILL_H)),
        f64::from(PILL_H + DOT_GAP + DOT_DIAM),
        f64::from(DOT_CY),
    ]
}

// A place as a map app draws one: its symbol white on a disc of its
// category's fill, rimmed white so it holds on any ground, lit from above and
// casting a soft shadow, or before its icon's zoom a small disc of the same; the name under it in its category's
// color, semibold, in a near-opaque white halo. Which places and when are
// Carto's (`pxl-spec bake/place_theme.spec.rs`); how they look is not.

pub const POINT_FONT: Font = Font {
    weight: SEMIBOLD,
    px: 12.0,
};
/// A symbol's disc across: the symbol (`SYMBOL_PX` at the sheet's scale)
/// with four pixels of fill round it.
const POINT_DISC: f32 = SYMBOL_PX as f32 / SHEET_SCALE as f32 + 2.0 * 4.0;
const POINT_DOT: f32 = 8.0;
pub const POINT_RIM: f32 = 1.5;
pub const POINT_RIM_FILL: u32 = 0xffffffe6;
/// The fill lighter at the disc's top and darker at its foot by this much.
pub const POINT_SHADE: f32 = 0.18;
/// A shadow as wide as the disc and a little more, dropped and blurred, and
/// the room it takes round the disc.
pub const POINT_SHADOW_DY: f32 = 1.5;
pub const POINT_SHADOW_BLUR: f32 = 3.0;
pub const POINT_SHADOW_FILL: u32 = 0x00000059;
pub const POINT_PAD: f32 = 3.0;
/// From the mark's foot to the name's first line.
pub const POINT_GAP: f32 = 2.0;
const POINT_HALO: f32 = 2.0;
const POINT_HALO_FILL: u32 = 0xfffffff2;
/// Under the halo as under the disc: dropped, and soft past the halo's edge.
pub const POINT_TEXT_HALO: Halo = Halo {
    color: POINT_HALO_FILL,
    radius: POINT_HALO,
    shadow: 0x00000038,
    shadow_radius: POINT_HALO + 1.5,
    shadow_dy: 1.0,
};
/// A line, the font's size and its halo above and below.
pub const POINT_LINE: f32 = POINT_FONT.px + 2.0 * POINT_HALO;
/// From one line to the next, as Mapnik sets it (text_line.cpp): the face's
/// own line height plus Carto's @standard-line-spacing-size, −0.15 em. The
/// face is the system's, whose metrics the sheet does not read; 1.2 em is
/// CSS's `line-height: normal` at its high end (CSS 2.1 §10.8.1).
pub const POINT_PITCH: f32 = (1.2 - 0.15) * POINT_FONT.px;

/// The disc's side and the shadow's room round it.
pub fn point_mark(icon: bool) -> f32 {
    2.0 * POINT_PAD + if icon { POINT_DISC } else { POINT_DOT }
}

/// A point's sprite, its mark an icon's or a dot's, for a name laid out in
/// `lines` as wide as `text_w`, or none: its width, height and where the
/// place is, down from its top.
pub fn point_size(icon: bool, text: Option<(f64, usize)>) -> [f64; 3] {
    let mark = f64::from(point_mark(icon));
    match text {
        None => [mark, mark, mark / 2.0],
        Some((w, lines)) => {
            // The last line's shadow reaches past its halo.
            let shadow =
                f64::from(POINT_TEXT_HALO.shadow_radius - POINT_HALO + POINT_TEXT_HALO.shadow_dy);
            let h = mark - f64::from(POINT_PAD)
                + f64::from(POINT_GAP)
                + f64::from(POINT_PITCH) * (lines.max(1) - 1) as f64
                + f64::from(POINT_LINE)
                + shadow;
            [
                (w + 2.0 * f64::from(POINT_TEXT_HALO.shadow_radius))
                    .ceil()
                    .max(mark),
                h.ceil(),
                mark / 2.0,
            ]
        }
    }
}

/// A line said in the chat, black on a white bubble whose tail points at its
/// speaker.
pub const BUBBLE_FONT: Font = Font {
    weight: SEMIBOLD,
    px: 14.0,
};
/// The widest a bubble's line runs before it wraps.
pub const BUBBLE_TEXT_W: f64 = 220.0;
const PADDING_X: f32 = 11.0;
pub const PADDING_Y: f32 = 8.0;
pub const LINE_H: f32 = 17.0;
pub const BORDER: f32 = 2.0;
pub const NOTCH: f32 = 3.0;
pub const TAIL_W: f32 = 14.0;
pub const TAIL_H: f32 = 10.0;

/// A bubble's box for `lines` lines, its tail not counted.
pub fn bubble_box_h(lines: usize) -> f32 {
    lines as f32 * LINE_H + PADDING_Y * 2.0
}

/// A bubble's sprite for `lines` lines as wide as `text_w`: its width,
/// height with the tail, and where its speaker is, at the tail's tip.
pub fn bubble_size(text_w: f64, lines: usize) -> [f64; 3] {
    let h = f64::from(bubble_box_h(lines) + TAIL_H);
    [text_w.ceil() + f64::from(PADDING_X) * 2.0, h, h]
}
