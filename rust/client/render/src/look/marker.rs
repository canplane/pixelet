// pxl-render/look/marker.rs

//! The layout of each marker's design, in logical pixels: its fonts, sizes and colors, and how
//! far below its top its anchor stands. The marker pass paints by these numbers
//! (pass/marker.rs), and the core sizes a marker's sprite by them before it is painted
//! (pxl-core domain/scene/marker/design.rs).

use pxl_spec::asset::symbol::{SYMBOL_PX, SYMBOL_RATIO};

/// The device pixels the atlas draws per logical pixel. It is the ratio the symbols' masks are
/// rasterized at, so they are laid texel for texel (pxl-spec asset/symbol.spec.rs
/// `SYMBOL_RATIO`). A display finer than this shows a marker scaled up.
pub const ATLAS_SCALE: f64 = SYMBOL_RATIO as f64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Font {
    pub weight: u32,
    pub px: f32,
}

/// A halo round a line's glyphs, with its color and radius, and a shadow under it, with its
/// color, radius and drop. Lengths are in logical pixels.
#[derive(Clone, Copy)]
pub struct Halo {
    pub color: u32,
    pub radius: f32,
    pub shadow: u32,
    pub shadow_radius: f32,
    pub shadow_dy: f32,
}

const REGULAR: u32 = 400;
const SEMIBOLD: u32 = 600;

pub const BLACK: u32 = 0x000000ff;
pub const WHITE: u32 = 0xffffffff;

/// A marker's words sit on a pill, and the dot under the pill stands at its cell's center.
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
/// The pill is dark, and its words are white.
pub const PILL_FILL: u32 = 0x14141ab8;
pub const PILL_INK: u32 = WHITE;
pub const DOT_FILL: u32 = 0xffffff99;
pub const DOT_RING: u32 = 0xffffff40;
/// These are the pill's colors turned over: a light pill with dark words.
pub const CNT_FILL: u32 = 0xffffffe6;
pub const CNT_INK: u32 = 0x14141aff;

/// A pill's sprite for words `text_w` wide: its width, its height down to the foot of the dot,
/// and how far below its top the marked point is, at the dot's center.
pub fn pill_size(text_w: f64) -> [f64; 3] {
    [
        (text_w + 2.0 * f64::from(PAD_X)).max(f64::from(PILL_H)),
        f64::from(PILL_H + DOT_GAP + DOT_DIAM),
        f64::from(DOT_CY),
    ]
}

// A place is drawn as a map app draws one. Its symbol is white on a disc of its category's
// fill, rimmed white so it holds on any ground, lit from above and casting a soft shadow.
// Before its icon's zoom it is a small disc of the same fill. Its name stands under it in its
// category's color, semibold, in a near-opaque white halo. Which places are drawn, and from
// which zoom, are Carto's choices (`pxl-spec bake/place_theme.spec.rs`).

pub const POINT_FONT: Font = Font {
    weight: SEMIBOLD,
    px: 12.0,
};
/// The width of a symbol's disc: the symbol (`SYMBOL_PX` at the ratio it is rasterized at)
/// with four pixels of fill round it.
const POINT_DISC: f32 = SYMBOL_PX as f32 / SYMBOL_RATIO as f32 + 2.0 * 4.0;
const POINT_DOT: f32 = 8.0;
pub const POINT_RIM: f32 = 1.5;
pub const POINT_RIM_FILL: u32 = 0xffffffe6;
/// How much lighter the fill is at the disc's top, and darker at its foot.
pub const POINT_SHADE: f32 = 0.18;
/// The disc's shadow, a little wider than the disc, dropped and blurred, and the room it takes
/// round the disc.
pub const POINT_SHADOW_DY: f32 = 1.5;
pub const POINT_SHADOW_BLUR: f32 = 3.0;
pub const POINT_SHADOW_FILL: u32 = 0x00000059;
pub const POINT_PAD: f32 = 3.0;
/// This is the distance from the mark's foot to the name's first line.
pub const POINT_GAP: f32 = 2.0;
const POINT_HALO: f32 = 2.0;
const POINT_HALO_FILL: u32 = 0xfffffff2;
/// The halo has a shadow as the disc does: dropped, and soft past the halo's edge.
pub const POINT_TEXT_HALO: Halo = Halo {
    color: POINT_HALO_FILL,
    radius: POINT_HALO,
    shadow: 0x00000038,
    shadow_radius: POINT_HALO + 1.5,
    shadow_dy: 1.0,
};
/// A line's height: the font's size with its halo above and below.
pub const POINT_LINE: f32 = POINT_FONT.px + 2.0 * POINT_HALO;
/// The distance from one line to the next, as Mapnik sets it (text_line.cpp): the face's
/// own line height plus Carto's @standard-line-spacing-size, −0.15 em. The
/// face is the system's, whose metrics the atlas does not read; 1.2 em is
/// CSS's `line-height: normal` at its high end (CSS 2.1 §10.8.1).
pub const POINT_PITCH: f32 = (1.2 - 0.15) * POINT_FONT.px;

/// The side of a point's mark: its disc or its dot, with the shadow's room round it.
pub fn point_mark(icon: bool) -> f32 {
    2.0 * POINT_PAD + if icon { POINT_DISC } else { POINT_DOT }
}

/// A point's sprite, its mark an icon's or a dot's, for a name of `lines` lines `text_w` wide,
/// or for no name: its width, its height, and how far below its top the place is.
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

/// The height of a bubble's box for `line_cnt` lines, without its tail.
pub fn bubble_rect_h(line_cnt: usize) -> f32 {
    line_cnt as f32 * LINE_H + PADDING_Y * 2.0
}

/// A bubble's sprite for `line_cnt` lines `text_w` wide: its width, its height with the tail, and
/// how far below its top the speaker is, at the tail's tip.
pub fn bubble_size(text_w: f64, line_cnt: usize) -> [f64; 3] {
    let h = f64::from(bubble_rect_h(line_cnt) + TAIL_H);
    [text_w.ceil() + f64::from(PADDING_X) * 2.0, h, h]
}
