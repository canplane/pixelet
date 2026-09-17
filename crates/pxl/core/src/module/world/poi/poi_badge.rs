// pxl_core/module/world/poi/poi_badge.rs

//! One POI's drawn badge, built once and kept.
//!
//! Holding badges by id fixes two things that look unrelated and are not. A
//! badge made when it entered the drawn set and destroyed when it left created
//! and destroyed browser-owned graphics objects at interaction rate — the 4.9 s
//! stall measured in [issue/20260910-where-the-allocation-goes.md] is that class
//! of cost. And a collision pass needs each candidate's screen box before
//! deciding what to draw, and the box comes from its width — measuring three
//! hundred candidates per tick by building three hundred canvases is not an
//! option; looking up three hundred cached sizes is free. Decluttering and the
//! texture churn turned out to share one prerequisite.
//!
//! Eviction runs on the same trigger the index rebuild does, because it is the
//! same event: a page leaving is exactly when a badge stops being reachable.

use pxl_shared::bake::format::PoiPage;
use xpute_core::ds::map::{I64Keys, IndexedMap, U32Keys};

use super::poi_theme::resolve_poi_theme;
use crate::module::world::billboard::atlas::{BillboardAtlas, DrawSpec, NO_SLOT};
use crate::module::world::billboard::canvas_scale::{FONT_FAMILY, FONT_WEIGHT_REGULAR};
use crate::module::world::billboard::sheet::billboard_sheet;
use crate::sys::global::{now, Global};
use crate::sys::host::measure_text;

fn font() -> String {
    format!("{FONT_WEIGHT_REGULAR} 12px {FONT_FAMILY}")
}

const NAME_MAX_CHARS: usize = 12;
const ICON_SIZE: f64 = 20.0;
// Icon-to-text gap and text's own right-edge margin — kept equal deliberately:
// the text's own left and right breathing room should match each other.
const GAP: f64 = 8.0;
const PAD_Y: f64 = 7.0;
/// Supersampling, and it should equal the renderer's own device pixel ratio.
/// The tick clamps that to [1, 2], and the distance fade returns at most 1, so a
/// billboard's drawn size never exceeds its logical size in CSS pixels — two is
/// the exact requirement rather than a guess. It was 3, which is 2.25x the
/// pixels for nothing: drawing at 3 to display at 2 is a minification, so it
/// went through the mipmap chain and came out softer.
pub const RENDER_SCALE: f64 = 2.0;

// A small dot under the pill marking the actual ground point, since the pill
// itself floats above and offset from it.
const DOT_DIAM: f64 = 6.0;
const DOT_GAP: f64 = 4.0;

const PILL_H: f64 = ICON_SIZE + PAD_Y;
const PILL_R: f64 = PILL_H / 2.0;
const TEXT_X: f64 = PILL_R + ICON_SIZE / 2.0 + GAP;
const LOGICAL_H: f64 = PILL_H + DOT_GAP + DOT_DIAM;
const DOT_CY: f64 = PILL_H + DOT_GAP + DOT_DIAM / 2.0;

/// A badge's size, without the badge — everything the collision pass needs,
/// since it works in screen boxes. Only the width varies: it is measured text,
/// cached below.
pub const BADGE_LOGICAL_H: f64 = LOGICAL_H;
pub const BADGE_CENTER_Y: f64 = DOT_CY;
pub const BADGE_COLLIDE_H: f64 = PILL_H;

/// One sheet for every badge this layer draws, places and clusters alike —
/// billboard/sheet.rs's, which the chat bubble draws into too.
pub fn badge_atlas() -> &'static mut BillboardAtlas {
    billboard_sheet()
}

/// What is known about each badge: its measured width, and the atlas slot it
/// is drawn in once it has been (NO_SLOT until then). A place by its id hash, a
/// cluster by its count.
struct BadgeTable<K: xpute_core::ds::map::KeyTraits> {
    index: IndexedMap<K>,
    logical_w: Vec<f64>,
    slot: Vec<i32>,
    /// Badges drawn in the sheet.
    built: u32,
}

impl<K: xpute_core::ds::map::KeyTraits> BadgeTable<K> {
    fn new(traits: K) -> BadgeTable<K> {
        BadgeTable {
            index: IndexedMap::new(traits, 256),
            logical_w: Vec::new(),
            slot: Vec::new(),
            built: 0,
        }
    }

    fn insert(&mut self, key: K::Key, logical_w: f64) -> usize {
        let pos = self.index.insert(key) as usize;
        if pos >= self.logical_w.len() {
            self.logical_w.resize(pos + 1, 0.0);
            self.slot.resize(pos + 1, NO_SLOT);
        }
        self.logical_w[pos] = logical_w;
        self.slot[pos] = NO_SLOT;
        pos
    }

