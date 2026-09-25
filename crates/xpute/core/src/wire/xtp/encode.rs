// xpute-core/wire/xtp/encode.rs

use crate::abi::word::{field_get32, field_set32};
use crate::status::errno::Errno;
use crate::status::error::MarshalError;

use super::spec::{
    AlignUnit, BranchNode, EncodingState, GraftNode, Node, NodeType, Numeric, ScalarNode, ScalarType, SequenceNode, SequenceType, SequenceVal, SpecialType, ALIGN, ALIGN_SZ, DESC_TYPE_MASK,
    DESC_TYPE_SHAMT, ELEM_SZ, GET_WORD, HDR_SZ, MAGIC, MAX_PKT_SZ, NODE_IS_BRANCH, NODE_IS_GRAFT, NODE_IS_LEAF, NODE_IS_NIL, NODE_IS_SEQ, NONE, RESERVED, SET_WORD, WORD_SZ,
};
use super::view::NodeView;

/// Scratchpad for 64-bit word operations to avoid allocation.
/// [lo32, hi32]
const WORD_REG: [u32; 2] = [NONE, NONE];

// ============ Encoder ============

#[derive(Default)]
pub struct TreeEncoderOptions {
    pub init_cap: Option<u32>, // soft initial buffer size hint
    pub max_cap: Option<u32>,  // hard packet size limit
}

// The encoder assumes the normal construction path went through the view layer.
// It still enforces minimal packet-boundary invariants such as cap/offset/type fallback.
pub struct TreeEncoder {
    pub max_cap: u32,

    pub buf: Vec<u8>,
}

impl Default for TreeEncoder {
    fn default() -> Self {
        TreeEncoder::new()
    }
}

impl TreeEncoder {
    pub fn new() -> TreeEncoder {
        TreeEncoder { max_cap: MAX_PKT_SZ, buf: Vec::new() }
    }

    // ---- Buffer ----

    fn _ensure(&mut self, new_cap: u32) -> Result<u32, MarshalError> {
        let cap = self.buf.len() as u32;
        if new_cap <= cap {
            return Ok(cap);
        }

        let new_cap = (cap * 2).max(new_cap);
        if new_cap > self.max_cap {
            return Err(MarshalError::new(Errno::EOVERFLOW));
        }

        self.buf.resize(new_cap as usize, 0);
        Ok(self.buf.len() as u32)
    }

    // ---- Entry ----

    pub fn encode<'a, V: NodeView<'a>>(&mut self, view: &V, opts: TreeEncoderOptions) -> Result<Vec<u8>, MarshalError> {
        let node = view.node();

        let _init_cap = opts.init_cap.unwrap_or(1 << 10);
        let max_cap = opts.max_cap.unwrap_or(MAX_PKT_SZ);
        if !(HDR_SZ..=MAX_PKT_SZ).contains(&max_cap) {
            return Err(MarshalError::new(Errno::EINVAL));
        }
        self.max_cap = max_cap;

        let init_cap = HDR_SZ.max(_init_cap.min(self.max_cap));
        self.buf = vec![0u8; init_cap as usize];

        SET_WORD(&mut self.buf, 0, MAGIC, RESERVED); // word0

        // packet payload
        let mut st = EncodingState {
            base: HDR_SZ,
            lim: HDR_SZ,
            type_: node.type_(),
        };
        self._node(node, &mut st)?;

        // root node kind is preserved even when payload_sz == 0
        let payload_sz = st.lim - HDR_SZ;
        let desc = field_set32(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, node.type_() as u32);
        SET_WORD(&mut self.buf, WORD_SZ, payload_sz, desc); // word1

        let pkt_nbyte = ALIGN(HDR_SZ + payload_sz, WORD_SZ)?;
        self._ensure(pkt_nbyte)?;
        Ok(self.buf[..pkt_nbyte as usize].to_vec())
    }

    // ---- Node Dispatch ----

    fn _node(&mut self, node: &Node, st: &mut EncodingState) -> Result<(), MarshalError> {
        // leaf
        if NODE_IS_LEAF(node) {
            if NODE_IS_SEQ(node) {
                let Node::Sequence(n) = node else { unreachable!() };
                return self._seq(n, st);
            }
            let Node::Scalar(n) = node else { unreachable!() };
            return self._scalar(n, st);
        }

        // special
        if NODE_IS_BRANCH(node) {
            let Node::Branch(n) = node else { unreachable!() };
            return self._branch(n, st);
        }
        if NODE_IS_GRAFT(node) {
            let Node::Graft(n) = node else { unreachable!() };
            return self._graft(n, st);
        }
        if NODE_IS_NIL(node) {
            return Ok(());
        }
        Err(MarshalError::new(Errno::EBADMSG))
    }

