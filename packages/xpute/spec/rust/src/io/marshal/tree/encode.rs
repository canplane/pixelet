// @xpute/spec/io/marshal/tree/encode.rs

use crate::io::marshal::tree::spec::{
    align, align_sz, get_word, node_is_branch, node_is_graft, node_is_leaf, node_is_nil,
    node_is_seq, node_type_from_u8, node_type_to_u8, set_word, EncodingState, Node, NodeType,
    ScalarValue, SequenceType, SequenceValue, SpecialType, DESC_TYPE_MASK,
    DESC_TYPE_SHAMT, HDR_SZ, MAGIC, MAX_PKT_SZ, RESERVED, WORD_SZ,
};
use crate::lib::kit::word::field_set;
use crate::sys::errno;
use crate::sys::error::MarshalError;

use super::view::TreeView;

#[derive(Clone, Debug)]
pub struct TreeEncoderOptions {
    pub init_cap: u32,
    pub max_cap: u32,
}

impl Default for TreeEncoderOptions {
    fn default() -> Self {
        Self { init_cap: 1 << 10, max_cap: MAX_PKT_SZ }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TreeEncoder {
    max_cap: u32,
    buf: Vec<u8>,
}

impl TreeEncoder {
    fn ensure(&mut self, new_cap: u32) -> Result<(), MarshalError> {
        if new_cap <= self.buf.len() as u32 {
            return Ok(());
        }
        let grown = (self.buf.len() as u32).saturating_mul(2).max(new_cap);
        if grown > self.max_cap {
            return Err(MarshalError::new(
                errno::EOVERFLOW,
                format!("required packet size exceeds max cap ({})", self.max_cap),
            ));
        }
        self.buf.resize(grown as usize, 0);
        Ok(())
    }

    pub fn encode_with_options(
        &mut self,
        view: &TreeView,
        opts: TreeEncoderOptions,
    ) -> Result<Vec<u8>, MarshalError> {
        if opts.max_cap < HDR_SZ || opts.max_cap > MAX_PKT_SZ {
            return Err(MarshalError::new(
                errno::EINVAL,
                format!("bad max cap {} (expected {}..{})", opts.max_cap, HDR_SZ, MAX_PKT_SZ),
            ));
        }

        self.max_cap = opts.max_cap;
        let init_cap = opts.init_cap.max(HDR_SZ).min(self.max_cap);
        self.buf = vec![0; init_cap as usize];

        set_word(&mut self.buf, 0, MAGIC, RESERVED);

        let mut st = EncodingState {
            base: HDR_SZ,
            lim: HDR_SZ,
            node_type: node_type_of(&view.node),
        };
        self.node(&view.node, &mut st)?;

        let payload_sz = st.lim - HDR_SZ;
        let desc = field_set(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, u32::from(node_type_to_u8(node_type_of(&view.node))));
        set_word(&mut self.buf, WORD_SZ, payload_sz, desc);

        let pkt_nbyte = align(HDR_SZ + payload_sz, WORD_SZ)?;
        self.ensure(pkt_nbyte)?;
        self.buf.truncate(pkt_nbyte as usize);
        Ok(self.buf.clone())
    }

    pub fn encode(&mut self, view: &TreeView) -> Result<Vec<u8>, MarshalError> {
        self.encode_with_options(view, TreeEncoderOptions::default())
    }

    fn node(&mut self, node: &Node, st: &mut EncodingState) -> Result<(), MarshalError> {
        if node_is_leaf(node) {
            if node_is_seq(node) {
                return self.seq(node, st);
            }
            return self.scalar(node, st);
        }
        if node_is_branch(node) {
            return self.branch(node, st);
        }
        if node_is_graft(node) {
            return self.graft(node, st);
        }
        if node_is_nil(node) {
            return Ok(());
        }
        Err(MarshalError::new(errno::EBADMSG, "unknown node"))
    }

    fn graft(&mut self, node: &Node, st: &mut EncodingState) -> Result<(), MarshalError> {
        let Node::Graft(pkt) = node else { return Err(MarshalError::new(errno::EBADMSG, "bad graft node")); };
        if pkt.len() < HDR_SZ as usize {
            return Err(MarshalError::new(errno::EBADMSG, "bad graft packet: truncated header"));
        }
        let (magic, _) = get_word(pkt, 0)?;
        if magic != MAGIC {
            return Err(MarshalError::new(errno::EBADMSG, "bad graft packet: magic mismatch"));
        }
        let (payload_sz, desc) = get_word(pkt, WORD_SZ)?;
        let node_type = node_type_from_u8((desc & DESC_TYPE_MASK) as u8)?;
        st.node_type = node_type;
        if payload_sz == 0 {
            return Ok(());
        }
        let align_sz = align_sz(node_type)?;
        if payload_sz % align_sz != 0 {
            return Err(MarshalError::new(errno::EBADMSG, format!("bad graft payload size: {payload_sz}")));
        }
        st.base = align(st.base, align_sz)?;
        self.ensure(st.base + payload_sz)?;
        self.buf[st.base as usize..(st.base + payload_sz) as usize]
            .copy_from_slice(&pkt[HDR_SZ as usize..(HDR_SZ + payload_sz) as usize]);
        st.lim = st.base + payload_sz;
        Ok(())
    }

    fn branch(&mut self, node: &Node, st: &mut EncodingState) -> Result<(), MarshalError> {
        let Node::Branch(children) = node else { return Err(MarshalError::new(errno::EBADMSG, "bad branch node")); };
        let Some(children) = children else { return Ok(()); };

        st.base = align(st.base, WORD_SZ)?;
        let table_start = st.base + WORD_SZ;
        let len = children.len() as u32;
        self.ensure(table_start + len * WORD_SZ)?;
        set_word(&mut self.buf, st.base, len, RESERVED);

        let mut child_st = EncodingState {
            base: table_start + len * WORD_SZ,
            lim: table_start + len * WORD_SZ,
            node_type: NodeType::Special(SpecialType::Nil),
        };

        for (index, child) in children.iter().enumerate() {
            child_st.base = child_st.lim;
            child_st.node_type = node_type_of(child);
            self.node(child, &mut child_st)?;
            let rel_off = if child_st.base == child_st.lim { 0 } else { child_st.base - st.base };
            let entry_off = table_start + (index as u32) * WORD_SZ;
            let desc = field_set(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, u32::from(node_type_to_u8(child_st.node_type)));
            set_word(&mut self.buf, entry_off, rel_off, desc);
        }

        st.lim = align(child_st.lim, WORD_SZ)?;
        self.ensure(st.lim)?;
        Ok(())
    }

    fn scalar(&mut self, node: &Node, st: &mut EncodingState) -> Result<(), MarshalError> {
        let Node::Scalar(node_type, value) = node else { return Err(MarshalError::new(errno::EBADMSG, "bad scalar node")); };
        let Some(value) = value else { return Ok(()); };

        let elem_sz = align_sz(NodeType::Scalar(*node_type))?;
        st.base = align(st.base, elem_sz)?;
        st.lim = st.base + elem_sz;
        self.ensure(st.lim)?;
        let off = st.base as usize;

        match value {
            ScalarValue::U8(v) => self.buf[off] = *v,
            ScalarValue::I8(v) => self.buf[off] = *v as u8,
            ScalarValue::U16(v) => self.buf[off..off + 2].copy_from_slice(&v.to_le_bytes()),
            ScalarValue::I16(v) => self.buf[off..off + 2].copy_from_slice(&v.to_le_bytes()),
            ScalarValue::U32(v) => self.buf[off..off + 4].copy_from_slice(&v.to_le_bytes()),
            ScalarValue::I32(v) => self.buf[off..off + 4].copy_from_slice(&v.to_le_bytes()),
            ScalarValue::U64(v) => self.buf[off..off + 8].copy_from_slice(&v.to_le_bytes()),
            ScalarValue::I64(v) => self.buf[off..off + 8].copy_from_slice(&v.to_le_bytes()),
            ScalarValue::F32(v) => self.buf[off..off + 4].copy_from_slice(&v.to_le_bytes()),
            ScalarValue::F64(v) => self.buf[off..off + 8].copy_from_slice(&v.to_le_bytes()),
            ScalarValue::Bool(v) => self.buf[off] = u8::from(*v),
        }
        Ok(())
    }

    fn seq(&mut self, node: &Node, st: &mut EncodingState) -> Result<(), MarshalError> {
        let Node::Sequence(node_type, value) = node else { return Err(MarshalError::new(errno::EBADMSG, "bad sequence node")); };
        let Some(value) = value else { return Ok(()); };

        st.base = align(st.base, WORD_SZ)?;
        let payload_start = st.base + WORD_SZ;

        let (len, payload) = match (node_type, value) {
            (SequenceType::U8Array, SequenceValue::U8Array(v)) => (v.len() as u32, v.clone()),
            (SequenceType::I8Array, SequenceValue::I8Array(v)) => (v.len() as u32, v.iter().map(|x| *x as u8).collect()),
            (SequenceType::U16Array, SequenceValue::U16Array(v)) => (v.len() as u32, flatten_words(v.iter().map(|x| x.to_le_bytes().to_vec()).collect())),
            (SequenceType::I16Array, SequenceValue::I16Array(v)) => (v.len() as u32, flatten_words(v.iter().map(|x| x.to_le_bytes().to_vec()).collect())),
            (SequenceType::U32Array, SequenceValue::U32Array(v)) => (v.len() as u32, flatten_words(v.iter().map(|x| x.to_le_bytes().to_vec()).collect())),
            (SequenceType::I32Array, SequenceValue::I32Array(v)) => (v.len() as u32, flatten_words(v.iter().map(|x| x.to_le_bytes().to_vec()).collect())),
            (SequenceType::U64Array, SequenceValue::U64Array(v)) => (v.len() as u32, flatten_words(v.iter().map(|x| x.to_le_bytes().to_vec()).collect())),
            (SequenceType::I64Array, SequenceValue::I64Array(v)) => (v.len() as u32, flatten_words(v.iter().map(|x| x.to_le_bytes().to_vec()).collect())),
            (SequenceType::F32Array, SequenceValue::F32Array(v)) => (v.len() as u32, flatten_words(v.iter().map(|x| x.to_le_bytes().to_vec()).collect())),
            (SequenceType::F64Array, SequenceValue::F64Array(v)) => (v.len() as u32, flatten_words(v.iter().map(|x| x.to_le_bytes().to_vec()).collect())),
            (SequenceType::Bitset, SequenceValue::Bitset(v)) => (v.len() as u32, pack_bitset(v)),
            (SequenceType::Str, SequenceValue::Str(v)) => {
                let mut bytes = v.as_bytes().to_vec();
                let len = bytes.len() as u32;
                bytes.push(0);
                (len, bytes)
            }
            _ => return Err(MarshalError::new(errno::EBADMSG, "sequence type/value mismatch")),
        };

        self.ensure(payload_start + payload.len() as u32)?;
        self.buf[payload_start as usize..(payload_start + payload.len() as u32) as usize]
            .copy_from_slice(&payload);
        set_word(&mut self.buf, st.base, len, RESERVED);
        st.lim = payload_start + align(payload.len() as u32, WORD_SZ)?;
        self.ensure(st.lim)?;
        Ok(())
    }
}

fn pack_bitset(bits: &[u8]) -> Vec<u8> {
    let mut out = vec![0; bits.len().div_ceil(8)];
    for (index, bit) in bits.iter().enumerate() {
        if *bit != 0 {
            out[index >> 3] |= 1 << (index & 7);
        }
    }
    out
}

fn flatten_words(parts: Vec<Vec<u8>>) -> Vec<u8> {
    let total = parts.iter().map(Vec::len).sum();
    let mut out = Vec::with_capacity(total);
    for part in parts {
        out.extend_from_slice(&part);
    }
    out
}

fn node_type_of(node: &Node) -> NodeType {
    match node {
        Node::Nil => NodeType::Special(SpecialType::Nil),
        Node::Scalar(ty, _) => NodeType::Scalar(*ty),
        Node::Sequence(ty, _) => NodeType::Sequence(*ty),
        Node::Branch(_) => NodeType::Special(SpecialType::Branch),
        Node::Graft(_) => NodeType::Special(SpecialType::Graft),
    }
}

#[cfg(test)]
mod tests {
    use super::TreeEncoder;
    use crate::io::marshal::tree::{CursorValue, ScalarValue, SequenceValue};
    use crate::io::marshal::tree::cursor::TreeReader;
    use crate::io::marshal::tree::view::{BranchView, TreeView};

    #[test]
    fn round_trips_branch_tree() {
        let mut tree = TreeView::new();
        tree.branch(Some(|b: &mut BranchView| {
            b.u32(Some(42));
            b.str(Some("hello".to_string()));
            b.branch(Some(|b2: &mut BranchView| {
                b2.u32(Some(7));
            }));
        }));

        let packet = TreeEncoder::default().encode(&tree).expect("encode");
        let reader = TreeReader::new(packet).expect("reader");
        let root = reader.read_branch().expect("branch");
        let deep = root.get_deep().expect("deep");

        match &deep[0] {
            CursorValue::Scalar(ScalarValue::U32(v)) => assert_eq!(*v, 42),
            _ => panic!("expected u32"),
        }
        match &deep[1] {
            CursorValue::Sequence(SequenceValue::Str(v)) => assert_eq!(v, "hello"),
            _ => panic!("expected string"),
        }
        match &deep[2] {
            CursorValue::Branch(children) => assert_eq!(children.len(), 1),
            _ => panic!("expected branch"),
        }
    }

    #[test]
    fn rejects_misaligned_sequence_child() {
        let mut tree = TreeView::new();
        tree.branch(Some(|b: &mut BranchView| {
            b.u32(Some(1));
            b.str(Some("hello".to_string()));
        }));

        let mut packet = TreeEncoder::default().encode(&tree).expect("encode");

        let branch_base = 16usize;
        let second_entry = branch_base + 8 + 8;
        packet[second_entry..second_entry + 4].copy_from_slice(&21u32.to_le_bytes());

        let reader = TreeReader::new(packet).expect("reader");
        let root = reader.read_branch().expect("branch");
        let err = root.at(1).expect_err("misaligned child must fail");
        assert_eq!(err.errno(), crate::sys::errno::EBADMSG);
    }
}
