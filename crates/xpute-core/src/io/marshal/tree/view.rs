// xpute-core/io/marshal/tree/view.rs

//! The write end: a `TreeView` holds a root node, a `BranchView` a branch's
//! children, and both write through the same set of typed writers. The
//! TypeScript's abstract `NodeView` is the trait: one write primitive
//! (`_set_node` for a tree, `_put_node` for a branch), and every writer on
//! top of it.

#![allow(non_snake_case)]

use crate::kit::array::{AnyElement, ArrayBufferView, TypedArray, U8Array};
use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

use super::spec::{BranchNode, GraftNode, Node, NodeValue, Numeric, ScalarNode, ScalarType, SequenceNode, SequenceType, SequenceVal, NIL_NODE, WORD_SZ};

fn scalar(type_: ScalarType, val: Option<Numeric>) -> Node {
    Node::Scalar(ScalarNode { type_, val })
}
fn seq(type_: SequenceType, val: Option<SequenceVal>) -> Node {
    Node::Sequence(SequenceNode { type_, val })
}
fn array<T: AnyElement>(arr: Option<&TypedArray<T>>) -> Option<SequenceVal> {
    arr.map(|a| SequenceVal::Array(a.view()))
}

// The word casts the TypeScript normalises through are the types here: a
// `u8` is already a u8.
const U8_NODE: fn(Option<u8>) -> Node = |u| scalar(ScalarType::U8, u.map(|u| Numeric::Number(u as f64)));
const I8_NODE: fn(Option<i8>) -> Node = |i| scalar(ScalarType::I8, i.map(|i| Numeric::Number(i as f64)));
const U16_NODE: fn(Option<u16>) -> Node = |u| scalar(ScalarType::U16, u.map(|u| Numeric::Number(u as f64)));
const I16_NODE: fn(Option<i16>) -> Node = |i| scalar(ScalarType::I16, i.map(|i| Numeric::Number(i as f64)));
const U32_NODE: fn(Option<u32>) -> Node = |u| scalar(ScalarType::U32, u.map(|u| Numeric::Number(u as f64)));
const I32_NODE: fn(Option<i32>) -> Node = |i| scalar(ScalarType::I32, i.map(|i| Numeric::Number(i as f64)));
const U64_NODE: fn(Option<u64>) -> Node = |u| scalar(ScalarType::U64, u.map(|u| Numeric::BigInt(u as i128)));
const I64_NODE: fn(Option<i64>) -> Node = |i| scalar(ScalarType::I64, i.map(|i| Numeric::BigInt(i as i128)));
const F32_NODE: fn(Option<f32>) -> Node = |f| scalar(ScalarType::F32, f.map(|f| Numeric::Number(f as f64)));
const F64_NODE: fn(Option<f64>) -> Node = |f| scalar(ScalarType::F64, f.map(Numeric::Number));

const BOOL_NODE: fn(Option<bool>) -> Node = |b| scalar(ScalarType::BOOL, b.map(|b| Numeric::Number(if b { 1.0 } else { 0.0 })));

fn ARRAY_NODE<T: AnyElement>(type_: SequenceType, arr: Option<&TypedArray<T>>) -> Node {
    seq(type_, array(arr))
}
fn BITSET_NODE(arr: Option<&U8Array>) -> Node {
    seq(SequenceType::BITSET, array(arr))
}
fn STR_NODE(s: Option<&str>) -> Node {
    seq(SequenceType::STR, s.map(|s| SequenceVal::Str(s.to_string())))
}

/// The sequence type of a typed array of `kind`.
fn array_type(view: &ArrayBufferView) -> SequenceType {
    use crate::kit::array::Kind;
    match view.kind {
        Kind::U8 => SequenceType::U8_ARRAY,
        Kind::I8 => SequenceType::I8_ARRAY,
        Kind::U16 => SequenceType::U16_ARRAY,
        Kind::I16 => SequenceType::I16_ARRAY,
        Kind::U32 => SequenceType::U32_ARRAY,
        Kind::I32 => SequenceType::I32_ARRAY,
        Kind::F32 => SequenceType::F32_ARRAY,
        Kind::F64 => SequenceType::F64_ARRAY,
        Kind::U64 => SequenceType::U64_ARRAY,
        Kind::I64 => SequenceType::I64_ARRAY,
    }
}

fn fits_i32(v: f64) -> bool {
    v.is_finite() && v.trunc() == v && v >= i32::MIN as f64 && v <= i32::MAX as f64
}
fn fits_i64(v: i128) -> bool {
    v >= i64::MIN as i128 && v <= i64::MAX as i128
}

/// A view's node, as the value a `set` or `put` takes in place of the view.
pub type ViewValue = NodeValue<Node>;

