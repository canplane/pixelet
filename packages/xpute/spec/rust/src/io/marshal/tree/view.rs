// @xpute/spec/io/marshal/tree/view.rs

use crate::io::marshal::tree::spec::{
    NIL_NODE, Node, NodeValue, ScalarType, ScalarValue, SequenceType, SequenceValue,
};
use crate::lib::kit::word::{fits_i32, fits_i64};
use crate::sys::errno;
use crate::sys::error::MarshalError;

fn val_to_node(val: NodeValue) -> Result<Node, MarshalError> {
    let node = match val {
        NodeValue::Null => Node::Nil,
        NodeValue::Number(v) => {
            if fits_i32(v as i64) && v.fract() == 0.0 {
                Node::Scalar(ScalarType::I32, Some(ScalarValue::I32(v as i32)))
            } else {
                Node::Scalar(ScalarType::F64, Some(ScalarValue::F64(v)))
            }
        }
        NodeValue::BigInt(v) => {
            if !fits_i64(v) {
                return Err(MarshalError::new(errno::EINVAL, "bigint does not fit i64"));
            }
            Node::Scalar(ScalarType::I64, Some(ScalarValue::I64(v as i64)))
        }
        NodeValue::Bool(v) => Node::Scalar(ScalarType::Bool, Some(ScalarValue::Bool(v))),
        NodeValue::Str(v) => Node::Sequence(SequenceType::Str, Some(SequenceValue::Str(v))),
        NodeValue::U8Array(v) => Node::Sequence(SequenceType::U8Array, Some(SequenceValue::U8Array(v))),
        NodeValue::I8Array(v) => Node::Sequence(SequenceType::I8Array, Some(SequenceValue::I8Array(v))),
        NodeValue::U16Array(v) => Node::Sequence(SequenceType::U16Array, Some(SequenceValue::U16Array(v))),
        NodeValue::I16Array(v) => Node::Sequence(SequenceType::I16Array, Some(SequenceValue::I16Array(v))),
        NodeValue::U32Array(v) => Node::Sequence(SequenceType::U32Array, Some(SequenceValue::U32Array(v))),
        NodeValue::I32Array(v) => Node::Sequence(SequenceType::I32Array, Some(SequenceValue::I32Array(v))),
        NodeValue::U64Array(v) => Node::Sequence(SequenceType::U64Array, Some(SequenceValue::U64Array(v))),
        NodeValue::I64Array(v) => Node::Sequence(SequenceType::I64Array, Some(SequenceValue::I64Array(v))),
        NodeValue::F32Array(v) => Node::Sequence(SequenceType::F32Array, Some(SequenceValue::F32Array(v))),
        NodeValue::F64Array(v) => Node::Sequence(SequenceType::F64Array, Some(SequenceValue::F64Array(v))),
        NodeValue::Array(vals) => {
            let mut out = Vec::with_capacity(vals.len());
            for value in vals {
                out.push(val_to_node(value)?);
            }
            Node::Branch(Some(out))
        }
    };
    Ok(node)
}

// ============ TreeView ============

#[derive(Clone, Debug)]
pub struct TreeView {
    pub node: Node,
}

impl Default for TreeView {
    fn default() -> Self {
        Self { node: NIL_NODE }
    }
}

impl TreeView {
    pub fn new() -> Self { Self::default() }

    fn set_node(&mut self, node: Node) -> &mut Self {
        self.node = node;
        self
    }

    pub fn set(&mut self, val: NodeValue) -> Result<&mut Self, MarshalError> {
        self.node = val_to_node(val)?;
        Ok(self)
    }

    pub fn nil(&mut self) -> &mut Self {
        self.set_node(NIL_NODE)
    }

    // ---- Scalar ----
    // Explicit scalar writers are caller-chosen constructors.
    // Typed optional leaf writers preserve node kind;
    // physical presence is decided during encode.

