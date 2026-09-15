// pxl_core/module/world/geocode/label.rs
//
// Shared with search: a GeocodePart chain needs the same "pick a language,
// then order broad-to-narrow or narrow-to-broad by convention" treatment
// whether it came from reverse geocoding a page or from a search result's own
// `context`, so both share one answer instead of two copies drifting apart.

use core::fmt;


use super::repository::{PartTable, Rows};

// Broad-to-narrow ("강남구 역삼1동") is a Korean/Japanese/Chinese address
// convention, not a property of the place itself — the same pattern shows up
// in how these languages write dates (year first). Every other supported
// language reverses it ("Yeoksam 1-dong, Gangnam-gu"). Extend this list, not
// the server, if a future UI language needs the same convention.
const BROAD_TO_NARROW_LANGS: [&str; 3] = ["ko", "ja", "zh"];

/// A language tag's primary subtag, lowercased: "ko-KR" reads as "ko". The
/// tags are ASCII (BCP 47), and none the UI has is longer than a few letters.
pub fn primary_lang(language: &str) -> String {
    let mut lang = String::new();
    for c in language.split('-').next().unwrap_or("").chars() {
        lang.push(c.to_ascii_lowercase());
    }
    lang
}

/// A broad-to-narrow GeocodePart chain as one display string against a
/// language (typically the UI language), written where it is displayed — the
/// join order and separator flip per BROAD_TO_NARROW_LANGS, the per-part name
/// resolves against the language regardless.
#[derive(Clone)]
pub struct Label<'a> {
    table: &'a PartTable,
    parts: Rows,
    lang: String,
}

pub fn assemble_label<'a>(table: &'a PartTable, parts: Rows, language: &str) -> Label<'a> {
    Label { table, parts, lang: primary_lang(language) }
}

impl fmt::Display for Label<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self.table.parts(self.parts).iter().map(|p| self.table.name_of(p, &self.lang));
        if BROAD_TO_NARROW_LANGS.contains(&self.lang.as_str()) {
            for (i, name) in names.enumerate() {
                if i > 0 {
                    f.write_str(" ")?;
                }
                f.write_str(name)?;
            }
        } else {
            for (i, name) in names.rev().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                f.write_str(name)?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Label<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", format_args!("{self}"))
    }
}
