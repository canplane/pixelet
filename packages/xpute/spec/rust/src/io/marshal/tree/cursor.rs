// @xpute/spec/io/marshal/tree/cursor.rs

use crate::io::marshal::tree::spec::{
    align_sz, get_word, node_type_from_u8, node_type_to_u8, CursorValue, NodeType, ScalarType,
    ScalarValue, SequenceType, SequenceValue, SpecialType, DESC_TYPE_MASK, HDR_SZ, MAGIC, WORD_SZ,
};
use crate::sys::errno;
use crate::sys::error::MarshalError;

#[derive(Clone, Debug)]
pub struct TreeReader {
    pkt: Vec<u8>,
    root: NodeCursor,
}

impl TreeReader {
    pub fn new(pkt: Vec<u8>) -> Result<Self, MarshalError> {
        if pkt.len() < HDR_SZ as usize {
            return Err(MarshalError::new(errno::EBADMSG, "bad packet: truncated header"));
        }
        let (magic, _) = get_word(&pkt, 0)?;
        if magic != MAGIC {
            return Err(MarshalError::new(errno::EBADMSG, "bad packet: magic mismatch"));
        }
        let (payload_sz, desc) = get_word(&pkt, WORD_SZ)?;
        if payload_sz > pkt.len() as u32 - HDR_SZ {
            return Err(MarshalError::new(errno::EBADMSG, "bad packet: truncated payload"));
        }
        let node_type = node_type_from_u8((desc & DESC_TYPE_MASK) as u8)?;
        let root = if payload_sz == 0 {
            NodeCursor::Null(NullCursor { pkt: pkt.clone(), node_type })
        } else {
            cursor(pkt.clone(), HDR_SZ, node_type)?
        };
        Ok(Self { pkt, root })
    }

    pub fn read(&self) -> NodeCursor { self.root.clone() }

    pub fn read_branch(&self) -> Result<BranchCursor, MarshalError> {
        self.root.as_branch()
    }

    pub fn packet(&self) -> &[u8] { &self.pkt }
}

#[derive(Clone, Debug)]
pub enum NodeCursor {
    Node(LeafCursor),
    Null(NullCursor),
    Branch(BranchCursor),
}

impl NodeCursor {
    pub fn is_branch(&self) -> bool { matches!(self, Self::Branch(_)) }

    pub fn as_branch(&self) -> Result<BranchCursor, MarshalError> {
        match self {
            Self::Branch(v) => Ok(v.clone()),
            _ => Err(MarshalError::new(errno::EFAULT, "cannot cast a null node to a branch")),
        }
    }

    pub fn opt(&self) -> Option<&Self> {
        match self {
            Self::Null(_) => None,
            _ => Some(self),
        }
    }

