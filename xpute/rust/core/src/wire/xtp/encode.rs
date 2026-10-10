// xpute-core/wire/xtp/encode.rs

use crate::abi::word::{field_get32, field_set32};
use crate::status::errno::Errno;
use crate::status::error::MarshalError;

use super::spec::{
    AlignUnit, BranchNode, EncodingState, GraftNode, Node, NodeType, Numeric, ScalarNode, ScalarType, SequenceNode, SequenceType, SequenceVal, SpecialType, ALIGN, ALIGN_SZ, DESC_TYPE_MASK,
    DESC_TYPE_SHAMT, ELEM_SZ, GET_WORD, HDR_SZ, MAGIC, MAX_PKT_SZ, NODE_IS_BRANCH, NODE_IS_GRAFT, NODE_IS_LEAF, NODE_IS_NIL, NODE_IS_SEQ, NONE, RESERVED, SET_WORD, WORD_SZ,
};
use super::view::NodeView;

const WORD_REG: [u32; 2] = [NONE, NONE];

#[derive(Default)]
pub struct TreeEncoderOptions {
    pub init_cap: Option<u32>,
    /// A hard limit on the packet's size.
    pub max_cap: Option<u32>,
}

/// Trusts the view layer for a node's shape; checks only the packet's bounds.
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

    fn ensure(&mut self, new_cap: u32) -> Result<u32, MarshalError> {
        let cap = self.buf.len() as u32;
        if new_cap <= cap {
            return Ok(cap);
        }

        // Doubling stops at the cap, so the cap refuses only a packet that does
        // not fit, never a growth step.
        if new_cap > self.max_cap {
            return Err(MarshalError::new(Errno::EOVERFLOW));
        }
        let new_cap = cap.saturating_mul(2).max(new_cap).min(self.max_cap);

        self.buf.resize(new_cap as usize, 0);
        Ok(self.buf.len() as u32)
    }

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

        SET_WORD(&mut self.buf, 0, MAGIC, RESERVED);

        let mut st = EncodingState {
            base: HDR_SZ,
            lim: HDR_SZ,
            type_: node.type_(),
        };
        self.node(node, &mut st)?;

        // A grafted root's type is the one the graft read.
        let payload_sz = st.lim - HDR_SZ;
        let desc = field_set32(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, st.type_ as u32);
        SET_WORD(&mut self.buf, WORD_SZ, payload_sz, desc);

        let pkt_nbyte = ALIGN(HDR_SZ + payload_sz, WORD_SZ)?;
        self.ensure(pkt_nbyte)?;
        Ok(self.buf[..pkt_nbyte as usize].to_vec())
    }

    fn node(&mut self, node: &Node, st: &mut EncodingState) -> Result<(), MarshalError> {
        if NODE_IS_LEAF(node) {
            if NODE_IS_SEQ(node) {
                let Node::Sequence(n) = node else { unreachable!() };
                return self.seq_of(n, st);
            }
            let Node::Scalar(n) = node else { unreachable!() };
            return self.scalar_of(n, st);
        }

        if NODE_IS_BRANCH(node) {
            let Node::Branch(n) = node else { unreachable!() };
            return self.branch_of(n, st);
        }
        if NODE_IS_GRAFT(node) {
            let Node::Graft(n) = node else { unreachable!() };
            return self.graft(n, st);
        }
        if NODE_IS_NIL(node) {
            return Ok(());
        }
        Err(MarshalError::new(Errno::EBADMSG))
    }

    // Copies the graft's root payload without its header, aligned for its root
    // type, so it lays out exactly as inline encoding would.
    fn graft(&mut self, node: &GraftNode, st: &mut EncodingState) -> Result<(), MarshalError> {
        let pkt = &node.val;

        if (pkt.len() as u32) < HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let hdr_view = &pkt[..HDR_SZ as usize];

        let mut reg = WORD_REG;
        let [magic, _] = GET_WORD(hdr_view, 0, &mut reg);
        if magic != MAGIC {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        let [payload_sz, desc] = GET_WORD(hdr_view, WORD_SZ, &mut reg);
        if payload_sz > pkt.len() as u32 - HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        st.type_ = field_get32(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType;

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
        self.ensure(st.lim)?;
        let at = st.base as usize;
        self.buf[at..at + payload_sz as usize].copy_from_slice(&pkt[HDR_SZ as usize..(HDR_SZ + payload_sz) as usize]);
        Ok(())
    }

    fn branch_of(&mut self, node: &BranchNode, st: &mut EncodingState) -> Result<(), MarshalError> {
        let Some(children) = &node.val else { return Ok(()) };

        st.base = ALIGN(st.base, WORD_SZ)?;
        let table_start = st.base + WORD_SZ;

        let len = children.len() as u32;
        if len > (self.max_cap >> 3) {
            return Err(MarshalError::new(Errno::EOVERFLOW));
        }

        st.lim = table_start + len * WORD_SZ;
        self.ensure(st.lim)?;

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

            self.node(child, &mut child_st)?;
            let rel_off = if child_st.base == child_st.lim { 0 } else { child_st.base - st.base };

            SET_WORD(&mut self.buf, table_off, rel_off, field_set32(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, child_st.type_ as u32));

            table_off += WORD_SZ;
        }
        st.lim = ALIGN(child_st.lim, WORD_SZ)?;
        self.ensure(st.lim)?;
        Ok(())
    }

    fn scalar_of(&mut self, node: &ScalarNode, st: &mut EncodingState) -> Result<(), MarshalError> {
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
        self.ensure(st.lim)?;
        self.buf[st.base as usize..st.lim as usize].copy_from_slice(&payload);
        Ok(())
    }

    fn seq_of(&mut self, node: &SequenceNode, st: &mut EncodingState) -> Result<(), MarshalError> {
        let type_ = node.type_;
        let Some(val) = &node.val else { return Ok(()) };

        st.base = ALIGN(st.base, WORD_SZ)?;
        let payload_start = st.base + WORD_SZ;

        let len: u32;
        let payload_sz: u32;

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
                self.ensure(st.lim)?;
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
                self.ensure(st.lim)?;

                // LSB-first within each byte.
                self.buf[payload_start as usize..(payload_start + payload_sz) as usize].fill(0);
                for i in 0..len as usize {
                    if arr[i] != 0 {
                        self.buf[payload_start as usize + (i >> 3)] |= 1 << (i & 7);
                    }
                }
            }

            // Room is judged in UTF-16 units at four bytes each, as the other
            // language's encoder must, so both refuse the same strings at a cap.
            SequenceType::STR => {
                let SequenceVal::Str(s) = val else { return Err(bad_type()) };
                let units = s.encode_utf16().count() as u32;
                let min_needed = units;

                // Past the cap the room wraps to a large u32, and the test fails.
                if units > (self.max_cap.wrapping_sub(payload_start) >> 2) {
                    return Err(MarshalError::new(Errno::EOVERFLOW));
                }
                let max_needed = units * 4;

                // Signed: the payload may start past the buffer's end.
                if self.buf.len() as i64 - (payload_start as i64) < min_needed as i64 {
                    self.ensure(payload_start + min_needed)?;
                }

                // The growth steps are part of where the cap refuses.
                let data = s.as_bytes();
                if data.len() as i64 > self.buf.len() as i64 - payload_start as i64 {
                    self.ensure(payload_start + max_needed)?;
                }

                len = data.len() as u32;
                payload_sz = len + 1;

                self.ensure(payload_start + payload_sz)?;
                let at = payload_start as usize;
                self.buf[at..at + len as usize].copy_from_slice(data);
                self.buf[at + len as usize] = 0;
                st.lim = payload_start + ALIGN(payload_sz, WORD_SZ)?;
                self.ensure(st.lim)?;
            }

            SequenceType::STRS => {
                let SequenceVal::Strs(items) = val else { return Err(bad_type()) };
                len = items.len() as u32;
                self.strs_of(items, payload_start, st)?;
            }
        }

        SET_WORD(&mut self.buf, st.base, len, RESERVED);
        Ok(())
    }
}

