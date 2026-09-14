// pxl_shared/record/palette.rs

//! The palette: one global record naming every kind of thing a placement
//! can be. A placement's `value` is a row here.
//!
//! A row for a cube is a colour; a row for a decor kind or an actor points
//! at its model by content — the glb's sha256 names the object in the
//! bucket, so the palette row is the directory entry and the model never
//! changes under it. A model whose texture is a separate image (the Kenney
//! packs share one colormap per pack) names that too, by the same rule;
//! a self-contained glb leaves it empty. `extent` is measured at ingest and
//! is what the range criterion reads; `license` and `author` are what
//! attribution shows. A user-uploaded kind is a row from USER_ID_BASE up
//! (issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).

use std::collections::HashMap;

use xpute_core::io::marshal::tree::BranchCursor;
use xpute_core::kit::array::{F32Array, U32Array, U64Array, U8Array};
use xpute_core::sys::error::MarshalError;

use super::table::{encode_table, refuse, str_at, Column, ColumnValue, RecordSource, Table};

pub const PALETTE_KIND: &str = "palette";
pub const PALETTE_VERSION: u32 = 2;

/// What a row is. The enum column stores its index in PALETTE_KINDS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteKind {
    Cube,
    Ground,
    Decor,
    Actor,
}

impl PaletteKind {
    /// The string the TypeScript's union member is.
    pub fn as_str(self) -> &'static str {
        match self {
            PaletteKind::Cube => "cube",
            PaletteKind::Ground => "ground",
            PaletteKind::Decor => "decor",
            PaletteKind::Actor => "actor",
        }
    }
}

pub const PALETTE_KINDS: [PaletteKind; 4] = [PaletteKind::Cube, PaletteKind::Ground, PaletteKind::Decor, PaletteKind::Actor];

/// Rows issued for user uploads start here; below is the shipped set.
pub const USER_ID_BASE: u32 = 1 << 20;

pub const PALETTE_KEYS: [&str; 12] = ["id", "kind", "name", "model_hash", "model_key", "skin_key", "extent", "size", "license", "author", "color", "group"];

/// The entry's numbers are the TypeScript's: doubles until they are
/// written into their f32 lanes.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteEntry {
    pub id: u32,
    pub kind: PaletteKind,
    pub name: String,
    /// FNV-1a of the model's content hash; 0 for a kind with no model.
    pub model_hash: u64,
    /// The bucket key, `asset/<sha256>`; empty for a kind with no model.
    pub model_key: String,
    /// The bucket key of the model's texture when it is a separate image;
    /// None for a self-contained glb or a kind with no model.
    pub skin_key: Option<String>,
    /// The model's bounds in world units (x, y, z), measured at ingest.
    pub extent: [f64; 3],
    /// The footprint's side in world units, a power of two.
    pub size: f64,
    pub license: String,
    pub author: String,
    /// A cube's colour, `#rrggbb`; None for a kind drawn from a model.
    pub color: Option<String>,
    /// The scatter group a model belongs to — "tree", "car" — so a kind's
    /// variants are found by name rather than by remembering ids.
    pub group: Option<String>,
}

pub fn encode_palette(rows: &[PaletteEntry]) -> Result<U8Array, MarshalError> {
    let n = rows.len();
    let (id, kind, model_hash) = (U32Array::new(n), U8Array::new(n), U64Array::new(n));
    let (extent, size) = (F32Array::new(n * 3), F32Array::new(n));
    for (i, r) in rows.iter().enumerate() {
        id.put(i, r.id);
        kind.put(i, PALETTE_KINDS.iter().position(|k| *k == r.kind).unwrap() as u8);
        model_hash.put(i, r.model_hash);
        for k in 0..3 {
            extent.put(i * 3 + k, r.extent[k] as f32);
        }
        size.put(i, r.size as f32);
    }
    let strings = |f: &dyn Fn(&PaletteEntry) -> String| ColumnValue::Strings(rows.iter().map(f).collect());
    let col = |key: &str, value: ColumnValue, required: bool| Column {
        key: key.to_string(),
        value,
        required,
        stride: None,
    };
    encode_table(
        PALETTE_KIND,
        PALETTE_VERSION,
        n as u32,
        &[
            col("id", ColumnValue::Array(id.view()), true),
            col("kind", ColumnValue::Array(kind.view()), true),
            col("name", strings(&|r| r.name.clone()), true),
            col("model_hash", ColumnValue::Array(model_hash.view()), true),
            col("model_key", strings(&|r| r.model_key.clone()), true),
            col("skin_key", strings(&|r| r.skin_key.clone().unwrap_or_default()), false),
            Column {
                stride: Some(3),
                ..col("extent", ColumnValue::Array(extent.view()), true)
            },
            col("size", ColumnValue::Array(size.view()), true),
            col("license", strings(&|r| r.license.clone()), false),
            col("author", strings(&|r| r.author.clone()), false),
            col("color", strings(&|r| r.color.clone().unwrap_or_default()), false),
            col("group", strings(&|r| r.group.clone().unwrap_or_default()), false),
        ],
    )
}