    pub fn get(&self) -> Result<CursorValue, MarshalError> {
        match self {
            Self::Node(v) => v.get(),
            Self::Null(_) => Ok(CursorValue::Null),
            Self::Branch(v) => v.get(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LeafCursor {
    pkt: Vec<u8>,
    base: u32,
    node_type: NodeType,
}

impl LeafCursor {
    pub fn get(&self) -> Result<CursorValue, MarshalError> {
        match self.node_type {
            NodeType::Scalar(ty) => Ok(CursorValue::Scalar(self.read_scalar(ty)?)),
            NodeType::Sequence(ty) => Ok(CursorValue::Sequence(self.read_seq(ty)?)),
            _ => Err(MarshalError::new(errno::EBADMSG, "unknown node type")),
        }
    }

    fn read_scalar(&self, ty: ScalarType) -> Result<ScalarValue, MarshalError> {
        let off = self.base as usize;
        let need = match ty {
            ScalarType::U8 | ScalarType::I8 | ScalarType::Bool => 1,
            ScalarType::U16 | ScalarType::I16 => 2,
            ScalarType::U32 | ScalarType::I32 | ScalarType::F32 => 4,
            ScalarType::U64 | ScalarType::I64 | ScalarType::F64 => 8,
        };
        if off + need > self.pkt.len() {
            return Err(MarshalError::new(errno::EBADMSG, "scalar OOB"));
        }
        let value = match ty {
            ScalarType::U8 => ScalarValue::U8(self.pkt[off]),
            ScalarType::I8 => ScalarValue::I8(self.pkt[off] as i8),
            ScalarType::U16 => ScalarValue::U16(u16::from_le_bytes(self.pkt[off..off + 2].try_into().expect("u16"))),
            ScalarType::I16 => ScalarValue::I16(i16::from_le_bytes(self.pkt[off..off + 2].try_into().expect("i16"))),
            ScalarType::U32 => ScalarValue::U32(u32::from_le_bytes(self.pkt[off..off + 4].try_into().expect("u32"))),
            ScalarType::I32 => ScalarValue::I32(i32::from_le_bytes(self.pkt[off..off + 4].try_into().expect("i32"))),
            ScalarType::U64 => ScalarValue::U64(u64::from_le_bytes(self.pkt[off..off + 8].try_into().expect("u64"))),
            ScalarType::I64 => ScalarValue::I64(i64::from_le_bytes(self.pkt[off..off + 8].try_into().expect("i64"))),
            ScalarType::F32 => ScalarValue::F32(f32::from_le_bytes(self.pkt[off..off + 4].try_into().expect("f32"))),
            ScalarType::F64 => ScalarValue::F64(f64::from_le_bytes(self.pkt[off..off + 8].try_into().expect("f64"))),
            ScalarType::Bool => ScalarValue::Bool(self.pkt[off] != 0),
        };
        Ok(value)
    }

    fn read_seq(&self, ty: SequenceType) -> Result<SequenceValue, MarshalError> {
        let (len, _) = get_word(&self.pkt, self.base)?;
        let payload_start = self.base + WORD_SZ;
        if payload_start as usize > self.pkt.len() {
            return Err(MarshalError::new(errno::EBADMSG, "payload OOB"));
        }
        let payload = &self.pkt[payload_start as usize..];
        let value = match ty {
            SequenceType::U8Array => SequenceValue::U8Array(read_u8s(payload, len)?),
            SequenceType::I8Array => SequenceValue::I8Array(read_u8s(payload, len)?.into_iter().map(|v| v as i8).collect()),
            SequenceType::U16Array => SequenceValue::U16Array(read_words(payload, len, |b| u16::from_le_bytes(b.try_into().expect("u16")))?),
            SequenceType::I16Array => SequenceValue::I16Array(read_words(payload, len, |b| i16::from_le_bytes(b.try_into().expect("i16")))?),
            SequenceType::U32Array => SequenceValue::U32Array(read_words(payload, len, |b| u32::from_le_bytes(b.try_into().expect("u32")))?),
            SequenceType::I32Array => SequenceValue::I32Array(read_words(payload, len, |b| i32::from_le_bytes(b.try_into().expect("i32")))?),
            SequenceType::U64Array => SequenceValue::U64Array(read_words(payload, len, |b| u64::from_le_bytes(b.try_into().expect("u64")))?),
            SequenceType::I64Array => SequenceValue::I64Array(read_words(payload, len, |b| i64::from_le_bytes(b.try_into().expect("i64")))?),
            SequenceType::F32Array => SequenceValue::F32Array(read_words(payload, len, |b| f32::from_le_bytes(b.try_into().expect("f32")))?),
            SequenceType::F64Array => SequenceValue::F64Array(read_words(payload, len, |b| f64::from_le_bytes(b.try_into().expect("f64")))?),
            SequenceType::Bitset => SequenceValue::Bitset(unpack_bitset(payload, len)?),
            SequenceType::Str => {
                if payload.len() < len as usize + 1 {
                    return Err(MarshalError::new(errno::EBADMSG, "payload OOB"));
                }
                if payload[len as usize] != 0 {
                    return Err(MarshalError::new(errno::EBADMSG, "missing string terminator"));
                }
                SequenceValue::Str(
                    String::from_utf8(payload[..len as usize].to_vec())
                        .map_err(|_| MarshalError::new(errno::EBADMSG, "invalid utf-8"))?,
                )
            }
        };
        Ok(value)
    }
}

#[derive(Clone, Debug)]
pub struct NullCursor {
    pkt: Vec<u8>,
    node_type: NodeType,
}

impl NullCursor {
    pub fn opt(&self) -> Option<&Self> { None }
    pub fn node_type(&self) -> NodeType { self.node_type }
    pub fn packet_len(&self) -> usize { self.pkt.len() }
}

#[derive(Clone, Debug)]
pub struct BranchCursor {
    pkt: Vec<u8>,
    base: u32,
    len: u32,
    child_start: u32,
}

impl BranchCursor {
    pub fn len(&self) -> u32 { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }

    pub fn get(&self) -> Result<CursorValue, MarshalError> {
        Ok(CursorValue::Branch(self.get_deep()?))
    }

    pub fn get_deep(&self) -> Result<Vec<CursorValue>, MarshalError> {
        let mut out = Vec::with_capacity(self.len as usize);
        for index in 0..self.len {
            let child = self.at(index)?;
            let value = match &child {
                NodeCursor::Branch(branch) => CursorValue::Branch(branch.get_deep()?),
                _ => child.get()?,
            };
            out.push(value);
        }
        Ok(out)
    }

    pub fn at(&self, idx: u32) -> Result<NodeCursor, MarshalError> {
        if idx >= self.len {
            return Err(MarshalError::new(errno::EFAULT, format!("child index out of bounds: {idx}")));
        }
        let entry_off = (self.base + WORD_SZ) + idx * WORD_SZ;
        let (rel_off, desc) = get_word(&self.pkt, entry_off)?;
        let node_type = node_type_from_u8((desc & DESC_TYPE_MASK) as u8)?;
        if rel_off == 0 {
            return Ok(NodeCursor::Null(NullCursor { pkt: self.pkt.clone(), node_type }));
        }
        if rel_off > self.pkt.len() as u32 - self.base {
            return Err(MarshalError::new(errno::EBADMSG, "child offset overflow"));
        }
        let base = self.base + rel_off;
        if base < self.child_start || base >= self.pkt.len() as u32 {
            return Err(MarshalError::new(
                errno::EBADMSG,
                format!("bad child offset: base={} rel_off={rel_off}", self.base),
            ));
        }
        cursor(self.pkt.clone(), base, node_type)
    }

    /// Convenience: get child at index as a BranchCursor (errors if null or non-branch).
    pub fn at_branch(&self, idx: u32) -> Result<BranchCursor, MarshalError> {
        self.at(idx)?.as_branch()
    }

    pub fn children(&self) -> Result<Vec<NodeCursor>, MarshalError> {
        let mut out = Vec::with_capacity(self.len as usize);
        for index in 0..self.len {
            out.push(self.at(index)?);
        }
        Ok(out)
    }
}

fn cursor(pkt: Vec<u8>, base: u32, node_type: NodeType) -> Result<NodeCursor, MarshalError> {
    let align_sz = align_sz(node_type)?;
    if !base.is_multiple_of(align_sz) {
        return Err(MarshalError::new(
            errno::EBADMSG,
            format!(
                "misaligned node base {} for type 0x{:x}",
                base,
                node_type_to_u8(node_type)
            ),
        ));
    }

    match node_type {
        NodeType::Special(SpecialType::Branch) => {
            let (len, _) = get_word(&pkt, base)?;
            let table_off = base + WORD_SZ;
            if len > (pkt.len() as u32 - table_off) / WORD_SZ {
                return Err(MarshalError::new(errno::EBADMSG, "branch table overflow"));
            }
            Ok(NodeCursor::Branch(BranchCursor {
                pkt,
                base,
                len,
                child_start: (base + WORD_SZ) + len * WORD_SZ,
            }))
        }
        NodeType::Scalar(_) | NodeType::Sequence(_) => Ok(NodeCursor::Node(LeafCursor { pkt, base, node_type })),
        _ => Err(MarshalError::new(
            errno::EBADMSG,
            format!("unsupported special node type: 0x{:x}", node_type_to_u8(node_type)),
        )),
    }
}

fn read_u8s(payload: &[u8], len: u32) -> Result<Vec<u8>, MarshalError> {
    if payload.len() < len as usize {
        return Err(MarshalError::new(errno::EBADMSG, "payload OOB"));
    }
    Ok(payload[..len as usize].to_vec())
}

fn read_words<T, F>(payload: &[u8], len: u32, decode: F) -> Result<Vec<T>, MarshalError>
where
    F: Fn(&[u8]) -> T,
{
    let elem_sz = std::mem::size_of::<T>();
    let need = len as usize * elem_sz;
    if payload.len() < need {
        return Err(MarshalError::new(errno::EBADMSG, "payload OOB"));
    }
    let mut out = Vec::with_capacity(len as usize);
    for chunk in payload[..need].chunks_exact(elem_sz) {
        out.push(decode(chunk));
    }
    Ok(out)
}

fn unpack_bitset(payload: &[u8], len: u32) -> Result<Vec<u8>, MarshalError> {
    let nbyte = len.div_ceil(8) as usize;
    if payload.len() < nbyte {
        return Err(MarshalError::new(errno::EBADMSG, "payload OOB"));
    }
    let mut out = vec![0; len as usize];
    for index in 0..len as usize {
        out[index] = u8::from((payload[index >> 3] & (1 << (index & 7))) != 0);
    }
    Ok(out)
}
