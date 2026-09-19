// xpute-core/wire/xtp/view.rs

//! The write end: a `TreeView` holds a root node, a `BranchView` a branch's
//! children, and both write through the same typed writers — `NodeView` is
//! one write primitive (`_set_node` for a tree, `_put_node` for a branch),
//! and every writer on top of it.

// Uppercase throughout, as the notation these operations are written in: one
// that holds no state and closes over nothing is named the way a C header
// names its macros. Rust's own answer to a macro like that is a `const fn`,
// which it cases snake, so the notation and the language disagree here and
// the file keeps the notation — suspended once for the file, which is the
// level the choice is made at rather than item by item.
#![allow(non_snake_case)]

use crate::status::error::MarshalError;

use super::spec::{Array, BranchNode, GraftNode, Node, NodeValue, Numeric, ScalarNode, ScalarType, SequenceNode, SequenceType, SequenceVal, NIL_NODE};

fn scalar<'a>(type_: ScalarType, val: Option<Numeric>) -> Node<'a> {
    Node::Scalar(ScalarNode { type_, val })
}
fn seq<'a>(type_: SequenceType, val: Option<SequenceVal<'a>>) -> Node<'a> {
    Node::Sequence(SequenceNode { type_, val })
}
fn array<'a, T: Copy>(type_: SequenceType, arr: Option<&'a [T]>) -> Option<SequenceVal<'a>> {
    arr.map(|a| {
        // SAFETY: a leaf's elements are its bytes, little-endian, as the
        // packet lays them out.
        let bytes = unsafe { core::slice::from_raw_parts(a.as_ptr() as *const u8, core::mem::size_of_val(a)) };
        SequenceVal::Array(Array { type_, bytes: bytes.into() })
    })
}

const U8_NODE: fn(Option<u8>) -> Node<'static> = |u| scalar(ScalarType::U8, u.map(Numeric::U8));
const I8_NODE: fn(Option<i8>) -> Node<'static> = |i| scalar(ScalarType::I8, i.map(Numeric::I8));
const U16_NODE: fn(Option<u16>) -> Node<'static> = |u| scalar(ScalarType::U16, u.map(Numeric::U16));
const I16_NODE: fn(Option<i16>) -> Node<'static> = |i| scalar(ScalarType::I16, i.map(Numeric::I16));
const U32_NODE: fn(Option<u32>) -> Node<'static> = |u| scalar(ScalarType::U32, u.map(Numeric::U32));
const I32_NODE: fn(Option<i32>) -> Node<'static> = |i| scalar(ScalarType::I32, i.map(Numeric::I32));
const U64_NODE: fn(Option<u64>) -> Node<'static> = |u| scalar(ScalarType::U64, u.map(Numeric::U64));
const I64_NODE: fn(Option<i64>) -> Node<'static> = |i| scalar(ScalarType::I64, i.map(Numeric::I64));
const F32_NODE: fn(Option<f32>) -> Node<'static> = |f| scalar(ScalarType::F32, f.map(Numeric::F32));
const F64_NODE: fn(Option<f64>) -> Node<'static> = |f| scalar(ScalarType::F64, f.map(Numeric::F64));

const BOOL_NODE: fn(Option<bool>) -> Node<'static> = |b| scalar(ScalarType::BOOL, b.map(|b| Numeric::U8(b as u8)));

fn ARRAY_NODE<'a, T: Copy>(type_: SequenceType, arr: Option<&'a [T]>) -> Node<'a> {
    seq(type_, array(type_, arr))
}
fn BITSET_NODE<'a>(arr: Option<&'a [u8]>) -> Node<'a> {
    seq(SequenceType::BITSET, array(SequenceType::BITSET, arr))
}
fn STR_NODE<'a>(s: Option<&str>) -> Node<'a> {
    seq(SequenceType::STR, s.map(|s| SequenceVal::Str(s.to_string())))
}

/// A view's node, as the value a `set` or `put` takes in place of the view.
pub type ViewValue<'a> = NodeValue<'a, Node<'a>>;