    // graft semantics:
    // - `pkt` is a complete tree packet
    // - the outer packet header is stripped
    // - only the grafted root payload region is copied into the current packet
    // - packet header word1 stores [root_payload_sz 32 | type 8 | reserved 24]
    // - placement alignment follows the grafted root node type
    // - because root_payload_sz excludes the fixed packet header, graft preserves
    //   the same root payload layout as inline encoding
    // - base-offset alignment is expected to be enforced by the view construction path
    fn _graft(&mut self, node: &GraftNode, st: &mut EncodingState) -> Result<(), MarshalError> {
        let pkt = &node.val;

        if (pkt.len() as u32) < HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let hdr_view = &pkt[..HDR_SZ as usize];

        // packet header fallback
        let mut reg = WORD_REG;
        let [magic, _] = GET_WORD(hdr_view, 0, &mut reg); // word0
        if magic != MAGIC {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        let [payload_sz, desc] = GET_WORD(hdr_view, WORD_SZ, &mut reg); // word1
        if payload_sz > pkt.len() as u32 - HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        st.type_ = field_get32(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType; // hi

        if payload_sz == 0 {
            return Ok(());
        }

        let align_sz: AlignUnit = ALIGN_SZ(st.type_)?;
        if !payload_sz.is_multiple_of(align_sz) {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        st.base = ALIGN(st.base, align_sz)?;

        if payload_sz as i64 > self.max_cap as i64 - st.base as i64 {
            return Err(MarshalError::new(Errno::EOVERFLOW));
        }
        st.lim = st.base + payload_sz;
        self._ensure(st.lim)?;
        let at = st.base as usize;
        self.buf[at..at + payload_sz as usize].copy_from_slice(&pkt[HDR_SZ as usize..(HDR_SZ + payload_sz) as usize]);
        Ok(())
    }

    // ---- Branch Encoding ----

    // branch layout:
    // [len_lo32 | reserved_hi32]
    // [rel_off_lo32 | desc_hi32]...
    //
    // desc_hi32:
    //   bits 0..7   : node type
    //   bits 8..31  : reserved
    //
    // - len and offsets occupy 8-byte slots
    // - only the low 32 bits of len/off are currently interpreted
    // - each child offset is relative to the enclosing branch node base
    // - branch alignment is WORD_SZ
    fn _branch(&mut self, node: &BranchNode, st: &mut EncodingState) -> Result<(), MarshalError> {
        let Some(children) = &node.val else { return Ok(()) };

        st.base = ALIGN(st.base, WORD_SZ)?;
        let table_start = st.base + WORD_SZ;

        let len = children.len() as u32;
        // empty branch (len = 0) is valid — analogous to [] or {} in JSON
        if len > (self.max_cap >> 3) {
            return Err(MarshalError::new(Errno::EOVERFLOW));
        }

        st.lim = table_start + len * WORD_SZ;
        self._ensure(st.lim)?; // [len | child entries...]

        SET_WORD(&mut self.buf, st.base, len, RESERVED);

        let mut table_off = table_start;
        let mut child_st = EncodingState {
            base: st.lim,
            lim: st.lim,
            type_: SpecialType::NIL as u8,
        };
        for child in children {
            child_st.base = child_st.lim;
            child_st.type_ = child.type_();

            self._node(child, &mut child_st)?;
            let rel_off = if child_st.base == child_st.lim { 0 } else { child_st.base - st.base };

            // child node kind is preserved even when rel_off == 0
            SET_WORD(&mut self.buf, table_off, rel_off, field_set32(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, child_st.type_ as u32));

            table_off += WORD_SZ;
        }
        st.lim = ALIGN(child_st.lim, WORD_SZ)?;
        self._ensure(st.lim)?;
        Ok(())
    }

    // ---- Leaf Dispatch ----

    // scalar layout:
    // [payload]
    // - scalar nodes have no header
    // - scalar alignment is equal to the element size
    fn _scalar(&mut self, node: &ScalarNode, st: &mut EncodingState) -> Result<(), MarshalError> {
        let type_ = node.type_;
        let Some(val) = node.val else { return Ok(()) };

        let mismatch = || MarshalError::new(Errno::EINVAL);

        let w = |n: Numeric| -> Result<Vec<u8>, MarshalError> {
            Ok(match (type_, n) {
                (ScalarType::U8, Numeric::U8(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::I8, Numeric::I8(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::U16, Numeric::U16(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::I16, Numeric::I16(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::U32, Numeric::U32(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::I32, Numeric::I32(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::U64, Numeric::U64(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::I64, Numeric::I64(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::F32, Numeric::F32(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::F64, Numeric::F64(v)) => v.to_le_bytes().to_vec(),
                (ScalarType::BOOL, Numeric::U8(v)) => vec![(v != 0) as u8],
                _ => return Err(mismatch()),
            })
        };
        let payload = w(val)?;
        let elem_sz: AlignUnit = payload.len() as AlignUnit;

        st.base = ALIGN(st.base, elem_sz)?;

        st.lim = st.base + elem_sz;
        self._ensure(st.lim)?;
        self.buf[st.base as usize..st.lim as usize].copy_from_slice(&payload);
        Ok(())
    }

    // sequence layout:
    // [len 32 | reserved 32][payload...]
    // - len is stored in an 8-byte slot
    // - only the low 32 bits of len are currently interpreted
    // - payload begins immediately after the len slot: base + WORD_SZ
    // - sequence node base is WORD_SZ-aligned
    // - STR stores a trailing NUL on wire, but len excludes that terminator
    fn _seq(&mut self, node: &SequenceNode, st: &mut EncodingState) -> Result<(), MarshalError> {
        let type_ = node.type_;
        let Some(val) = &node.val else { return Ok(()) };

        st.base = ALIGN(st.base, WORD_SZ)?;
        let payload_start = st.base + WORD_SZ;

        let len: u32; // type-specific length
        let payload_sz: u32; // byte length

        match type_ {
            SequenceType::U8_ARRAY
            | SequenceType::I8_ARRAY
            | SequenceType::U16_ARRAY
            | SequenceType::I16_ARRAY
            | SequenceType::U32_ARRAY
            | SequenceType::I32_ARRAY
            | SequenceType::U64_ARRAY
            | SequenceType::I64_ARRAY
            | SequenceType::F32_ARRAY
            | SequenceType::F64_ARRAY => {
                let SequenceVal::Array(arr) = val else { return Err(bad_type()) };
                payload_sz = arr.bytes.len() as u32;
                len = payload_sz / ELEM_SZ(type_) as u32;

                if payload_sz as i64 > self.max_cap as i64 - payload_start as i64 {
                    return Err(MarshalError::new(Errno::EOVERFLOW));
                }
                st.lim = payload_start + ALIGN(payload_sz, WORD_SZ)?;
                self._ensure(st.lim)?;
                let at = payload_start as usize;
                self.buf[at..at + payload_sz as usize].copy_from_slice(&arr.bytes);
            }

            SequenceType::BITSET => {
                let SequenceVal::Array(arr) = val else { return Err(bad_type()) };
                if arr.type_ != SequenceType::BITSET {
                    return Err(bad_type());
                }
                let arr = &arr.bytes;
                len = arr.len() as u32;
                payload_sz = ((len as u64 + 7) >> 3) as u32;

                if payload_sz as i64 > self.max_cap as i64 - payload_start as i64 {
                    return Err(MarshalError::new(Errno::EOVERFLOW));
                }
                st.lim = payload_start + ALIGN(payload_sz, WORD_SZ)?;
                self._ensure(st.lim)?;

                // bit packing: bit i -> byte[i >> 3], bit position (i & 7), LSB-first within each byte
                self.buf[payload_start as usize..(payload_start + payload_sz) as usize].fill(0);
                for i in 0..len as usize {
                    if arr[i] != 0 {
                        self.buf[payload_start as usize + (i >> 3)] |= 1 << (i & 7);
                    }
                }
            }

            // A string's room is judged in UTF-16 code units, four bytes each,
            // before it is encoded — the worst case of UTF-8 per unit — and a
            // string that does not fit that bound at the cap is refused. Which
            // strings a cap refuses is part of what a packet is, so the bound
            // is counted in units on both ends even though the bytes are
            // known here up front.
            SequenceType::STR => {
                let SequenceVal::Str(s) = val else { return Err(bad_type()) };
                let units = s.encode_utf16().count() as u32;
                let min_needed = units;

                // A payload that already starts past the cap has negative room;
                // wrapping it to a large u32 is what makes the test fail then.
                if units > (self.max_cap.wrapping_sub(payload_start) >> 2) {
                    return Err(MarshalError::new(Errno::EOVERFLOW));
                }
                let max_needed = units * 4;

                // Signed: the payload may start past the buffer's end, which
                // the growth below is for.
                if self.buf.len() as i64 - (payload_start as i64) < min_needed as i64 {
                    self._ensure(payload_start + min_needed)?;
                }

                // The tail holds all of the string's UTF-8, or the worst case is
                // reserved: the growth steps are part of where the cap refuses.
                let data = s.as_bytes();
                if data.len() as i64 > self.buf.len() as i64 - payload_start as i64 {
                    self._ensure(payload_start + max_needed)?;
                }

                len = data.len() as u32; // UTF-8 byte length, excluding trailing NUL
                payload_sz = len + 1; // wire payload includes trailing NUL for C-friendly reads

                self._ensure(payload_start + payload_sz)?;
                let at = payload_start as usize;
                self.buf[at..at + len as usize].copy_from_slice(data);
                self.buf[at + len as usize] = 0;
                st.lim = payload_start + ALIGN(payload_sz, WORD_SZ)?;
                self._ensure(st.lim)?;
            }
        }

        // [len, ...payload]
        SET_WORD(&mut self.buf, st.base, len, RESERVED);
        Ok(())
    }
}

fn bad_type() -> MarshalError {
    MarshalError::new(Errno::EBADMSG)
}

/// A fresh encoder: one is made where one is used, and holds its buffer
/// for that use.
pub fn encoder() -> TreeEncoder {
    TreeEncoder::new()
}

// ============ In-place writer ============

/// The root branch's base in a packet.
const ROOT: u32 = HDR_SZ;

const fn align(n: u32, unit: u32) -> u32 {
    (n + unit - 1) & !(unit - 1)
}

/// A packet of one root branch, written straight into bytes the caller holds
/// — a ring slot's payload — in the layout `TreeEncoder` gives the same
/// children, byte for byte, with no tree built first and nothing allocated.
/// A child may itself be a branch (`branch`). A branch's children are
/// declared up front, since its table precedes them. A child that does not
/// fit spoils the packet and `finish` refuses it; declaring one count and
/// writing another is the caller's bug, and panics.
pub struct PacketWriter<'a> {
    buf: &'a mut [u8],
    /// The branch being written: where it starts, from the packet's start,
    /// how many children it declared and how many it has been given.
    base: u32,
    count: u32,
    written: u32,
    /// The end of what is written, from the packet's start.
    lim: u32,
    fits: bool,
}

impl<'a> PacketWriter<'a> {
    pub fn new(buf: &'a mut [u8], count: u32) -> PacketWriter<'a> {
        let lim = ROOT + WORD_SZ + count * WORD_SZ;
        let fits = lim as usize <= buf.len();
        let mut w = PacketWriter {
            buf,
            base: ROOT,
            count,
            written: 0,
            lim,
            fits,
        };
        if fits {
            w.buf[..lim as usize].fill(0);
            w.word(0, MAGIC, RESERVED);
            w.word(ROOT, count, RESERVED);
        }
        w
    }

    fn word(&mut self, at: u32, lo: u32, hi: u32) {
        let at = at as usize;
        self.buf[at..at + 4].copy_from_slice(&lo.to_le_bytes());
        self.buf[at + 4..at + 8].copy_from_slice(&hi.to_le_bytes());
    }

    /// `bytes` past what is written, aligned to `unit`, the gap and the room
    /// zeroed: where they start, or none when they do not fit.
    fn reserve(&mut self, unit: u32, bytes: u32) -> Option<usize> {
        if !self.fits {
            return None;
        }
        let base = align(self.lim, unit);
        match base.checked_add(bytes) {
            Some(end) if end as usize <= self.buf.len() => {
                self.buf[self.lim as usize..end as usize].fill(0);
                self.lim = end;
                Some(base as usize)
            }
            _ => {
                self.fits = false;
                None
            }
        }
    }

    /// The child's table entry: its offset from the branch, 0 for an empty one.
    fn entry(&mut self, base: Option<usize>, type_: u8) -> &mut Self {
        crate::ensure!(self.written < self.count, ENOSPC, self.count);
        if self.fits {
            let rel_off = base.map_or(0, |b| b as u32 - self.base);
            self.word(self.base + WORD_SZ + self.written * WORD_SZ, rel_off, type_ as u32);
        }
        self.written += 1;
        self
    }

    fn scalar<const N: usize>(&mut self, type_: ScalarType, bytes: Option<[u8; N]>) -> &mut Self {
        let base = bytes.and_then(|b| {
            let at = self.reserve(N as u32, N as u32)?;
            self.buf[at..at + N].copy_from_slice(&b);
            Some(at)
        });
        self.entry(base, type_ as u8)
    }

    /// A sequence child: `[len | 0]` then `bytes` of payload, to the word.
    fn seq(&mut self, type_: SequenceType, len: u32, bytes: u32, fill: impl FnOnce(&mut [u8])) -> &mut Self {
        let at = self.reserve(WORD_SZ, WORD_SZ + align(bytes, WORD_SZ)).inspect(|&at| {
            self.word(at as u32, len, RESERVED);
            fill(&mut self.buf[at + WORD_SZ as usize..at + (WORD_SZ + bytes) as usize]);
        });
        self.entry(at, type_ as u8)
    }

    pub fn u8(&mut self, v: Option<u8>) -> &mut Self {
        self.scalar(ScalarType::U8, v.map(u8::to_le_bytes))
    }

    pub fn i32(&mut self, v: Option<i32>) -> &mut Self {
        self.scalar(ScalarType::I32, v.map(i32::to_le_bytes))
    }

    pub fn u32(&mut self, v: Option<u32>) -> &mut Self {
        self.scalar(ScalarType::U32, v.map(u32::to_le_bytes))
    }

    pub fn u64(&mut self, v: Option<u64>) -> &mut Self {
        self.scalar(ScalarType::U64, v.map(u64::to_le_bytes))
    }

    pub fn f32(&mut self, v: Option<f32>) -> &mut Self {
        self.scalar(ScalarType::F32, v.map(f32::to_le_bytes))
    }

    pub fn f64(&mut self, v: Option<f64>) -> &mut Self {
        self.scalar(ScalarType::F64, v.map(f64::to_le_bytes))
    }

    pub fn bool(&mut self, v: Option<bool>) -> &mut Self {
        self.scalar(ScalarType::BOOL, v.map(|b| [b as u8]))
    }

    /// UTF-8 with the trailing NUL the wire carries, which `len` excludes.
    pub fn str(&mut self, s: Option<&str>) -> &mut Self {
        match s {
            Some(s) => self.seq(SequenceType::STR, s.len() as u32, s.len() as u32 + 1, |out| {
                out[..s.len()].copy_from_slice(s.as_bytes());
            }),
            None => self.entry(None, SequenceType::STR as u8),
        }
    }

    /// What `v` displays as, written straight into the packet as a string:
    /// for text that lies in no string to copy from.
    pub fn str_display(&mut self, v: impl core::fmt::Display) -> &mut Self {
        struct Room<'b> {
            buf: &'b mut [u8],
            len: usize,
        }
        impl core::fmt::Write for Room<'_> {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                let end = self.len + s.len();
                let out = self.buf.get_mut(self.len..end).ok_or(core::fmt::Error)?;
                out.copy_from_slice(s.as_bytes());
                self.len = end;
                Ok(())
            }
        }
        let base = align(self.lim, WORD_SZ) as usize;
        let payload = base + WORD_SZ as usize;
        if !self.fits || payload > self.buf.len() {
            self.fits = false;
            return self.entry(None, SequenceType::STR as u8);
        }
        let mut room = Room {
            buf: &mut self.buf[payload..],
            len: 0,
        };
        let written = core::fmt::Write::write_fmt(&mut room, format_args!("{v}")).map(|_| room.len);
        let end = written
            .ok()
            .map(|len| (len, align((payload + len + 1) as u32, WORD_SZ) as usize))
            .filter(|&(_, end)| end <= self.buf.len());
        let Some((len, end)) = end else {
            self.fits = false;
            return self.entry(None, SequenceType::STR as u8);
        };
        self.buf[self.lim as usize..payload].fill(0);
        self.buf[payload + len..end].fill(0);
        self.word(base as u32, len as u32, RESERVED);
        self.lim = end as u32;
        self.entry(Some(base), SequenceType::STR as u8)
    }

    pub fn u8_array(&mut self, a: Option<&[u8]>) -> &mut Self {
        match a {
            Some(a) => self.seq(SequenceType::U8_ARRAY, a.len() as u32, a.len() as u32, |out| out.copy_from_slice(a)),
            None => self.entry(None, SequenceType::U8_ARRAY as u8),
        }
    }

    /// A number array of `len` elements, element `i` being `value(i)`'s bytes.
    fn array_with<const N: usize>(&mut self, type_: SequenceType, len: u32, mut value: impl FnMut(u32) -> [u8; N]) -> &mut Self {
        self.seq(type_, len, len * N as u32, |out| {
            for (i, o) in out.as_chunks_mut::<N>().0.iter_mut().enumerate() {
                *o = value(i as u32);
            }
        })
    }

    pub fn u32_array(&mut self, a: Option<&[u32]>) -> &mut Self {
        match a {
            Some(a) => self.array_with(SequenceType::U32_ARRAY, a.len() as u32, |i| a[i as usize].to_le_bytes()),
            None => self.entry(None, SequenceType::U32_ARRAY as u8),
        }
    }

    pub fn u64_array(&mut self, a: Option<&[u64]>) -> &mut Self {
        match a {
            Some(a) => self.array_with(SequenceType::U64_ARRAY, a.len() as u32, |i| a[i as usize].to_le_bytes()),
            None => self.entry(None, SequenceType::U64_ARRAY as u8),
        }
    }

    pub fn f64_array(&mut self, a: Option<&[f64]>) -> &mut Self {
        match a {
            Some(a) => self.array_with(SequenceType::F64_ARRAY, a.len() as u32, |i| a[i as usize].to_le_bytes()),
            None => self.entry(None, SequenceType::F64_ARRAY as u8),
        }
    }

    /// A u32 array of `len` elements, written from `value` as it goes: for
    /// words that lie in no lane to copy from.
    pub fn u32_array_with(&mut self, len: u32, mut value: impl FnMut(u32) -> u32) -> &mut Self {
        self.array_with(SequenceType::U32_ARRAY, len, |i| value(i).to_le_bytes())
    }

    /// A u64 array of `len` elements, written from `value` as it goes.
    pub fn u64_array_with(&mut self, len: u32, mut value: impl FnMut(u32) -> u64) -> &mut Self {
        self.array_with(SequenceType::U64_ARRAY, len, |i| value(i).to_le_bytes())
    }

    /// A branch of `count` children, which `fill` writes as it would the
    /// packet's own.
    pub fn branch(&mut self, count: u32, fill: impl FnOnce(&mut Self)) -> &mut Self {
        let at = self.reserve(WORD_SZ, WORD_SZ + count * WORD_SZ);
        if let Some(at) = at {
            self.word(at as u32, count, RESERVED);
        }
        // A branch that did not fit has spoiled the packet: its children are
        // still counted, against a base nothing is written at.
        let outer = (self.base, self.count, self.written);
        (self.base, self.count, self.written) = (at.map_or(ROOT, |at| at as u32), count, 0);
        fill(self);
        crate::ensure!(self.written == self.count, ENOTRECOVERABLE, self.count, self.written);
        (self.base, self.count, self.written) = outer;
        self.entry(at, SpecialType::BRANCH as u8)
    }

    /// A branch that is not there, its kind kept.
    pub fn no_branch(&mut self) -> &mut Self {
        self.entry(None, SpecialType::BRANCH as u8)
    }

    /// The packet's length, header included and to the word; none when a
    /// child did not fit.
    pub fn finish(mut self) -> Option<u32> {
        crate::ensure!(self.written == self.count, ENOTRECOVERABLE, self.count, self.written);
        if !self.fits {
            return None;
        }
        let end = align(self.lim, WORD_SZ);
        if end as usize > self.buf.len() {
            return None;
        }
        self.buf[self.lim as usize..end as usize].fill(0);
        self.word(WORD_SZ, end - HDR_SZ, SpecialType::BRANCH as u32);
        Some(end)
    }
}

#[cfg(test)]
#[path = "encode.test.rs"]
mod test;