fn strs_payload<S: AsRef<str>>(items: &[S], out: &mut [u8]) {
    let lane = 4 * (items.len() + 1);
    let mut off = 0u32;
    out[0..4].copy_from_slice(&0u32.to_le_bytes());
    for (i, s) in items.iter().enumerate() {
        let s = s.as_ref().as_bytes();
        out[lane + off as usize..lane + off as usize + s.len()].copy_from_slice(s);
        off += s.len() as u32;
        out[4 * (i + 1)..4 * (i + 2)].copy_from_slice(&off.to_le_bytes());
    }
}

fn strs_size<S: AsRef<str>>(items: &[S]) -> u64 {
    4 * (items.len() as u64 + 1) + items.iter().map(|s| s.as_ref().len() as u64).sum::<u64>()
}

impl TreeEncoder {
    fn strs_of(&mut self, items: &[String], payload_start: u32, st: &mut EncodingState) -> Result<(), MarshalError> {
        let size = strs_size(items);
        if size as i64 > self.max_cap as i64 - payload_start as i64 {
            return Err(MarshalError::new(Errno::EOVERFLOW));
        }
        st.lim = payload_start + ALIGN(size as u32, WORD_SZ)?;
        self.ensure(st.lim)?;
        strs_payload(items, &mut self.buf[payload_start as usize..(payload_start as u64 + size) as usize]);
        Ok(())
    }
}