/// `Math.max` of three: NaN if any is, where `f32::max` would drop it.
fn js_max3(a: f32, b: f32, c: f32) -> f32 {
    if a.is_nan() || b.is_nan() || c.is_nan() {
        return f32::NAN;
    }
    a.max(b).max(c)
}

/// A palette opened over its packet, with a row index by id.
#[derive(Clone, Debug)]
pub struct PaletteTable {
    pub count: u32,
    pub id: U32Array,
    pub kind: U8Array,
    pub model_hash: U64Array,
    /// Three per row.
    pub extent: F32Array,
    pub size: F32Array,
    name: BranchCursor,
    model_key: BranchCursor,
    skin_key: Option<BranchCursor>,
    license: Option<BranchCursor>,
    author: Option<BranchCursor>,
    color: Option<BranchCursor>,
    group: Option<BranchCursor>,
    by_id: HashMap<u32, u32>,
}

impl PaletteTable {
    pub fn new(source: RecordSource) -> Result<PaletteTable, MarshalError> {
        let t = Table::new(source, &PALETTE_KEYS)?;
        if t.kind != PALETTE_KIND {
            return Err(refuse(format!("not a palette record: {}", t.kind)));
        }
        let id: U32Array = t.lanes("id")?;
        let mut by_id = HashMap::new();
        for i in 0..t.count {
            by_id.insert(id[i as usize], i);
        }
        let required_strings = |key: &str| t.strings(key)?.ok_or_else(|| refuse(format!("record {}: no column {key}", t.kind)));
        Ok(PaletteTable {
            count: t.count,
            kind: t.lanes("kind")?,
            model_hash: t.lanes("model_hash")?,
            extent: t.lanes("extent")?,
            size: t.lanes("size")?,
            name: required_strings("name")?,
            model_key: required_strings("model_key")?,
            skin_key: t.strings("skin_key")?,
            license: t.strings("license")?,
            author: t.strings("author")?,
            color: t.strings("color")?,
            group: t.strings("group")?,
            id,
            by_id,
        })
    }

    /// The rows of a scatter group, in declaration order — a kind's variants.
    pub fn rows_of_group(&self, group: &str) -> Result<Vec<u32>, MarshalError> {
        let mut out = Vec::new();
        let Some(groups) = &self.group else { return Ok(out) };
        for i in 0..self.count {
            if str_at(groups, i)? == group {
                out.push(i);
            }
        }
        Ok(out)
    }

    /// The row for a palette id, or -1.
    pub fn row_of(&self, id: u32) -> i64 {
        self.by_id.get(&id).map_or(-1, |i| *i as i64)
    }
    pub fn kind_of(&self, i: u32) -> Result<PaletteKind, MarshalError> {
        let k = self.kind[i as usize];
        PALETTE_KINDS.get(k as usize).copied().ok_or_else(|| refuse(format!("palette: row {i} has kind {k}")))
    }
    pub fn name_of(&self, i: u32) -> Result<String, MarshalError> {
        str_at(&self.name, i)
    }
    pub fn model_key_of(&self, i: u32) -> Result<String, MarshalError> {
        str_at(&self.model_key, i)
    }
    pub fn skin_key_of(&self, i: u32) -> Result<String, MarshalError> {
        self.skin_key.as_ref().map_or(Ok(String::new()), |c| str_at(c, i))
    }
    pub fn license_of(&self, i: u32) -> Result<String, MarshalError> {
        self.license.as_ref().map_or(Ok(String::new()), |c| str_at(c, i))
    }
    pub fn author_of(&self, i: u32) -> Result<String, MarshalError> {
        self.author.as_ref().map_or(Ok(String::new()), |c| str_at(c, i))
    }
    pub fn color_of(&self, i: u32) -> Result<String, MarshalError> {
        self.color.as_ref().map_or(Ok(String::new()), |c| str_at(c, i))
    }
    pub fn group_of(&self, i: u32) -> Result<String, MarshalError> {
        self.group.as_ref().map_or(Ok(String::new()), |c| str_at(c, i))
    }
    /// The largest dimension of the model — what the range criterion reads.
    pub fn extent_max(&self, i: u32) -> f32 {
        let at = i as usize * 3;
        js_max3(self.extent[at], self.extent[at + 1], self.extent[at + 2])
    }

    pub fn entry(&self, i: u32) -> Result<PaletteEntry, MarshalError> {
        let at = i as usize;
        let non_empty = |s: String| if s.is_empty() { None } else { Some(s) };
        Ok(PaletteEntry {
            id: self.id[at],
            kind: self.kind_of(i)?,
            name: self.name_of(i)?,
            model_hash: self.model_hash[at],
            model_key: self.model_key_of(i)?,
            skin_key: non_empty(self.skin_key_of(i)?),
            extent: [self.extent[at * 3] as f64, self.extent[at * 3 + 1] as f64, self.extent[at * 3 + 2] as f64],
            size: self.size[at] as f64,
            license: self.license_of(i)?,
            author: self.author_of(i)?,
            color: non_empty(self.color_of(i)?),
            group: non_empty(self.group_of(i)?),
        })
    }
}

#[cfg(test)]
#[path = "palette.test.rs"]
mod test;