fn val_to_node(val: ViewValue) -> Result<Node, MarshalError> {
    Ok(match val {
        NodeValue::Extra(node) => node,

        NodeValue::Null => NIL_NODE,

        NodeValue::Number(v) => {
            if fits_i32(v) {
                I32_NODE(Some(v as i32))
            } else {
                F64_NODE(Some(v))
            }
        }
        NodeValue::BigInt(v) => {
            if !fits_i64(v) {
                return Err(MarshalError::new(Errno::EINVAL, Some("bigint does not fit i64"), None));
            }
            I64_NODE(Some(v as i64))
        }
        NodeValue::Bool(b) => BOOL_NODE(Some(b)),
        NodeValue::Str(s) => STR_NODE(Some(&s)),

        NodeValue::Array(view) => seq(array_type(&view), Some(SequenceVal::Array(view))),

        NodeValue::List(vals) => {
            let mut branch = BranchViewImpl::new();
            for child in vals {
                branch.put(child)?;
            }
            branch.node.clone()
        }
    })
}

// ============ View ============

pub trait NodeView {
    fn node(&self) -> &Node;

    /// The write primitive: a tree replaces its root, a branch appends a child.
    fn write(&mut self, node: Node) -> &mut Self;

    fn nil(&mut self) -> &mut Self {
        // generic/untyped null lowering
        self.write(NIL_NODE)
    }

    // ---- Scalar ----
    // explicit scalar writers are caller-chosen constructors;
    // inputs are normalized through the corresponding word cast helpers.
    // typed optional leaf writers preserve node kind;
    // physical presence is decided during encode.

    fn u8(&mut self, u: Option<u8>) -> &mut Self {
        self.write(U8_NODE(u))
    }
    fn i8(&mut self, i: Option<i8>) -> &mut Self {
        self.write(I8_NODE(i))
    }
    fn u16(&mut self, u: Option<u16>) -> &mut Self {
        self.write(U16_NODE(u))
    }
    fn i16(&mut self, i: Option<i16>) -> &mut Self {
        self.write(I16_NODE(i))
    }
    fn u32(&mut self, u: Option<u32>) -> &mut Self {
        self.write(U32_NODE(u))
    }
    fn i32(&mut self, i: Option<i32>) -> &mut Self {
        self.write(I32_NODE(i))
    }
    fn u64(&mut self, u: Option<u64>) -> &mut Self {
        self.write(U64_NODE(u))
    }
    fn i64(&mut self, i: Option<i64>) -> &mut Self {
        self.write(I64_NODE(i))
    }
    fn f32(&mut self, f: Option<f32>) -> &mut Self {
        self.write(F32_NODE(f))
    }
    fn f64(&mut self, f: Option<f64>) -> &mut Self {
        self.write(F64_NODE(f))
    }

    fn bool(&mut self, b: Option<bool>) -> &mut Self {
        self.write(BOOL_NODE(b))
    }

    // ---- Sequence ----

    fn u8_array(&mut self, arr: Option<&TypedArray<u8>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::U8_ARRAY, arr))
    }
    fn i8_array(&mut self, arr: Option<&TypedArray<i8>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::I8_ARRAY, arr))
    }
    fn u16_array(&mut self, arr: Option<&TypedArray<u16>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::U16_ARRAY, arr))
    }
    fn i16_array(&mut self, arr: Option<&TypedArray<i16>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::I16_ARRAY, arr))
    }
    fn u32_array(&mut self, arr: Option<&TypedArray<u32>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::U32_ARRAY, arr))
    }
    fn i32_array(&mut self, arr: Option<&TypedArray<i32>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::I32_ARRAY, arr))
    }
    fn u64_array(&mut self, arr: Option<&TypedArray<u64>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::U64_ARRAY, arr))
    }
    fn i64_array(&mut self, arr: Option<&TypedArray<i64>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::I64_ARRAY, arr))
    }
    fn f32_array(&mut self, arr: Option<&TypedArray<f32>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::F32_ARRAY, arr))
    }
    fn f64_array(&mut self, arr: Option<&TypedArray<f64>>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::F64_ARRAY, arr))
    }

    fn bitset(&mut self, arr: Option<&U8Array>) -> &mut Self {
        self.write(BITSET_NODE(arr))
    }

    fn str(&mut self, s: Option<&str>) -> &mut Self {
        self.write(STR_NODE(s))
    }

    // ---- Subtree ----

    /// Builds a branch subtree inline and writes it into the current view.
    fn branch(&mut self, f: Option<&mut dyn FnMut(&mut BranchViewImpl)>) -> &mut Self {
        // typed optional branch; node kind is preserved and physical presence is decided during encode
        let Some(f) = f else { return self.write(Node::Branch(BranchNode { val: None })) };

        let mut branch = BranchViewImpl::new();
        f(&mut branch);
        let node = branch.node.clone();
        self.write(node)
    }

    /// Grafts an already-encoded subtree packet.
    ///
    /// Contract:
    /// - `pkt` must be a tree packet carrier intended for this format
    /// - `pkt.byte_offset` must be WORD_SZ-aligned
    /// - The grafted root may be physically null
    /// - The caller must not mutate `pkt` after grafting it
    ///
    /// This method enforces only construction-boundary checks.
    /// Packet header validation remains a read/encode-side concern.
    fn graft(&mut self, pkt: U8Array) -> Result<&mut Self, MarshalError> {
        if !pkt.byte_offset().is_multiple_of(WORD_SZ as usize) {
            return Err(MarshalError::new(
                Errno::EBADMSG,
                Some(&format!("bad graft packet: misaligned base offset {}", pkt.byte_offset())),
                None,
            ));
        }
        Ok(self.write(Node::Graft(GraftNode { val: pkt })))
    }
}

