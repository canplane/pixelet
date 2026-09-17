// xpute-core/io/marshal/tree/encode.rs

use crate::kit::array::{Bytes, Kind, TypedArray, U8Array};
use crate::kit::encoding;
use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

use super::spec::{
    AlignUnit, BranchNode, EncodingState, GraftNode, Node, NodeType, Numeric, ScalarNode, ScalarType, SequenceNode, SequenceType, SequenceVal, SpecialType, ALIGN, ALIGN_SZ, DESC_TYPE_MASK,
    DESC_TYPE_SHAMT, GET_WORD, HDR_SZ, MAGIC, MAX_PKT_SZ, NODE_IS_BRANCH, NODE_IS_GRAFT, NODE_IS_LEAF, NODE_IS_NIL, NODE_IS_SEQ, NONE, RESERVED, SET_WORD, WORD_SZ,
};
use super::view::NodeView;

/// Scratchpad for 64-bit word operations to avoid allocation.
/// [lo32, hi32]
const WORD_REG: [u32; 2] = [NONE, NONE];

fn field_set(u: u32, shamt: u32, mask: u32, v: u32) -> u32 {
    (u & !(mask << shamt)) | ((v & mask) << shamt)
}
fn field_get(u: u32, shamt: u32, mask: u32) -> u32 {
    (u >> shamt) & mask
}

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

    pub buf: U8Array,
}

impl Default for TreeEncoder {
    fn default() -> Self {
        TreeEncoder::new()
    }
}

impl TreeEncoder {
    pub fn new() -> TreeEncoder {
        TreeEncoder {
            max_cap: MAX_PKT_SZ,
            buf: TypedArray::new(0),
        }
    }

    // ---- Buffer ----