fn val_to_node<'a>(val: ViewValue<'a>) -> Result<Node<'a>, MarshalError> {
    Ok(match val {
        NodeValue::Extra(node) => node,

        NodeValue::Null => NIL_NODE,

        NodeValue::U8(v) => U8_NODE(Some(v)),
        NodeValue::I8(v) => I8_NODE(Some(v)),
        NodeValue::U16(v) => U16_NODE(Some(v)),
        NodeValue::I16(v) => I16_NODE(Some(v)),
        NodeValue::U32(v) => U32_NODE(Some(v)),
        NodeValue::I32(v) => I32_NODE(Some(v)),
        NodeValue::U64(v) => U64_NODE(Some(v)),
        NodeValue::I64(v) => I64_NODE(Some(v)),
        NodeValue::F32(v) => F32_NODE(Some(v)),
        NodeValue::F64(v) => F64_NODE(Some(v)),
        NodeValue::Bool(b) => BOOL_NODE(Some(b)),
        NodeValue::Str(s) => STR_NODE(Some(&s)),

        NodeValue::Array(a) => seq(a.type_, Some(SequenceVal::Array(a))),

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

pub trait NodeView<'a> {
    fn node(&self) -> &Node<'a>;

    /// The write primitive: a tree replaces its root, a branch appends a child.
    fn write(&mut self, node: Node<'a>) -> &mut Self;

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

    fn u8_array(&mut self, arr: Option<&'a [u8]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::U8_ARRAY, arr))
    }
    fn i8_array(&mut self, arr: Option<&'a [i8]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::I8_ARRAY, arr))
    }
    fn u16_array(&mut self, arr: Option<&'a [u16]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::U16_ARRAY, arr))
    }
    fn i16_array(&mut self, arr: Option<&'a [i16]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::I16_ARRAY, arr))
    }
    fn u32_array(&mut self, arr: Option<&'a [u32]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::U32_ARRAY, arr))
    }
    fn i32_array(&mut self, arr: Option<&'a [i32]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::I32_ARRAY, arr))
    }
    fn u64_array(&mut self, arr: Option<&'a [u64]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::U64_ARRAY, arr))
    }
    fn i64_array(&mut self, arr: Option<&'a [i64]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::I64_ARRAY, arr))
    }
    fn f32_array(&mut self, arr: Option<&'a [f32]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::F32_ARRAY, arr))
    }
    fn f64_array(&mut self, arr: Option<&'a [f64]>) -> &mut Self {
        self.write(ARRAY_NODE(SequenceType::F64_ARRAY, arr))
    }

    fn bitset(&mut self, arr: Option<&'a [u8]>) -> &mut Self {
        self.write(BITSET_NODE(arr))
    }

    fn str(&mut self, s: Option<&str>) -> &mut Self {
        self.write(STR_NODE(s))
    }

    // ---- Subtree ----

    /// Builds a branch subtree inline and writes it into the current view.
    fn branch(&mut self, f: Option<&mut dyn FnMut(&mut BranchViewImpl<'a>)>) -> &mut Self {
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
    /// - The grafted root may be physically null
    /// - The caller must not mutate `pkt` after grafting it
    ///
    /// This method enforces only construction-boundary checks.
    /// Packet header validation remains a read/encode-side concern.
    fn graft(&mut self, pkt: &'a [u8]) -> Result<&mut Self, MarshalError> {
        Ok(self.write(Node::Graft(GraftNode { val: pkt })))
    }
}

pub struct TreeView<'a> {
    pub node: Node<'a>,
}

impl Default for TreeView<'_> {
    fn default() -> Self {
        TreeView::new()
    }
}

impl<'a> TreeView<'a> {
    pub fn new() -> TreeView<'a> {
        TreeView { node: NIL_NODE }
    }

    // ---- Write Primitive ----

    /// Replaces the current root node.
    fn _set_node(&mut self, node: Node<'a>) -> &mut Self {
        self.node = node;
        self
    }

    /// Lowers a value into the root node: each width its own leaf, an array
    /// the matching array node, a list a branch in element order.
    ///
    /// There is no keyed value here — an object is a higher schema's to
    /// model — and the explicit writers, `u8()` and `i64()` and the rest,
    /// are the caller's choice of leaf rather than a lowering to check.
    pub fn set(&mut self, val: ViewValue<'a>) -> Result<&mut Self, MarshalError> {
        let node = val_to_node(val)?;
        Ok(self._set_node(node))
    }
}

impl<'a> NodeView<'a> for TreeView<'a> {
    fn node(&self) -> &Node<'a> {
        &self.node
    }

    fn write(&mut self, node: Node<'a>) -> &mut Self {
        self._set_node(node)
    }
}

/// The branch view: what `branch(|b| ...)` hands its closure.
pub trait BranchView<'a>: NodeView<'a> {
    /// Replaces all existing children with the provided sequence.
    fn set(&mut self, vals: Vec<ViewValue<'a>>) -> Result<&mut Self, MarshalError>;

    /// Lowers one generic value and appends it as a child.
    fn put(&mut self, val: ViewValue<'a>) -> Result<&mut Self, MarshalError>;

    /// Appends an in-memory subtree as one child.
    /// The subtree is encoded inline as part of the current tree.
    fn subtree<V: NodeView<'a>>(&mut self, subtree: &V) -> &mut Self;
}

pub struct BranchViewImpl<'a> {
    pub node: Node<'a>,
}

impl Default for BranchViewImpl<'_> {
    fn default() -> Self {
        BranchViewImpl::new()
    }
}

impl<'a> BranchViewImpl<'a> {
    pub fn new() -> BranchViewImpl<'a> {
        BranchViewImpl {
            node: Node::Branch(BranchNode { val: Some(Vec::new()) }),
        }
    }

    fn children(&mut self) -> &mut Vec<Node<'a>> {
        match &mut self.node {
            Node::Branch(BranchNode { val: Some(children) }) => children,
            _ => unreachable!("a branch view holds a branch with children"),
        }
    }

    // ---- Append Primitive ----

    /// Appends one child node to the current branch.
    fn _put_node(&mut self, node: Node<'a>) -> &mut Self {
        self.children().push(node);
        self
    }
}

impl<'a> NodeView<'a> for BranchViewImpl<'a> {
    fn node(&self) -> &Node<'a> {
        &self.node
    }

    fn write(&mut self, node: Node<'a>) -> &mut Self {
        self._put_node(node)
    }
}

impl<'a> BranchView<'a> for BranchViewImpl<'a> {
    fn set(&mut self, vals: Vec<ViewValue<'a>>) -> Result<&mut Self, MarshalError> {
        self.children().clear();
        for val in vals {
            self.put(val)?;
        }
        Ok(self)
    }

    fn put(&mut self, val: ViewValue<'a>) -> Result<&mut Self, MarshalError> {
        let node = val_to_node(val)?;
        Ok(self._put_node(node))
    }

    fn subtree<V: NodeView<'a>>(&mut self, subtree: &V) -> &mut Self {
        let node = subtree.node().clone();
        self._put_node(node)
    }
}
