// pxl_shared/record/table.rs

//! The column layer: a record is a table of named columns on the tree
//! packet, and every record kind this project keeps is one of these.
//!
//!   [kind str, version u32, keys [str...], required u8[], count u32, column...]
//!
//! `keys` names the columns in order; `required` has a byte per column, 1
//! where a reader that does not know the column must refuse the record
//! rather than skip it (glTF's `extensionsRequired`, PNG's critical bit).
//! `count` is the row count every column has. A column is a typed-array
//! leaf, or a branch of `str` leaves read one row at a time.
//!
//! What this adds to the packet is self-description: a reader finds a
//! column by name, a writer can add one without the readers that do not
//! care noticing, and a record says what it is. What it does not add is a
//! schema system — a record kind's module says which keys mean what and
//! what type each is
//! (issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).

use std::collections::HashMap;

use xpute_core::io::marshal::tree::{encoder, BranchCursor, BranchView, BranchViewImpl, Cursor, NodeValue, NodeView, TreeEncoderOptions, TreeReader, TreeView};
use xpute_core::kit::array::{AnyElement, ArrayBufferView, TypedArray, U8Array};
use xpute_core::sys::errno::Errno;
use xpute_core::sys::error::MarshalError;

#[derive(Clone, Debug)]
pub enum ColumnValue {
    /// A typed array of either family.
    Array(ArrayBufferView),
    Strings(Vec<String>),
}

impl ColumnValue {
    /// `value.length`: elements, or strings.
    fn length(&self) -> usize {
        match self {
            ColumnValue::Array(a) => a.length(),
            ColumnValue::Strings(s) => s.len(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Column {
    pub key: String,
    pub value: ColumnValue,
    /// A reader that does not know this column must refuse the record.
    pub required: bool,
    /// Components per row, for a column that packs a vector per row (an
    /// extent is three, a quaternion four). The kind's module knows it; the
    /// record does not say it.
    pub stride: Option<u32>,
}

fn put_column(b: &mut BranchViewImpl, value: &ColumnValue) -> Result<(), MarshalError> {
    match value {
        ColumnValue::Strings(strings) => {
            b.branch(Some(&mut |s| {
                for v in strings {
                    s.str(Some(v));
                }
            }));
        }
        // The generic lowering maps each typed array to its own leaf kind.
        ColumnValue::Array(a) => {
            b.put(NodeValue::Array(a.clone()))?;
        }
    }
    Ok(())
}

/// Encodes one record. Every column must have `count` rows.
pub fn encode_table(kind: &str, version: u32, count: u32, columns: &[Column]) -> Result<U8Array, MarshalError> {
    for c in columns {
        let rows = c.value.length() as f64 / c.stride.unwrap_or(1) as f64;
        if rows != count as f64 {
            panic!("record {kind}: column {} has {rows} rows, not {count}", c.key);
        }
    }
    let required = U8Array::new(columns.len());
    for (i, c) in columns.iter().enumerate() {
        required.put(i, if c.required { 1 } else { 0 });
    }
    let mut t = TreeView::new();
    let mut failed: Option<MarshalError> = None;
    t.branch(Some(&mut |b| {
        b.str(Some(kind)).u32(Some(version));
        b.branch(Some(&mut |k| {
            for c in columns {
                k.str(Some(&c.key));
            }
        }));
        b.u8_array(Some(&required)).u32(Some(count));
        for c in columns {
            if let Err(e) = put_column(b, &c.value) {
                failed.get_or_insert(e);
            }
        }
    }));
    if let Some(e) = failed {
        return Err(e);
    }
    encoder().encode(
        &t,
        TreeEncoderOptions {
            init_cap: Some(1 << 12),
            ..Default::default()
        },
    )
}

const HEADER: u32 = 5;

/// What a record can be opened from: its own packet, or a branch inside a
/// larger packet a container grafted it into.
#[derive(Clone, Debug)]
pub enum RecordSource {
    Packet(U8Array),
    Branch(BranchCursor),
}

pub fn record_root(source: RecordSource) -> Result<BranchCursor, MarshalError> {
    match source {
        RecordSource::Packet(pkt) => TreeReader::new(pkt)?.read_branch(),
        RecordSource::Branch(b) => Ok(b),
    }
}

/// A refusal of a record's content: an Error there, EBADMSG here.
pub(crate) fn refuse(message: String) -> MarshalError {
    MarshalError::new(Errno::EBADMSG, Some(&message), None)
}

/// A record opened over its packet, or over the branch a container holds
/// it in. Columns are cursors into the packet, read on demand; a typed
/// column is a view, not a copy.
///
/// `known` is what the reader understands. A required column outside it is
/// refused here, once, rather than misread later.
#[derive(Clone, Debug)]
pub struct Table {
    pub kind: String,
    pub version: u32,
    pub count: u32,
    pub keys: Vec<String>,
    root: BranchCursor,
    index: HashMap<String, u32>,
}

impl Table {
    pub fn new(source: RecordSource, known: &[&str]) -> Result<Table, MarshalError> {
        let root = record_root(source)?;
        let kind = root.at(0)?.get_str()?;
        let version = root.at(1)?.get_number()? as u32;
        let keys: Vec<String> = root.at(2)?.as_branch()?.iter().map(|c| c?.get_str()).collect::<Result<_, _>>()?;
        let required = root.at(3)?.get_array::<u8>()?;
        let count = root.at(4)?.get_number()? as u32;
        let mut index = HashMap::new();
        for (i, key) in keys.iter().enumerate() {
            // A key given twice is found at its last column, as the Map is.
            index.insert(key.clone(), i as u32);
        }
        for (i, key) in keys.iter().enumerate() {
            let flag = if i < required.length() { required[i] } else { 0 };
            if flag != 0 && !known.contains(&key.as_str()) {
                return Err(refuse(format!("record {kind} v{version}: required column {key} is not understood")));
            }
        }
        Ok(Table {
            kind,
            version,
            count,
            keys,
            root,
            index,
        })
    }

    pub fn has(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    /// The column's cursor, or None when the record does not carry it.
    pub fn column(&self, key: &str) -> Result<Option<Cursor>, MarshalError> {
        match self.index.get(key) {
            None => Ok(None),
            Some(i) => Ok(Some(self.root.at(HEADER + i)?)),
        }
    }

    /// A typed-array column as a view into the packet. Refused when absent:
    /// the caller decided it is required.
    pub fn lanes<T: AnyElement>(&self, key: &str) -> Result<TypedArray<T>, MarshalError> {
        match self.column(key)? {
            None => Err(refuse(format!("record {}: no column {key}", self.kind))),
            Some(c) => c.get_array(),
        }
    }

    /// A typed-array column, or None when absent.
    pub fn lanes_opt<T: AnyElement>(&self, key: &str) -> Result<Option<TypedArray<T>>, MarshalError> {
        self.column(key)?.map(|c| c.get_array()).transpose()
    }

    /// A string column, read one row at a time through the branch.
    pub fn strings(&self, key: &str) -> Result<Option<BranchCursor>, MarshalError> {
        self.column(key)?.map(|c| c.as_branch()).transpose()
    }
}

/// One row's string, from a column `strings()` returned.
pub fn str_at(column: &BranchCursor, row: u32) -> Result<String, MarshalError> {
    column.at(row)?.get_str()
}

#[cfg(test)]
#[path = "table.test.rs"]
mod test;
