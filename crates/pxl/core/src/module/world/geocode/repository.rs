// pxl_core/module/world/geocode/repository.rs

//! Reverse-geocode transport's kernel half — the pre-baked static set when
//! one is configured, the live API otherwise. A missing object just means
//! "outside Overture's Divisions coverage for this page", not a genuine
//! failure, so a 404 resolves straight to none. Reverse geocoding is
//! page-keyed like everything else this app bakes, not per exact coordinate.
//!
//! A response is read where it lies into a table of lanes — parts, their
//! names and the text of both — cleared before the next, so an answer builds
//! nothing on a heap.

use core::ops::Range;

use xpute_core::kit::json::{read_json, JsonKind, JsonValue};
use xpute_core::kit::text::FixStr;

use crate::lib::env::config::GEOCODE_TILE_BASE_URL;
use crate::lib::env::route::{Joined, TileBase};
use crate::memory::{FixedVec, Span, TextLane};

/// A run of a table's rows: the first, and how many.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rows {
    pub first: u32,
    pub count: u32,
}

impl Rows {
    pub fn range(self) -> Range<usize> {
        self.first as usize..(self.first + self.count) as usize
    }
}

/// One administrative division, mirroring the API's own GeocodePart — its
/// name is Overture's local-language `names.primary`, and `common` the rows of
/// a language-tag-keyed subset of `names.common`, present only when Overture
/// had a translation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeocodePart {
    pub name: Span,
    pub common: Rows,
}

/// A translated name, by its language tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommonName {
    pub lang: FixStr<16>,
    pub name: Span,
}

/// Parts and the names and text they hold.
pub struct PartTable {
    text: TextLane,
    names: FixedVec<CommonName>,
    parts: FixedVec<GeocodePart>,
}

impl PartTable {
    pub fn new(parts: usize, names: usize, text_bytes: usize) -> PartTable {
        PartTable {
            text: TextLane::new(text_bytes),
            names: FixedVec::new(names),
            parts: FixedVec::new(parts),
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.names.clear();
        self.parts.clear();
    }

    pub fn text(&self, span: Span) -> &str {
        self.text.get(span)
    }

    /// Text appended to the table's lane.
    pub fn push_text(&mut self, v: impl core::fmt::Display) -> Option<Span> {
        self.text.push(v)
    }

    pub fn parts(&self, rows: Rows) -> &[GeocodePart] {
        &self.parts.as_slice()[rows.range()]
    }

    // Overture's own names.primary is always the local-language (Korean) form —
    // for a `lang` UI can't find in `common`, falling back to `name` means "show
    // the Korean name" rather than an empty part.
    /// The name among `common` for `lang`, else `primary`.
    pub fn name_in(&self, common: Rows, primary: Span, lang: &str) -> &str {
        self.names.as_slice()[common.range()].iter().find(|c| c.lang == lang).map_or(self.text(primary), |c| self.text(c.name))
    }

    pub fn name_of(&self, part: &GeocodePart, lang: &str) -> &str {
        self.name_in(part.common, part.name, lang)
    }

    /// A `names.common` object as rows. A tag too long to be one, or a name
    /// past what the table holds, is left out: the primary name stands in.
    pub fn read_common(&mut self, value: Option<JsonValue>) -> Rows {
        let first = self.names.len() as u32;
        if let Some(object) = value.filter(|v| v.kind() == JsonKind::Object) {
            for (key, v) in object.fields() {
                let mut lang = FixStr::new();
                let (Some(s), Ok(())) = (v.as_str(), core::fmt::Write::write_fmt(&mut lang, format_args!("{key}"))) else { continue };
                let Some(name) = self.text.push(s) else { break };
                if !self.names.push(CommonName { lang, name }) {
                    break;
                }
            }
        }
        Rows { first, count: self.names.len() as u32 - first }
    }

    /// A part as the JSON carries it; none where it does not read or the
    /// table is full.
    pub fn read_part(&mut self, value: JsonValue) -> Option<GeocodePart> {
        let name = self.text.push(value.get("name")?.as_str()?)?;
        let common = self.read_common(value.get("common"));
        let part = GeocodePart { name, common };
        self.parts.push(part).then_some(part)
    }

    /// An array of parts as rows, in order; the ones that do not read left out.
    pub fn read_parts(&mut self, value: Option<JsonValue>) -> Rows {
        let first = self.parts.len() as u32;
        for p in value.map(|v| v.items()).into_iter().flatten() {
            self.read_part(p);
        }
        Rows { first, count: self.parts.len() as u32 - first }
    }
}

pub fn geocode_url(key: &str) -> Joined<'_, TileBase> {
    Joined(GEOCODE_TILE_BASE_URL(), key)
}

/// A response body, as `res.json()` read it, into `table` cleared first: the
/// rows of its parts, broad-to-narrow — e.g. `[{name:"강남구"}, {name:"역삼1동"}]`,
/// unjoined and each part's language unresolved until label.rs assembles them
/// against the viewer's own UI language; none where it does not read.
pub fn parse_geocode(table: &mut PartTable, body: &str) -> Option<Rows> {
    table.clear();
    let parts = read_json(body)?.get("parts").filter(|p| p.kind() == JsonKind::Array)?;
    Some(table.read_parts(Some(parts)))
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod test;