    fn remove(&mut self, pos: usize) {
        if self.slot[pos] != NO_SLOT {
            badge_atlas().release(self.slot[pos] as u32);
            self.built -= 1;
        }
        let from = self.index.remove_at(pos as u32) as usize;
        if from != pos {
            self.logical_w[pos] = self.logical_w[from];
            self.slot[pos] = self.slot[from];
        }
        self.logical_w.truncate(from);
        self.slot.truncate(from);
    }
}

struct Badges {
    places: BadgeTable<I64Keys>,
    clusters: BadgeTable<U32Keys>,
    advance: Option<Advances>,
    stats: PoiBadgeBuildStats,
}

static BADGES: Global<Badges> = Global::new();

fn badges() -> &'static mut Badges {
    BADGES.get(|| Badges {
        places: BadgeTable::new(I64Keys),
        clusters: BadgeTable::new(U32Keys),
        advance: None,
        stats: PoiBadgeBuildStats::default(),
    })
}

/// Truncated by character count rather than a word wrap — a map badge reads as
/// one short line, not a multi-line chat message.
fn display_name(page: &PoiPage, rec: u32) -> String {
    let name = page.name_of(rec).unwrap_or_default();
    let units: Vec<u16> = name.encode_utf16().collect();
    if units.len() > NAME_MAX_CHARS {
        format!("{}…", String::from_utf16_lossy(&units[..NAME_MAX_CHARS - 1]))
    } else {
        name
    }
}

/// Cache misses and what they cost, cumulative. `metrics` is a text
/// measurement — asked only for what is drawn, since a candidate's width is
/// estimated; `builds` is a sheet entry and the upload.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoiBadgeBuildStats {
    pub metrics: u32,
    pub builds: u32,
    pub ms: f64,
}

pub fn poi_badge_build_stats() -> PoiBadgeBuildStats {
    badges().stats
}

/// Text width from the characters, without measuring the text.
///
/// Every candidate a placement offers to the screen needs a width for the
/// collision test, and a placement over a new area offers a couple of
/// thousand. `measureText` on a phone is around a millisecond for a Korean
/// name — shaping — so measuring every candidate was a third of a second of
/// main-thread time per new area (2026-09-13). An estimate from the character
/// classes is within a glyph or two of the truth, which the collision test does
/// not notice, and costs nothing. The font's own advances are measured once per
/// class rather than assumed, so a font change does not drift it. What is drawn
/// is measured exactly, at build, and the cache is corrected to that.
#[derive(Clone, Copy)]
struct Advances {
    cjk: f64,
    lower: f64,
    upper: f64,
    digit: f64,
    space: f64,
}

fn advances() -> Advances {
    let b = badges();
    *b.advance.get_or_insert_with(|| {
        let f = font();
        Advances {
            cjk: measure_text(&f, "가나다라마바사아자차") / 10.0,
            lower: measure_text(&f, "abcdefghijklmnopqrstuvwxyz") / 26.0,
            upper: measure_text(&f, "ABCDEFGHIJKLMNOPQRSTUVWXYZ") / 26.0,
            digit: measure_text(&f, "0123456789") / 10.0,
            space: measure_text(&f, " "),
        }
    })
}

fn estimate_text_w(text: &str) -> f64 {
    let a = advances();
    let mut w = 0.0;
    for ch in text.chars() {
        let c = ch as u32;
        w += if c == 0x20 {
            a.space
        } else if (0x30..=0x39).contains(&c) {
            a.digit
        } else if (0x41..=0x5a).contains(&c) {
            a.upper
        } else if c < 0x80 {
            a.lower
        } else if c >= 0x1100 {
            // Hangul, CJK ideographs, kana, fullwidth forms: one em each.
            a.cjk
        } else {
            a.upper
        };
    }
    w.ceil()
}

/// The badge's width in logical pixels: estimated on first ask, exact once the
/// badge has been drawn.
pub fn poi_badge_logical_w(page: &PoiPage, rec: u32) -> f64 {
    let key = page.id_hash.get(rec as usize) as i64;
    let places = &mut badges().places;
    let pos = places.index.find(key);
    if pos >= 0 {
        return places.logical_w[pos as usize];
    }
    let w = TEXT_X + estimate_text_w(&display_name(page, rec)) + GAP;
    let pos = badges().places.insert(key, w);
    badges().places.logical_w[pos]
}

/// A single-line pill (colored icon circle + name) with an anchor dot centered
/// below it, painted by the shell from the spec.
fn build_poi_badge(page: &PoiPage, rec: u32, logical_w: f64) -> i32 {
    let theme = resolve_poi_theme(page.category_of(rec));
    let name = display_name(page, rec);
    badge_atlas().add(
        logical_w * RENDER_SCALE,
        LOGICAL_H * RENDER_SCALE,
        DrawSpec::PoiBadge {
            icon: theme.icon,
            color: theme.color,
            name,
            logical_w,
        },
    )
}