    pub fn u8(&mut self, val: Option<u8>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::U8, val.map(ScalarValue::U8)))
    }
    pub fn i8(&mut self, val: Option<i8>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::I8, val.map(ScalarValue::I8)))
    }
    pub fn u16(&mut self, val: Option<u16>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::U16, val.map(ScalarValue::U16)))
    }
    pub fn i16(&mut self, val: Option<i16>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::I16, val.map(ScalarValue::I16)))
    }
    pub fn u32(&mut self, val: Option<u32>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::U32, val.map(ScalarValue::U32)))
    }
    pub fn i32(&mut self, val: Option<i32>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::I32, val.map(ScalarValue::I32)))
    }
    pub fn u64(&mut self, val: Option<u64>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::U64, val.map(ScalarValue::U64)))
    }
    pub fn i64(&mut self, val: Option<i64>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::I64, val.map(ScalarValue::I64)))
    }
    pub fn f32(&mut self, val: Option<f32>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::F32, val.map(ScalarValue::F32)))
    }
    pub fn f64(&mut self, val: Option<f64>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::F64, val.map(ScalarValue::F64)))
    }
    pub fn bool(&mut self, val: Option<bool>) -> &mut Self {
        self.set_node(Node::Scalar(ScalarType::Bool, val.map(ScalarValue::Bool)))
    }

    // ---- Sequence ----

    pub fn u8_array(&mut self, val: Option<Vec<u8>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::U8Array, val.map(SequenceValue::U8Array)))
    }
    pub fn i8_array(&mut self, val: Option<Vec<i8>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::I8Array, val.map(SequenceValue::I8Array)))
    }
    pub fn u16_array(&mut self, val: Option<Vec<u16>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::U16Array, val.map(SequenceValue::U16Array)))
    }
    pub fn i16_array(&mut self, val: Option<Vec<i16>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::I16Array, val.map(SequenceValue::I16Array)))
    }
    pub fn u32_array(&mut self, val: Option<Vec<u32>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::U32Array, val.map(SequenceValue::U32Array)))
    }
    pub fn i32_array(&mut self, val: Option<Vec<i32>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::I32Array, val.map(SequenceValue::I32Array)))
    }
    pub fn u64_array(&mut self, val: Option<Vec<u64>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::U64Array, val.map(SequenceValue::U64Array)))
    }
    pub fn i64_array(&mut self, val: Option<Vec<i64>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::I64Array, val.map(SequenceValue::I64Array)))
    }
    pub fn f32_array(&mut self, val: Option<Vec<f32>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::F32Array, val.map(SequenceValue::F32Array)))
    }
    pub fn f64_array(&mut self, val: Option<Vec<f64>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::F64Array, val.map(SequenceValue::F64Array)))
    }
    pub fn bitset(&mut self, val: Option<Vec<u8>>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::Bitset, val.map(SequenceValue::Bitset)))
    }
    pub fn str(&mut self, val: Option<String>) -> &mut Self {
        self.set_node(Node::Sequence(SequenceType::Str, val.map(SequenceValue::Str)))
    }

    // ---- Subtree ----

    /// Builds a branch subtree inline and writes it into the current root node.
    pub fn branch<F>(&mut self, f: Option<F>) -> &mut Self
    where
        F: FnOnce(&mut BranchView),
    {
        if let Some(f) = f {
            let mut branch = BranchView::new();
            f(&mut branch);
            self.node = branch.node;
        } else {
            // typed optional branch; node kind is preserved, physical presence decided during encode
            self.node = Node::Branch(None);
        }
        self
    }

    /// Grafts an already-encoded subtree packet into the current root node.
    ///
    /// Contract:
    /// - `pkt` must be a valid tree packet for this format.
    /// - The grafted root may be physically null.
    /// - The caller must not mutate `pkt` after grafting.
    ///
    /// Note (vs TS):
    /// - TS checks `pkt.byteOffset % WORD_SZ` (the Uint8Array's offset within its ArrayBuffer).
    /// - A Rust `Vec<u8>` always has byteOffset=0 by definition, so this check does not apply.
    /// - Full packet validation is deferred to encode/decode time.
    pub fn graft(&mut self, pkt: Vec<u8>) -> &mut Self {
        self.node = Node::Graft(pkt);
        self
    }
}

// ============ BranchView ============

#[derive(Clone, Debug)]
pub struct BranchView {
    pub node: Node,
}

impl Default for BranchView {
    fn default() -> Self {
        Self { node: Node::Branch(Some(Vec::new())) }
    }
}

impl BranchView {
    pub fn new() -> Self { Self::default() }

    fn children_mut(&mut self) -> &mut Vec<Node> {
        match &mut self.node {
            Node::Branch(Some(children)) => children,
            Node::Branch(None) => panic!("typed-null branch has no mutable children"),
            _ => panic!("branch view invariant violated"),
        }
    }

    fn put_node(&mut self, node: Node) -> &mut Self {
        self.children_mut().push(node);
        self
    }

    /// Replaces all existing children with the provided sequence.
    pub fn set(&mut self, vals: Vec<NodeValue>) -> Result<&mut Self, MarshalError> {
        self.children_mut().clear();
        for val in vals {
            self.put(val)?;
        }
        Ok(self)
    }

    /// Lowers one generic value and appends it as a child.
    pub fn put(&mut self, val: NodeValue) -> Result<&mut Self, MarshalError> {
        self.children_mut().push(val_to_node(val)?);
        Ok(self)
    }