    fn _ensure(&mut self, new_cap: u32) -> Result<u32, MarshalError> {
        let cap = self.buf.byte_length() as u32;
        if new_cap <= cap {
            return Ok(cap);
        }

        let new_cap = (cap * 2).max(new_cap);
        if new_cap > self.max_cap {
            return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("required packet size exceeds max cap ({})", self.max_cap)), None));
        }

        let new_buf: U8Array = TypedArray::new(new_cap as usize);
        new_buf.set(&self.buf);

        self.buf = new_buf;
        Ok(self.buf.byte_length() as u32)
    }

    // ---- Entry ----

    pub fn encode<V: NodeView>(&mut self, view: &V, opts: TreeEncoderOptions) -> Result<U8Array, MarshalError> {
        let node = view.node();

        let _init_cap = opts.init_cap.unwrap_or(1 << 10);
        let max_cap = opts.max_cap.unwrap_or(MAX_PKT_SZ);
        if !(HDR_SZ..=MAX_PKT_SZ).contains(&max_cap) {
            return Err(MarshalError::new(Errno::EINVAL, Some(&format!("bad max cap {max_cap} (expected {HDR_SZ}..{MAX_PKT_SZ})")), None));
        }
        self.max_cap = max_cap;

        let init_cap = HDR_SZ.max(_init_cap.min(self.max_cap));
        self.buf = TypedArray::new(init_cap as usize);

        SET_WORD(&self.buf, 0, MAGIC, RESERVED); // word0

        // packet payload
        let mut st = EncodingState {
            base: HDR_SZ,
            lim: HDR_SZ,
            type_: node.type_(),
        };
        self._node(node, &mut st)?;

        // root node kind is preserved even when payload_sz == 0
        let payload_sz = st.lim - HDR_SZ;
        let desc = field_set(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, node.type_() as u32);
        SET_WORD(&self.buf, WORD_SZ, payload_sz, desc); // word1

        let pkt_nbyte = ALIGN(HDR_SZ + payload_sz, WORD_SZ)?;
        self._ensure(pkt_nbyte)?;
        Ok(self.buf.subarray(0, pkt_nbyte as usize))
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
        Err(MarshalError::new(Errno::EBADMSG, Some(&format!("unknown type 0x{:x} at {}", node.type_(), st.base)), None))
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

        if (pkt.byte_length() as u32) < HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG, Some("bad graft packet: truncated header"), None));
        }
        let hdr_view: Bytes = TypedArray::over(pkt.buffer().clone(), pkt.byte_offset(), HDR_SZ as usize);

        // packet header fallback
        let mut reg = WORD_REG;
        let [magic, _] = GET_WORD(&hdr_view, 0, &mut reg); // word0
        if magic != MAGIC {
            return Err(MarshalError::new(Errno::EBADMSG, Some("bad graft packet: magic mismatch"), None));
        }

        let [payload_sz, desc] = GET_WORD(&hdr_view, WORD_SZ, &mut reg); // word1
        if payload_sz > pkt.byte_length() as u32 - HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG, Some("bad graft packet: truncated payload"), None));
        }
        st.type_ = field_get(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType; // hi

        if payload_sz == 0 {
            return Ok(());
        }

        let align_sz: AlignUnit = ALIGN_SZ(st.type_)?;
        if !payload_sz.is_multiple_of(align_sz) {
            return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("bad graft payload size: {payload_sz}")), None));
        }

        st.base = ALIGN(st.base, align_sz)?;

        if payload_sz as i64 > self.max_cap as i64 - st.base as i64 {
            return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("required packet size exceeds max cap ({})", self.max_cap)), None));
        }
        st.lim = st.base + payload_sz;
        self._ensure(st.lim)?;
        self.buf.set_at(&pkt.subarray(HDR_SZ as usize, (HDR_SZ + payload_sz) as usize), st.base as usize);
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
            return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("required packet size exceeds max cap ({})", self.max_cap)), None));
        }

        st.lim = table_start + len * WORD_SZ;
        self._ensure(st.lim)?; // [len | child entries...]

        SET_WORD(&self.buf, st.base, len, RESERVED);

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
            SET_WORD(&self.buf, table_off, rel_off, field_set(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, child_st.type_ as u32));

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

        // A DataView setter refuses the other family (a bigint to setUint8, a
        // number to setBigUint64) with a TypeError; the kinds are not mixed.
        let mismatch = || MarshalError::new(Errno::EINVAL, Some(&format!("scalar 0x{:x} given the other numeric family at {}", type_ as u8, st.base)), None);
        let number = || match val {
            Numeric::Number(v) => Ok(v),
            Numeric::BigInt(_) => Err(mismatch()),
        };
        let bigint = || match val {
            Numeric::BigInt(v) => Ok(v),
            Numeric::Number(_) => Err(mismatch()),
        };
        // A typed-array store's conversion: the fraction dropped, modulo 2^32.
        let int = |v: f64| if v.is_finite() { (v.trunc() % 4_294_967_296.0) as i64 } else { 0 };

        let (elem_sz, payload): (AlignUnit, Vec<u8>) = match type_ {
            ScalarType::U8 => (1, (int(number()?) as u8).to_le_bytes().to_vec()),
            ScalarType::I8 => (1, (int(number()?) as i8).to_le_bytes().to_vec()),
            ScalarType::U16 => (2, (int(number()?) as u16).to_le_bytes().to_vec()),
            ScalarType::I16 => (2, (int(number()?) as i16).to_le_bytes().to_vec()),
            ScalarType::U32 => (4, (int(number()?) as u32).to_le_bytes().to_vec()),
            ScalarType::I32 => (4, (int(number()?) as i32).to_le_bytes().to_vec()),
            ScalarType::U64 => (8, (bigint()? as u64).to_le_bytes().to_vec()),
            ScalarType::I64 => (8, (bigint()? as i64).to_le_bytes().to_vec()),
            ScalarType::F32 => (4, (number()? as f32).to_le_bytes().to_vec()),
            ScalarType::F64 => (8, number()?.to_le_bytes().to_vec()),

            // BOOL(val): truthiness, of either family — NaN and 0n are false.
            ScalarType::BOOL => (
                1,
                vec![match val {
                    Numeric::Number(v) => (v != 0.0 && !v.is_nan()) as u8,
                    Numeric::BigInt(v) => (v != 0) as u8,
                }],
            ),
        };

        st.base = ALIGN(st.base, elem_sz)?;

        st.lim = st.base + elem_sz;
        self._ensure(st.lim)?;
        self.buf.as_mut_slice()[st.base as usize..st.lim as usize].copy_from_slice(&payload);
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
                let SequenceVal::Array(arr) = val else { return Err(bad_type(type_ as u8, st.base)) };
                len = arr.length() as u32;
                payload_sz = len * arr.kind.bytes_per_element() as u32;

                if payload_sz as i64 > self.max_cap as i64 - payload_start as i64 {
                    return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("required packet size exceeds max cap ({})", self.max_cap)), None));
                }
                st.lim = payload_start + ALIGN(payload_sz, WORD_SZ)?;
                self._ensure(st.lim)?;
                self.buf.set_at(&crate::kit::array::to_bytes(arr), payload_start as usize);
            }

            SequenceType::BITSET => {
                let SequenceVal::Array(arr) = val else { return Err(bad_type(type_ as u8, st.base)) };
                if arr.kind != Kind::U8 {
                    return Err(bad_type(type_ as u8, st.base));
                }
                let arr = TypedArray::<u8>::of(arr).unwrap();
                len = arr.length() as u32;
                payload_sz = ((len as u64 + 7) >> 3) as u32;

                if payload_sz as i64 > self.max_cap as i64 - payload_start as i64 {
                    return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("required packet size exceeds max cap ({})", self.max_cap)), None));
                }
                st.lim = payload_start + ALIGN(payload_sz, WORD_SZ)?;
                self._ensure(st.lim)?;

                // bit packing: bit i -> byte[i >> 3], bit position (i & 7), LSB-first within each byte
                let buf = self.buf.as_mut_slice();
                buf[payload_start as usize..(payload_start + payload_sz) as usize].fill(0);
                for i in 0..len as usize {
                    if arr[i] != 0 {
                        buf[payload_start as usize + (i >> 3)] |= 1 << (i & 7);
                    }
                }
            }

            // optimistic UTF-8 encode:
            // - first try encodeInto() into the currently available tail
            // - if it does not fit, reserve the worst-case bound (s.length * 4)
            //   and retry once
            // - avoids an intermediate encode() allocation while keeping retry bounded
            //
            // `s.length` is UTF-16 code units, and the bound and the growth
            // follow it as the TypeScript's do: they decide when a string is
            // refused at a cap, which is part of what a packet is.
            SequenceType::STR => {
                let SequenceVal::Str(s) = val else { return Err(bad_type(type_ as u8, st.base)) };
                let units = s.encode_utf16().count() as u32;
                let min_needed = units;

                // `>>>` there: a negative room reads as its u32, which wrapping_sub is.
                if units > (self.max_cap.wrapping_sub(payload_start) >> 2) {
                    return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("required packet size exceeds max cap ({})", self.max_cap)), None));
                }
                let max_needed = units * 4;

                // Signed, as the TypeScript's doubles are: the payload may start
                // past the buffer's end, which the growth below is for.
                if self.buf.byte_length() as i64 - (payload_start as i64) < min_needed as i64 {
                    self._ensure(payload_start + min_needed)?;
                }

                // encodeInto writes every character only when the tail holds
                // all of the string's UTF-8; otherwise the worst case is reserved.
                let data = encoding::te.encode(s);
                if data.length() as i64 > self.buf.byte_length() as i64 - payload_start as i64 {
                    self._ensure(payload_start + max_needed)?;
                }

                len = data.length() as u32; // UTF-8 byte length, excluding trailing NUL
                payload_sz = len + 1; // wire payload includes trailing NUL for C-friendly reads

                self._ensure(payload_start + payload_sz)?;
                self.buf.set_at(&data, payload_start as usize);
                self.buf.put((payload_start + len) as usize, 0);
                st.lim = payload_start + ALIGN(payload_sz, WORD_SZ)?;
                self._ensure(st.lim)?;
            }
        }

        // [len, ...payload]
        SET_WORD(&self.buf, st.base, len, RESERVED);
        Ok(())
    }
}