pub struct TreeView {
    pub node: Node,
}

impl Default for TreeView {
    fn default() -> Self {
        TreeView::new()
    }
}

impl TreeView {
    pub fn new() -> TreeView {
        TreeView { node: NIL_NODE }
    }

    // ---- Write Primitive ----

    /// Replaces the current root node.
    fn _set_node(&mut self, node: Node) -> &mut Self {
        self.node = node;
        self
    }

    /// Lowers a generic value into the current root node.
    ///
    /// Mapping:
    /// - Number  -> i32 if it fits_i32(), otherwise f64
    /// - BigInt  -> i64
    /// - Bool -> bool
    /// - Str  -> str
    /// - Array -> matching array node
    /// - List -> branch preserving element order
    ///
    /// Note:
    /// - NodeValue currently excludes plain objects.
    /// - key-bearing object semantics must be modeled by a higher-level schema/plugin.
    /// - The number -> i32/f64 split is a JS-side lowering policy, not a wire-level
    ///   requirement of the tree packet format itself.
    ///
    /// Contract:
    /// - Unsupported input is treated as programmer error and errs.
    /// - bigint is accepted only when it fits the i64 lowering path.
    /// - Generic scalar bigint lowering currently targets only i64.
    ///   Unsigned 64-bit scalar writes must use the explicit u64() writer.
    /// - Explicit writers such as u8(), i64(), f64() are considered caller-chosen
    ///   leaf constructors and therefore do not add generic lowering validation.
    pub fn set(&mut self, val: ViewValue) -> Result<&mut Self, MarshalError> {
        let node = val_to_node(val)?;
        Ok(self._set_node(node))
    }
}

impl NodeView for TreeView {
    fn node(&self) -> &Node {
        &self.node
    }

    fn write(&mut self, node: Node) -> &mut Self {
        self._set_node(node)
    }
}

/// The branch view: what `branch(|b| ...)` hands its closure.
pub trait BranchView: NodeView {
    /// Replaces all existing children with the provided sequence.
    fn set(&mut self, vals: Vec<ViewValue>) -> Result<&mut Self, MarshalError>;

    /// Lowers one generic value and appends it as a child.
    fn put(&mut self, val: ViewValue) -> Result<&mut Self, MarshalError>;

    /// Appends an in-memory subtree as one child.
    /// The subtree is encoded inline as part of the current tree.
    fn subtree<V: NodeView>(&mut self, subtree: &V) -> &mut Self;
}

pub struct BranchViewImpl {
    pub node: Node,
}

impl Default for BranchViewImpl {
    fn default() -> Self {
        BranchViewImpl::new()
    }
}

impl BranchViewImpl {
    pub fn new() -> BranchViewImpl {
        BranchViewImpl {
            node: Node::Branch(BranchNode { val: Some(Vec::new()) }),
        }
    }

    fn children(&mut self) -> &mut Vec<Node> {
        match &mut self.node {
            Node::Branch(BranchNode { val: Some(children) }) => children,
            _ => unreachable!("a branch view holds a branch with children"),
        }
    }

    // ---- Append Primitive ----

    /// Appends one child node to the current branch.
    fn _put_node(&mut self, node: Node) -> &mut Self {
        self.children().push(node);
        self
    }
}

impl NodeView for BranchViewImpl {
    fn node(&self) -> &Node {
        &self.node
    }

    fn write(&mut self, node: Node) -> &mut Self {
        self._put_node(node)
    }
}

impl BranchView for BranchViewImpl {
    fn set(&mut self, vals: Vec<ViewValue>) -> Result<&mut Self, MarshalError> {
        self.children().clear();
        for val in vals {
            self.put(val)?;
        }
        Ok(self)
    }

    fn put(&mut self, val: ViewValue) -> Result<&mut Self, MarshalError> {
        let node = val_to_node(val)?;
        Ok(self._put_node(node))
    }

    fn subtree<V: NodeView>(&mut self, subtree: &V) -> &mut Self {
        let node = subtree.node().clone();
        self._put_node(node)
    }
}