    /// Generic/untyped null lowering.
    pub fn nil(&mut self) -> &mut Self {
        self.put_node(NIL_NODE)
    }

    // ---- Scalar ----
    // Explicit scalar writers are caller-chosen constructors.
    // Typed optional leaf writers preserve node kind;
    // physical presence is decided during encode.

    pub fn u8(&mut self, val: Option<u8>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::U8, val.map(ScalarValue::U8)))
    }
    pub fn i8(&mut self, val: Option<i8>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::I8, val.map(ScalarValue::I8)))
    }
    pub fn u16(&mut self, val: Option<u16>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::U16, val.map(ScalarValue::U16)))
    }
    pub fn i16(&mut self, val: Option<i16>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::I16, val.map(ScalarValue::I16)))
    }
    pub fn u32(&mut self, val: Option<u32>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::U32, val.map(ScalarValue::U32)))
    }
    pub fn i32(&mut self, val: Option<i32>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::I32, val.map(ScalarValue::I32)))
    }
    pub fn u64(&mut self, val: Option<u64>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::U64, val.map(ScalarValue::U64)))
    }
    pub fn i64(&mut self, val: Option<i64>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::I64, val.map(ScalarValue::I64)))
    }
    pub fn f32(&mut self, val: Option<f32>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::F32, val.map(ScalarValue::F32)))
    }
    pub fn f64(&mut self, val: Option<f64>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::F64, val.map(ScalarValue::F64)))
    }
    pub fn bool(&mut self, val: Option<bool>) -> &mut Self {
        self.put_node(Node::Scalar(ScalarType::Bool, val.map(ScalarValue::Bool)))
    }

    // ---- Sequence ----

    pub fn u8_array(&mut self, val: Option<Vec<u8>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::U8Array, val.map(SequenceValue::U8Array)))
    }
    pub fn i8_array(&mut self, val: Option<Vec<i8>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::I8Array, val.map(SequenceValue::I8Array)))
    }
    pub fn u16_array(&mut self, val: Option<Vec<u16>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::U16Array, val.map(SequenceValue::U16Array)))
    }
    pub fn i16_array(&mut self, val: Option<Vec<i16>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::I16Array, val.map(SequenceValue::I16Array)))
    }
    pub fn u32_array(&mut self, val: Option<Vec<u32>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::U32Array, val.map(SequenceValue::U32Array)))
    }
    pub fn i32_array(&mut self, val: Option<Vec<i32>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::I32Array, val.map(SequenceValue::I32Array)))
    }
    pub fn u64_array(&mut self, val: Option<Vec<u64>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::U64Array, val.map(SequenceValue::U64Array)))
    }
    pub fn i64_array(&mut self, val: Option<Vec<i64>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::I64Array, val.map(SequenceValue::I64Array)))
    }
    pub fn f32_array(&mut self, val: Option<Vec<f32>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::F32Array, val.map(SequenceValue::F32Array)))
    }
    pub fn f64_array(&mut self, val: Option<Vec<f64>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::F64Array, val.map(SequenceValue::F64Array)))
    }
    pub fn bitset(&mut self, val: Option<Vec<u8>>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::Bitset, val.map(SequenceValue::Bitset)))
    }
    pub fn str(&mut self, val: Option<String>) -> &mut Self {
        self.put_node(Node::Sequence(SequenceType::Str, val.map(SequenceValue::Str)))
    }

    // ---- Subtree ----

    /// Appends a branch subtree inline.
    pub fn branch<F>(&mut self, f: Option<F>) -> &mut Self
    where
        F: FnOnce(&mut BranchView),
    {
        if let Some(f) = f {
            let mut branch = BranchView::new();
            f(&mut branch);
            self.children_mut().push(branch.node);
        } else {
            // typed optional branch; node kind is preserved, physical presence decided during encode
            self.children_mut().push(Node::Branch(None));
        }
        self
    }

    /// Appends an in-memory subtree as one child.
    pub fn subtree(&mut self, subtree: &TreeView) -> &mut Self {
        self.children_mut().push(subtree.node.clone());
        self
    }

    /// Grafts an already-encoded subtree packet as one child.
    ///
    /// Note (vs TS):
    /// - TS checks `pkt.byteOffset % WORD_SZ` (the Uint8Array's offset within its ArrayBuffer).
    /// - A Rust `Vec<u8>` always has byteOffset=0 by definition, so this check does not apply.
    /// - Full packet validation is deferred to encode/decode time.
    pub fn graft(&mut self, pkt: Vec<u8>) -> &mut Self {
        self.children_mut().push(Node::Graft(pkt));
        self
    }
}