/// The atlas slot of this POI's badge, drawn on first ask and kept after. Only
/// ask for one that is going to be drawn — poi_badge_logical_w answers
/// everything the selection pass needs without it. NO_SLOT when the sheet is
/// full, which the caller shows as nothing rather than as a wrong rectangle.
pub fn poi_badge_slot(page: &PoiPage, rec: u32) -> i32 {
    poi_badge_logical_w(page, rec);
    let key = page.id_hash.get(rec as usize) as i64;
    let pos = badges().places.index.find(key) as usize;
    if badges().places.slot[pos] != NO_SLOT {
        return badges().places.slot[pos];
    }

    let t0 = now();
    // Exact, now that it is drawn: the pill has to fit the text, and the
    // sprite has to be the pill's size.
    let logical_w = TEXT_X + measure_text(&font(), &display_name(page, rec)).ceil() + GAP;
    badges().places.logical_w[pos] = logical_w;
    badges().stats.metrics += 1;
    let slot = build_poi_badge(page, rec, logical_w);
    let b = badges();
    b.stats.builds += 1;
    b.stats.ms += now() - t0;

    // A full sheet gives back nothing rather than a wrong rectangle, and the
    // miss is not cached — the space it wanted may be free by the next ask.
    // Silently drawing nothing is here on purpose, because the right answer is
    // a policy this cache does not have yet: evict the least recently used badge
    // and take its rectangle.
    if slot == NO_SLOT {
        return NO_SLOT;
    }
    b.places.slot[pos] = slot;
    b.places.built += 1;
    slot
}

// ---- cluster badge ----
//
// A count where a name would be. It is the same pill a place gets, at the same
// height, with the same dot under it — one design for the layer. There is no
// icon: a cluster has no category, and the number is the information.
//
// Keyed by the count rather than by an id, which is the whole reason this is
// cheap. Every cluster of twelve looks alike, so the cache holds a few dozen
// entries for a whole session no matter how much world is crossed — and a
// cluster has no stable identity to key on anyway.

/// Horizontal breathing room either side of the count. Chosen so a single
/// digit fills exactly a circle of the pill's height, and anything longer
/// stretches it into a pill.
const CLUSTER_PAD_X: f64 = 10.0;

/// Capped, because four digits does not fit and the difference between 1200
/// and 1400 is not a difference anyone acts on.
fn cluster_label(count: u32) -> String {
    if count > 999 {
        "999+".to_string()
    } else {
        count.to_string()
    }
}

pub fn cluster_badge_logical_w(count: u32) -> f64 {
    let clusters = &mut badges().clusters;
    let pos = clusters.index.find(count);
    if pos >= 0 {
        return clusters.logical_w[pos as usize];
    }
    let text_w = measure_text(&font(), &cluster_label(count)).ceil();
    let pos = badges().clusters.insert(count, PILL_H.max(text_w + CLUSTER_PAD_X * 2.0));
    badges().clusters.logical_w[pos]
}

pub fn cluster_badge_slot(count: u32) -> i32 {
    let logical_w = cluster_badge_logical_w(count);
    let pos = badges().clusters.index.find(count) as usize;
    if badges().clusters.slot[pos] != NO_SLOT {
        return badges().clusters.slot[pos];
    }

    let t0 = now();
    let slot = badge_atlas().add(
        logical_w * RENDER_SCALE,
        LOGICAL_H * RENDER_SCALE,
        DrawSpec::Cluster {
            label: cluster_label(count),
            logical_w,
        },
    );
    let b = badges();
    b.stats.builds += 1;
    b.stats.ms += now() - t0;
    if slot == NO_SLOT {
        return NO_SLOT;
    }
    b.clusters.slot[pos] = slot;
    b.clusters.built += 1;
    slot
}

/// Drops every badge whose POI is no longer reachable. What this reclaims is
/// the rectangle, and what it prevents is a session-long table that grows with
/// every page ever visited.
pub fn evict_poi_badges(live: &IndexedMap<I64Keys>) -> u32 {
    let places = &mut badges().places;
    let mut dropped = 0;
    // Backwards, since a removal moves the last position into the hole. The
    // measured width goes with it: a stale size is a wrong collision box.
    for pos in (0..places.index.size() as usize).rev() {
        if live.find(places.index.key_at(pos as u32)) >= 0 {
            continue;
        }
        if places.slot[pos] != NO_SLOT {
            dropped += 1;
        }
        places.remove(pos);
    }
    // The only moment the sheet's live set shrinks, so the moment to see
    // whether it still needs to be as large as the densest area made it.
    if dropped > 0 {
        badge_atlas().compact();
    }
    dropped
}

pub fn poi_badge_count() -> u32 {
    badges().places.built
}
