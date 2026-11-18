// pxl-core/domain/scene/marker/design.rs

//! What a marker paints, each as its own design on the one atlas: text on a
//! pill, a place as Carto lays it out, or a line said in a bubble. A design
//! describes only what is painted, never who shows it, so two markers that
//! paint alike share one design. A design is measured, given its slot and
//! painted once, and it is shown only once painted (`ready`). Each source
//! chooses the text and colors it paints (`place.rs`, `pin.rs` and
//! `line.rs`). The platform lays out and draws the text (sys/io/text.rs), in
//! a marker's font and at the atlas's scale (`text_raster`).

use std::rc::Rc;

use fnv::{FnvHashMap, FnvHashSet};

use crate::domain::scene::marker::atlas::atlas;
use crate::sys::io::text::{text_lines, text_pixels};
use pxl_global::Global;
use pxl_render::frame::shelf::{NO_SLOT, Paint};
use pxl_render::gpu::text::{Raster, raster_box};
use pxl_render::look::marker::ATLAS_SCALE;
use pxl_render::look::marker::{
    BUBBLE_FONT, BUBBLE_TEXT_W, Font, PILL_FONT, POINT_FONT, bubble_size, pill_size, point_size,
};

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// `text` in `font`, broken into lines no wider than `max_width` (0 for one
/// line), each with its width. None when the platform could not lay it out
/// this frame.
fn lines(font: Font, max_width: f64, text: &str) -> Option<Vec<(&str, f64)>> {
    text_lines(font.weight, f64::from(font.px), max_width, text)
}

/// The width of `text` set on one line in `font`.
fn width(font: Font, text: &str) -> Option<f64> {
    lines(font, 0.0, text).map(|lines| lines.iter().map(|&(_, w)| w).sum())
}

