// pxl_shared/record/placement.rs

//! The Placement: one record for one placed thing, whatever it is.
//!
//! A tree, a placed cube, a car, a bare ground cell — every thing drawn at
//! a place is a row of this table, and what differs between them is the
//! palette entry `value` names, not the row
//! (issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).
//!
//! Positions are page-local world units, as the POI page's are; `heading`
//! is radians about the up axis, counter-clockwise from +x, in the ENU
//! frame every model is converted into at the glTF boundary. `id` is the
//! FNV-1a of the thing's address, or of its external id where it has one.
//! `scale`, `rotation` and `tags` are ancillary: a reader that does not
//! know them skips them, and a record need not carry them.
//!
//! The row's numbers are the TypeScript's: doubles until they are written
//! into their f32 lanes, and a read row gives back what the lanes hold.

use xpute_core::io::marshal::tree::BranchCursor;
use xpute_core::kit::array::{F32Array, U32Array, U64Array, U8Array};
use xpute_core::kit::json::{parse_str_map, stringify_str_map, StrMap};
use xpute_core::sys::error::MarshalError;

use super::table::{encode_table, refuse, str_at, Column, ColumnValue, RecordSource, Table};

pub const PLACEMENT_KIND: &str = "placement";
pub const PLACEMENT_VERSION: u32 = 1;

/// The columns a placement reader understands, required ones first.
pub const PLACEMENT_KEYS: [&str; 10] = ["id", "x", "y", "z", "size", "heading", "value", "scale", "rotation", "tags"];

#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// The footprint's side in world units — a cube 1, a sub-cube 1/4, a
    /// two-cell building 2. A power of two by construction.
    pub size: f64,
    pub heading: f64,
    pub value: u32,
    pub scale: Option<f64>,
    /// Full orientation as a quaternion (x, y, z, w), when a thing has one.
    pub rotation: Option<[f64; 4]>,
    pub tags: Option<StrMap>,
}

/// Encodes rows. An ancillary column is written only if some row has it;
/// a row without it gets the neutral value (scale 1, identity rotation, no
/// tags).
pub fn encode_placements(rows: &[Placement]) -> Result<U8Array, MarshalError> {
    let n = rows.len();
    let (id, x, y, z) = (U64Array::new(n), F32Array::new(n), F32Array::new(n), F32Array::new(n));
    let (size, heading, value) = (F32Array::new(n), F32Array::new(n), U32Array::new(n));
    let (mut any_scale, mut any_rotation, mut any_tags) = (false, false, false);
    for (i, r) in rows.iter().enumerate() {
        id.put(i, r.id);
        x.put(i, r.x as f32);
        y.put(i, r.y as f32);
        z.put(i, r.z as f32);
        size.put(i, r.size as f32);
        heading.put(i, r.heading as f32);
        value.put(i, r.value);
        any_scale |= r.scale.is_some();
        any_rotation |= r.rotation.is_some();
        any_tags |= r.tags.is_some();
    }
    let col = |key: &str, value: ColumnValue, required: bool| Column {
        key: key.to_string(),
        value,
        required,
        stride: None,
    };
    let mut columns: Vec<Column> = vec![
        col("id", ColumnValue::Array(id.view()), true),
        col("x", ColumnValue::Array(x.view()), true),
        col("y", ColumnValue::Array(y.view()), true),
        col("z", ColumnValue::Array(z.view()), true),
        col("size", ColumnValue::Array(size.view()), true),
        col("heading", ColumnValue::Array(heading.view()), true),
        col("value", ColumnValue::Array(value.view()), true),
    ];
    if any_scale {
        let scale = F32Array::new(n);
        for (i, r) in rows.iter().enumerate() {
            scale.put(i, r.scale.unwrap_or(1.0) as f32);
        }
        columns.push(col("scale", ColumnValue::Array(scale.view()), false));
    }
    if any_rotation {
        let rotation = F32Array::new(n * 4);
        for (i, r) in rows.iter().enumerate() {
            for (k, q) in r.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]).iter().enumerate() {
                rotation.put(i * 4 + k, *q as f32);
            }
        }
        columns.push(Column {
            stride: Some(4),
            ..col("rotation", ColumnValue::Array(rotation.view()), false)
        });
    }
    if any_tags {
        let tags = rows.iter().map(|r| r.tags.as_ref().map_or(String::new(), |t| stringify_str_map(t))).collect();
        columns.push(col("tags", ColumnValue::Strings(tags), false));
    }
    encode_table(PLACEMENT_KIND, PLACEMENT_VERSION, n as u32, &columns)
}

/// A placement record opened over its packet: the lanes are views, the
/// optional ones None when the record does not carry them.
#[derive(Clone, Debug)]
pub struct PlacementTable {
    pub count: u32,
    pub id: U64Array,
    pub x: F32Array,
    pub y: F32Array,
    pub z: F32Array,
    pub size: F32Array,
    pub heading: F32Array,
    pub value: U32Array,
    pub scale: Option<F32Array>,
    /// Four per row.
    pub rotation: Option<F32Array>,
    tags: Option<BranchCursor>,
}

impl PlacementTable {
    pub fn new(source: RecordSource) -> Result<PlacementTable, MarshalError> {
        let t = Table::new(source, &PLACEMENT_KEYS)?;
        if t.kind != PLACEMENT_KIND {
            return Err(refuse(format!("not a placement record: {}", t.kind)));
        }
        Ok(PlacementTable {
            count: t.count,
            id: t.lanes("id")?,
            x: t.lanes("x")?,
            y: t.lanes("y")?,
            z: t.lanes("z")?,
            size: t.lanes("size")?,
            heading: t.lanes("heading")?,
            value: t.lanes("value")?,
            scale: t.lanes_opt("scale")?,
            rotation: t.lanes_opt("rotation")?,
            tags: t.strings("tags")?,
        })
    }

    pub fn scale_of(&self, i: u32) -> f64 {
        self.scale.as_ref().map_or(1.0, |s| s[i as usize] as f64)
    }

    pub fn tags_of(&self, i: u32) -> Result<Option<StrMap>, MarshalError> {
        let Some(tags) = &self.tags else { return Ok(None) };
        let s = str_at(tags, i)?;
        if s.is_empty() {
            Ok(None)
        } else {
            parse_str_map(&s).map(Some)
        }
    }

    /// The row as the object form — a detail view, a test. One allocation
    /// per ask, never per record.
    pub fn row(&self, i: u32) -> Result<Placement, MarshalError> {
        let at = i as usize;
        Ok(Placement {
            id: self.id[at],
            x: self.x[at] as f64,
            y: self.y[at] as f64,
            z: self.z[at] as f64,
            size: self.size[at] as f64,
            heading: self.heading[at] as f64,
            value: self.value[at],
            scale: self.scale.as_ref().map(|s| s[at] as f64),
            rotation: self.rotation.as_ref().map(|r| [r[at * 4] as f64, r[at * 4 + 1] as f64, r[at * 4 + 2] as f64, r[at * 4 + 3] as f64]),
            tags: self.tags_of(i)?,
        })
    }
}

#[cfg(test)]
#[path = "placement.test.rs"]
mod test;