#[track_caller]
fn bad_type() -> MarshalError {
    MarshalError::new(Errno::EBADMSG)
}

pub fn encoder() -> TreeEncoder {
    TreeEncoder::new()
}

const ROOT: u32 = HDR_SZ;

const fn align(n: u32, unit: u32) -> u32 {
    (n + unit - 1) & !(unit - 1)
}

/// Writes a root-branch packet in place, without allocating, byte for byte as
/// `TreeEncoder` would. A branch declares its child count up front; a child
/// that does not fit makes `finish` refuse, and a miscount panics.
pub struct PacketWriter<'a> {
    buf: &'a mut [u8],
    base: u32,
    count: u32,
    written: u32,
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

    /// Refuses exactly what `TreeEncoder` refuses at the same cap.
    pub fn str(&mut self, s: Option<&str>) -> &mut Self {
        match s {
            Some(s) if self.utf16_fits(s) => self.seq(SequenceType::STR, s.len() as u32, s.len() as u32 + 1, |out| {
                out[..s.len()].copy_from_slice(s.as_bytes());
            }),
            Some(_) => {
                self.fits = false;
                self.entry(None, SequenceType::STR as u8)
            }
            None => self.entry(None, SequenceType::STR as u8),
        }
    }

    fn utf16_fits(&self, s: &str) -> bool {
        let payload = align(self.lim, WORD_SZ) as usize + WORD_SZ as usize;
        let room = self.buf.len().saturating_sub(payload);
        s.encode_utf16().count() <= room >> 2
    }

    pub fn strs<S: AsRef<str>>(&mut self, items: Option<&[S]>) -> &mut Self {
        match items {
            Some(items) => match u32::try_from(strs_size(items)) {
                Ok(size) => self.seq(SequenceType::STRS, items.len() as u32, size, |out| strs_payload(items, out)),
                Err(_) => {
                    self.fits = false;
                    self.entry(None, SequenceType::STRS as u8)
                }
            },
            None => self.entry(None, SequenceType::STRS as u8),
        }
    }

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
        let written = core::str::from_utf8(&self.buf[payload..payload + len]).unwrap_or_default();
        if !self.utf16_fits(written) {
            self.fits = false;
            return self.entry(None, SequenceType::STR as u8);
        }
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

    pub fn u32_array_with(&mut self, len: u32, mut value: impl FnMut(u32) -> u32) -> &mut Self {
        self.array_with(SequenceType::U32_ARRAY, len, |i| value(i).to_le_bytes())
    }

    pub fn u64_array_with(&mut self, len: u32, mut value: impl FnMut(u32) -> u64) -> &mut Self {
        self.array_with(SequenceType::U64_ARRAY, len, |i| value(i).to_le_bytes())
    }

    pub fn branch(&mut self, count: u32, fill: impl FnOnce(&mut Self)) -> &mut Self {
        let at = self.reserve(WORD_SZ, WORD_SZ + count * WORD_SZ);
        if let Some(at) = at {
            self.word(at as u32, count, RESERVED);
        }
        // A branch that did not fit still counts its children, against a base
        // nothing is written at.
        let outer = (self.base, self.count, self.written);
        (self.base, self.count, self.written) = (at.map_or(ROOT, |at| at as u32), count, 0);
        fill(self);
        crate::ensure!(self.written == self.count, ENOTRECOVERABLE, self.count, self.written);
        (self.base, self.count, self.written) = outer;
        // A branch ends on a word, as the encoder's does.
        if at.is_some() && self.fits {
            let end = align(self.lim, WORD_SZ);
            if end as usize > self.buf.len() {
                self.fits = false;
            } else {
                self.buf[self.lim as usize..end as usize].fill(0);
                self.lim = end;
            }
        }
        self.entry(at, SpecialType::BRANCH as u8)
    }

    pub fn no_branch(&mut self) -> &mut Self {
        self.entry(None, SpecialType::BRANCH as u8)
    }

    /// The packet's length, or none when a child did not fit.
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