/// One line of `text` in `font` and `color` (0xRRGGBBAA), as the atlas
/// paints it: drawn at the atlas's scale into the line's box (`raster_box`).
/// None when it could not be drawn this frame, and a later frame asks again.
pub(crate) fn text_raster(font: Font, color: u32, text: &str) -> Option<Raster<'static>> {
    let advance = width(font, text)? as f32;
    let (size, origin) = raster_box(font, advance);
    let scale = ATLAS_SCALE as f32;
    let px = text_pixels(
        font.weight,
        f64::from(font.px * scale),
        color,
        size,
        f64::from(origin[0]),
        text,
    )?;
    Some(Raster {
        px,
        width: size[0],
        height: size[1],
        origin,
        advance: advance * scale,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Design {
    /// Text in `ink` on a pill in `fill`.
    Pill { text: Rc<str>, fill: u32, ink: u32 },
    /// A place as Carto lays it out: its Maki symbol when `icon` is set, or
    /// else a dot, in its category's `fill`. Under it stands its name, when it has
    /// one, in its category's text color `ink`, wrapped at `wrap` pixels
    /// (`with_spacing`).
    Point {
        name: Option<Rc<str>>,
        wrap: u32,
        icon: bool,
        symbol: Option<u16>,
        fill: u32,
        ink: u32,
    },
    /// A line said in the chat.
    Bubble { text: Rc<str> },
}

impl Design {
    /// The design as laid out where a place keeps `spacing` pixels to itself
    /// (param.rs `SPACING_PX`), with the width of the region it keeps
    /// (`set.rs`). A place's name is wrapped at `spacing`, and its region is
    /// `spacing` wide. Any other design keeps a region of its own width,
    /// given here as 0, which the candidates widen to that width.
    pub(super) fn with_spacing(mut self, spacing: f64) -> (Design, f64) {
        match &mut self {
            Design::Point { name, wrap, .. } => {
                if name.is_some() {
                    *wrap = spacing as u32;
                }
                (self, spacing)
            }
            _ => (self, 0.0),
        }
    }
}

/// A design as the atlas holds it: its size, laid out once, and its slot,
/// which is `NO_SLOT` until one is asked for (`slot`).
struct Sprite {
    size: [f64; 3],
    /// Its text as it was laid out when it was sized. The client's layouts
    /// are a cache the client may drop, so the lines are kept here.
    lines: Vec<String>,
    slot: i32,
}
static SPRITES: Global<FnvHashMap<Design, Sprite>> = Global::new();
fn sprites() -> &'static mut FnvHashMap<Design, Sprite> {
    SPRITES.get(FnvHashMap::default)
}

/// Drops every design not in `designs`, which holds those a marker shown or
/// waiting uses, and frees the dropped designs' slots. Returns whether any
/// slot was freed, since a design the atlas had no room for may fit now.
pub(super) fn retain(designs: &FnvHashSet<Design>) -> bool {
    let mut dropped = false;
    sprites().retain(|design, b| {
        if designs.contains(design) {
            return true;
        }
        if b.slot != NO_SLOT {
            atlas().release(b.slot as u32);
            dropped = true;
        }
        false
    });
    // The atlas's live set has just shrunk, so this is when the atlas may
    // stop being as large as the densest view made it.
    if dropped {
        atlas().compact();
    }
    dropped
}

/// Whether the design will become ready (`ready`) with nothing else
/// changing. Either its text has been asked of the client and a later frame
/// brings the answer, or its slot is given and the render will paint it. A
/// design the atlas has no room for becomes ready only once a slot is freed
/// (`retain`), so it asks for no frame of its own.
pub(super) fn coming(design: &Design) -> bool {
    sprites()
        .get(design)
        .is_none_or(|b| b.slot != NO_SLOT && !atlas().is_painted(b.slot as u32))
}

/// The design's width, its height, and how far down from its top it stands.
/// None when its text could not be laid out this frame.
pub(super) fn size(design: &Design) -> Option<[f64; 3]> {
    if let Some(b) = sprites().get(design) {
        return Some(b.size);
    }
    let laid = |font, wrap, text: &str| -> Option<(f64, Vec<String>)> {
        let laid = lines(font, wrap, text)?;
        Some((
            laid.iter().map(|&(_, w)| w).fold(0.0, f64::max),
            laid.iter().map(|&(l, _)| l.trim().to_owned()).collect(),
        ))
    };
    let (size, lines) = match design {
        Design::Pill { text, .. } => (pill_size(width(PILL_FONT, text)?.ceil()), Vec::new()),
        Design::Point {
            name: None, icon, ..
        } => (point_size(*icon, None), Vec::new()),
        Design::Point {
            name: Some(name),
            wrap,
            icon,
            ..
        } => {
            let (w, lines) = laid(POINT_FONT, f64::from(*wrap), name)?;
            (point_size(*icon, Some((w, lines.len()))), lines)
        }
        Design::Bubble { text } => {
            let (w, lines) = laid(BUBBLE_FONT, BUBBLE_TEXT_W, text)?;
            (bubble_size(w, lines.len()), lines)
        }
    };
    sprites().insert(
        design.clone(),
        Sprite {
            size,
            lines,
            slot: NO_SLOT,
        },
    );
    Some(size)
}

/// Whether the design is laid out and painted, so drawing it shows its
/// text. It asks the atlas for no slot, which `slot` does.
pub(super) fn ready(design: &Design) -> bool {
    size(design).is_some()
        && sprites()
            .get(design)
            .is_some_and(|b| b.slot != NO_SLOT && atlas().is_painted(b.slot as u32))
}

/// The design's slot on the atlas, asked for on first use. The design must
/// have been measured (`size`) first.
pub(super) fn slot(design: &Design) -> i32 {
    let b = sprites().get_mut(design).or_bug(Errno::ENOTRECOVERABLE);
    if b.slot == NO_SLOT {
        let [w, h, _] = b.size;
        b.slot = atlas().add(
            w * ATLAS_SCALE,
            h * ATLAS_SCALE,
            match design {
                Design::Pill { text, fill, ink } => Paint::Pill {
                    text: text.to_string(),
                    fill: *fill,
                    ink: *ink,
                    logical_w: w,
                },
                Design::Bubble { .. } => Paint::Bubble {
                    lines: b.lines.clone(),
                    box_w: w,
                },
                &Design::Point {
                    icon,
                    symbol,
                    fill,
                    ink,
                    ..
                } => Paint::Point {
                    lines: b.lines.clone(),
                    icon,
                    symbol,
                    fill,
                    ink,
                    logical_w: w,
                },
            },
        );
    }
    b.slot
}
