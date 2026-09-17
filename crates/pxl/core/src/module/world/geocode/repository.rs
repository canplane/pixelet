// pxl_core/module/world/geocode/repository.rs

//! Reverse-geocode transport's kernel half — the pre-baked static set when
//! one is configured, the live API otherwise. A missing object just means
//! "outside Overture's Divisions coverage for this page", not a genuine
//! failure, so a 404 resolves straight to none. Reverse geocoding is
//! page-keyed like everything else this app bakes, not per exact coordinate.
//!
//! A response is read into a table — parts and their names — cleared before
//! the next.

use core::ops::Range;
use pxl_shared::spec::space::pvf3::ivec::IVec;

use xpute_core::kit::json::{read_json, JsonKind, JsonValue};

use crate::lib::env::config::GEOCODE_TILE_BASE_URL;
use crate::lib::env::route::{Joined, TileBase};

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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GeocodePart {
    pub name: String,
    pub common: Rows,
}

/// A translated name, by its language tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommonName {
    pub lang: String,
    pub name: String,
}

/// Parts and the names they hold.
pub struct PartTable {
    names: Vec<CommonName>,
    parts: Vec<GeocodePart>,
}

impl PartTable {
    pub fn new(parts: usize, names: usize) -> PartTable {
        PartTable {
            names: Vec::with_capacity(names),
            parts: Vec::with_capacity(parts),
        }
    }

    pub fn clear(&mut self) {
        self.names.clear();
        self.parts.clear();
    }

    pub fn parts(&self, rows: Rows) -> &[GeocodePart] {
        &self.parts[rows.range()]
    }

    // Overture's own names.primary is always the local-language (Korean) form —
    // for a `lang` UI can't find in `common`, falling back to `name` means "show
    // the Korean name" rather than an empty part.
    /// The name among `common` for `lang`, else `primary`.
    pub fn name_in<'a>(&'a self, common: Rows, primary: &'a str, lang: &str) -> &'a str {
        self.names[common.range()].iter().find(|c| c.lang == lang).map_or(primary, |c| c.name.as_str())
    }

    pub fn name_of<'a>(&'a self, part: &'a GeocodePart, lang: &str) -> &'a str {
        self.name_in(part.common, &part.name, lang)
    }

    /// A `names.common` object as rows.
    pub fn read_common(&mut self, value: Option<JsonValue>) -> Rows {
        let first = self.names.len() as u32;
        if let Some(object) = value.filter(|v| v.kind() == JsonKind::Object) {
            for (key, v) in object.fields() {
                let Some(s) = v.as_str() else { continue };
                self.names.push(CommonName {
                    lang: key.to_string(),
                    name: s.to_string(),
                });
            }
        }
        Rows {
            first,
            count: self.names.len() as u32 - first,
        }
    }

    /// A part as the JSON carries it; none where it does not read.
    pub fn read_part(&mut self, value: JsonValue) -> Option<GeocodePart> {
        let name = value.get("name")?.as_str()?.to_string();
        let common = self.read_common(value.get("common"));
        let part = GeocodePart { name, common };
        self.parts.push(part.clone());
        Some(part)
    }

    /// An array of parts as rows, in order; the ones that do not read left out.
    pub fn read_parts(&mut self, value: Option<JsonValue>) -> Rows {
        let first = self.parts.len() as u32;
        for p in value.map(|v| v.items()).into_iter().flatten() {
            self.read_part(p);
        }
        Rows {
            first,
            count: self.parts.len() as u32 - first,
        }
    }
}

pub fn geocode_url(page_idx: IVec) -> Joined<TileBase, IVec> {
    Joined(GEOCODE_TILE_BASE_URL(), page_idx)
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