fn bad_type(type_: u8, base: u32) -> MarshalError {
    MarshalError::new(Errno::EBADMSG, Some(&format!("unknown type 0x{type_:x} at {base}")), None)
}

/// The TypeScript's shared `encoder` instance: an encoder is made where one
/// is used, since a `const` here would be a fresh one each use.
pub fn encoder() -> TreeEncoder {
    TreeEncoder::new()
}

// ============ In-place writer ============

/// The root branch's base in a packet.
const ROOT: u32 = HDR_SZ;

const fn align(n: u32, unit: u32) -> u32 {
    (n + unit - 1) & !(unit - 1)
}

/// A packet of one root branch of leaves, written straight into bytes the
/// caller holds — a ring slot's payload — in the layout `TreeEncoder` gives the same
/// children, byte for byte, with no tree built first and nothing allocated.
/// The children are declared up front, since the branch's table precedes
/// them. A child that does not fit spoils the packet and `finish` refuses it;
/// declaring one count and writing another is the caller's bug, and panics.
pub struct PacketWriter<'a> {
    buf: &'a mut [u8],
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
        let mut w = PacketWriter { buf, count, written: 0, lim, fits };
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
        assert!(self.written < self.count, "a packet writer given more than the {} children it declared", self.count);
        if self.fits {
            let rel_off = base.map_or(0, |b| b as u32 - ROOT);
            self.word(ROOT + WORD_SZ + self.written * WORD_SZ, rel_off, type_ as u32);
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
            for (i, o) in out.chunks_exact_mut(N).enumerate() {
                o.copy_from_slice(&value(i as u32));
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

    /// The packet's length, header included and to the word; none when a
    /// child did not fit.
    pub fn finish(mut self) -> Option<u32> {
        assert_eq!(self.written, self.count, "a packet writer declared {} children and was given {}", self.count, self.written);
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
