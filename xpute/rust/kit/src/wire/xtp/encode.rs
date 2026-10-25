// xpute-kit/wire/xtp/encode.rs

use crate::status::errno::Errno;
use crate::status::error::MarshalError;

use super::spec::{
    BranchNode, GraftNode, Node, Numeric, ScalarNode, ScalarType, SequenceNode, SequenceType, SequenceVal, SpecialType, ALIGN_SZ, ELEM_SZ, GET_DESC, GET_LEN, HDR_SZ, MAGIC, MAX_PKT_SZ, RESERVED,
    WORD_SZ,
};
use super::view::NodeView;

pub struct TreeEncoderOptions {
    /// The first buffer's size; it doubles where the tree overflows it.
    pub init_cap: u32,
    /// A hard limit on the packet's size.
    pub max_cap: Option<u32>,
}

/// A tree walked onto `PacketWriter`, the one writer of the layout, as the
/// C++ encoder does (cpp kit wire/xtp/view.hpp `encode`): into a fixed
/// buffer, walked again on one twice as large where it overflowed, up to the
/// cap. Trusts the view layer for a node's shape.
#[derive(Default)]
pub struct TreeEncoder;

impl TreeEncoder {
    pub fn new() -> TreeEncoder {
        TreeEncoder
    }

    /// EINVAL for a cap outside a packet's sizes, EOVERFLOW for a tree past
    /// the cap.
    pub fn encode<'a, V: NodeView<'a>>(&mut self, view: &V, opts: TreeEncoderOptions) -> Result<Vec<u8>, MarshalError> {
        let max_cap = opts.max_cap.unwrap_or(MAX_PKT_SZ);
        if !(HDR_SZ..=MAX_PKT_SZ).contains(&max_cap) {
            return Err(MarshalError::new(Errno::EINVAL));
        }
        let mut cap = HDR_SZ.max(opts.init_cap.min(max_cap));
        loop {
            let mut buf = vec![0u8; cap as usize];
            let mut w = PacketWriter::root(&mut buf);
            walk(&mut w, view.node());
            match w.finish() {
                Ok(n) => {
                    buf.truncate(n as usize);
                    return Ok(buf);
                }
                Err(e) if e.errnum == Errno::EOVERFLOW && cap < max_cap => cap = if cap > max_cap / 2 { max_cap } else { cap * 2 },
                Err(e) => return Err(e),
            }
        }
    }
}

pub fn encoder() -> TreeEncoder {
    TreeEncoder::new()
}

/// A node written as the entry the writer is at; a value its type does not
/// name refuses the packet.
fn walk(w: &mut PacketWriter<'_>, node: &Node) {
    match node {
        Node::Nil(_) => {
            w.nil();
        }
        Node::Scalar(n) => scalar_into(w, n),
        Node::Sequence(n) => seq_into(w, n),
        Node::Branch(BranchNode { val: Some(children) }) => {
            w.branch(children.len() as u32, |b| children.iter().for_each(|c| walk(b, c)));
        }
        Node::Branch(BranchNode { val: None }) => {
            w.no_branch();
        }
        Node::Graft(GraftNode { val }) => {
            w.graft(val);
        }
    }
}

fn scalar_into(w: &mut PacketWriter<'_>, n: &ScalarNode) {
    let Some(v) = n.val else {
        w.entry(None, n.type_ as u8);
        return;
    };
    match (n.type_, v) {
        (ScalarType::U8, Numeric::U8(v)) => w.u8(Some(v)),
        (ScalarType::I8, Numeric::I8(v)) => w.i8(Some(v)),
        (ScalarType::U16, Numeric::U16(v)) => w.u16(Some(v)),
        (ScalarType::I16, Numeric::I16(v)) => w.i16(Some(v)),
        (ScalarType::U32, Numeric::U32(v)) => w.u32(Some(v)),
        (ScalarType::I32, Numeric::I32(v)) => w.i32(Some(v)),
        (ScalarType::U64, Numeric::U64(v)) => w.u64(Some(v)),
        (ScalarType::I64, Numeric::I64(v)) => w.i64(Some(v)),
        (ScalarType::F32, Numeric::F32(v)) => w.f32(Some(v)),
        (ScalarType::F64, Numeric::F64(v)) => w.f64(Some(v)),
        (ScalarType::BOOL, Numeric::U8(v)) => w.bool(Some(v != 0)),
        (t, _) => {
            w.spoil(Errno::EINVAL);
            w.entry(None, t as u8)
        }
    };
}

fn seq_into(w: &mut PacketWriter<'_>, n: &SequenceNode) {
    let type_ = n.type_;
    match (type_, &n.val) {
        (_, None) => {
            w.entry(None, type_ as u8);
        }
        (SequenceType::STR, Some(SequenceVal::Str(s))) => {
            w.str(Some(s));
        }
        (SequenceType::STRS, Some(SequenceVal::Strs(items))) => {
            w.strs(Some(items));
        }
        (SequenceType::BITSET, Some(SequenceVal::Array(a))) if a.type_ == SequenceType::BITSET => {
            w.bitset(Some(&a.bytes));
        }
        (SequenceType::BITSET | SequenceType::STR | SequenceType::STRS, _) | (_, Some(SequenceVal::Str(_) | SequenceVal::Strs(_))) => {
            w.spoil(Errno::EBADMSG);
            w.entry(None, type_ as u8);
        }
        (_, Some(SequenceVal::Array(a))) => {
            w.array_bytes(type_, Some(&a.bytes));
        }
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

const ROOT: u32 = HDR_SZ;

const fn align(n: u32, unit: u32) -> u32 {
    (n + unit - 1) & !(unit - 1)
}

/// The table a writer's entries go in: a branch's, or the root's, a branch
/// of one whose entry is the header.
#[derive(Clone, Copy)]
struct Frame {
    base: u32,
    count: u32,
    written: u32,
    root: bool,
}

/// Writes a packet in place into bytes the caller holds, with no tree and no
/// allocation: the one writer of the layout, which `TreeEncoder` walks its
/// tree onto (cpp kit wire/xtp/encode.hpp). A branch declares its child
/// count up front; a child that does not fit makes `finish` refuse, and a
/// miscount panics.
pub struct PacketWriter<'a> {
    buf: &'a mut [u8],
    frame: Frame,
    root_type: u8,
    lim: u32,
    /// The first reason only.
    error: Option<Errno>,
}

impl<'a> PacketWriter<'a> {
    /// A packet of one root, any node; the buffer's length is the cap, at
    /// most MAX_PKT_SZ.
    pub fn root(buf: &'a mut [u8]) -> PacketWriter<'a> {
        let cap = buf.len().min(MAX_PKT_SZ as usize);
        let mut w = PacketWriter {
            buf: &mut buf[..cap],
            frame: Frame {
                base: 0,
                count: 1,
                written: 0,
                root: true,
            },
            root_type: SpecialType::NIL as u8,
            lim: ROOT,
            error: None,
        };
        match w.buf.get_mut(..ROOT as usize) {
            Some(h) => h.fill(0),
            None => {
                w.spoil(Errno::EINVAL);
            }
        }
        w
    }

    /// A packet whose root is a branch of `count`, its children written next.
    pub fn new(buf: &'a mut [u8], count: u32) -> PacketWriter<'a> {
        let mut w = PacketWriter::root(buf);
        let at = w.branch_open(count);
        w.root_type = SpecialType::BRANCH as u8;
        w.frame = Frame {
            base: at.unwrap_or(ROOT),
            count,
            written: 0,
            root: false,
        };
        w
    }

    fn word(&mut self, at: u32, lo: u32, hi: u32) {
        let at = at as usize;
        self.buf[at..at + 4].copy_from_slice(&lo.to_le_bytes());
        self.buf[at + 4..at + 8].copy_from_slice(&hi.to_le_bytes());
    }

    fn spoil(&mut self, errnum: Errno) -> Option<u32> {
        self.error.get_or_insert(errnum);
        None
    }

    /// In u64, as the C++ writer sizes: a count past what a u32 holds is too
    /// large, never wrapped small.
    fn reserve(&mut self, unit: u32, bytes: u64) -> Option<u32> {
        if self.error.is_some() {
            return None;
        }
        let base = align(self.lim, unit);
        let end = u64::from(base) + bytes;
        if end > self.buf.len() as u64 {
            return self.spoil(Errno::EOVERFLOW);
        }
        self.buf[self.lim as usize..end as usize].fill(0);
        self.lim = end as u32;
        Some(base)
    }

    fn entry(&mut self, at: Option<u32>, type_: u8) -> &mut Self {
        let f = self.frame;
        crate::ensure!(f.written < f.count, ENOSPC, f.count);
        if f.root {
            self.root_type = type_;
        } else if self.error.is_none() {
            self.word(f.base + WORD_SZ + f.written * WORD_SZ, at.map_or(0, |b| b - f.base), type_ as u32);
        }
        self.frame.written += 1;
        self
    }

    fn scalar<const N: usize>(&mut self, type_: ScalarType, bytes: Option<[u8; N]>) -> &mut Self {
        let at = bytes.and_then(|b| {
            let at = self.reserve(N as u32, N as u64)?;
            self.buf[at as usize..at as usize + N].copy_from_slice(&b);
            Some(at)
        });
        self.entry(at, type_ as u8)
    }

    /// A sequence's length word and `bytes` of payload after it, filled by `fill`.
    fn seq(&mut self, type_: SequenceType, len: u32, bytes: u64, fill: impl FnOnce(&mut [u8])) -> &mut Self {
        let at = self.reserve(WORD_SZ, u64::from(WORD_SZ) + bytes.next_multiple_of(u64::from(WORD_SZ))).inspect(|&at| {
            self.word(at, len, RESERVED);
            let payload = (at + WORD_SZ) as usize;
            fill(&mut self.buf[payload..payload + bytes as usize]);
        });
        self.entry(at, type_ as u8)
    }

    pub fn nil(&mut self) -> &mut Self {
        self.entry(None, SpecialType::NIL as u8)
    }

    pub fn u8(&mut self, v: Option<u8>) -> &mut Self {
        self.scalar(ScalarType::U8, v.map(u8::to_le_bytes))
    }

    pub fn i8(&mut self, v: Option<i8>) -> &mut Self {
        self.scalar(ScalarType::I8, v.map(i8::to_le_bytes))
    }

    pub fn u16(&mut self, v: Option<u16>) -> &mut Self {
        self.scalar(ScalarType::U16, v.map(u16::to_le_bytes))
    }

    pub fn i16(&mut self, v: Option<i16>) -> &mut Self {
        self.scalar(ScalarType::I16, v.map(i16::to_le_bytes))
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

    pub fn i64(&mut self, v: Option<i64>) -> &mut Self {
        self.scalar(ScalarType::I64, v.map(i64::to_le_bytes))
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

    /// Followed by a NUL that the length leaves out.
    pub fn str(&mut self, s: Option<&str>) -> &mut Self {
        let Some(s) = s else { return self.entry(None, SequenceType::STR as u8) };
        self.str_with(|room| {
            let out = room.get_mut(..s.len())?;
            out.copy_from_slice(s.as_bytes());
            Some(s.len())
        })
    }

    /// `len` + 1 offsets into the UTF-8 after them.
    pub fn strs<S: AsRef<str>>(&mut self, items: Option<&[S]>) -> &mut Self {
        match items {
            Some(items) => match u32::try_from(strs_size(items)) {
                Ok(size) => self.seq(SequenceType::STRS, items.len() as u32, u64::from(size), |out| strs_payload(items, out)),
                Err(_) => {
                    self.spoil(Errno::EOVERFLOW);
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
        self.str_with(|buf| {
            let mut room = Room { buf, len: 0 };
            core::fmt::Write::write_fmt(&mut room, format_args!("{v}")).ok().map(|()| room.len)
        })
    }

    /// A string written into the room after its length's word, `write`
    /// answering how many bytes it wrote, or none where they did not fit:
    /// the one way a string goes in, whether copied or written in place, so
    /// the two lay out alike. Followed by a NUL that the length leaves out.
    fn str_with(&mut self, write: impl FnOnce(&mut [u8]) -> Option<usize>) -> &mut Self {
        let base = align(self.lim, WORD_SZ) as usize;
        let payload = base + WORD_SZ as usize;
        if self.error.is_some() || payload > self.buf.len() {
            self.spoil(Errno::EOVERFLOW);
            return self.entry(None, SequenceType::STR as u8);
        }
        let end = write(&mut self.buf[payload..])
            .map(|len| (len, align((payload + len + 1) as u32, WORD_SZ) as usize))
            .filter(|&(_, end)| end <= self.buf.len());
        let Some((len, end)) = end else {
            self.spoil(Errno::EOVERFLOW);
            return self.entry(None, SequenceType::STR as u8);
        };
        self.buf[self.lim as usize..payload].fill(0);
        self.buf[payload + len..end].fill(0);
        self.word(base as u32, len as u32, RESERVED);
        self.lim = end as u32;
        self.entry(Some(base as u32), SequenceType::STR as u8)
    }

    /// An array of `type_` from its elements' bytes, little-endian; bytes
    /// that are not whole elements refuse the packet.
    fn array_bytes(&mut self, type_: SequenceType, a: Option<&[u8]>) -> &mut Self {
        let Some(a) = a else { return self.entry(None, type_ as u8) };
        let elem = ELEM_SZ(type_) as usize;
        match u32::try_from(a.len() / elem) {
            Ok(len) if a.len() % elem == 0 => self.seq(type_, len, a.len() as u64, |out| out.copy_from_slice(a)),
            _ => {
                self.spoil(Errno::EBADMSG);
                self.entry(None, type_ as u8)
            }
        }
    }

    pub fn u8_array(&mut self, a: Option<&[u8]>) -> &mut Self {
        self.array_bytes(SequenceType::U8_ARRAY, a)
    }

    fn array_with<const N: usize>(&mut self, type_: SequenceType, len: u32, mut value: impl FnMut(u32) -> [u8; N]) -> &mut Self {
        self.seq(type_, len, u64::from(len) * N as u64, |out| {
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

    /// One byte a bit, packed low bit first.
    pub fn bitset(&mut self, bits: Option<&[u8]>) -> &mut Self {
        match bits {
            Some(bits) => self.seq(SequenceType::BITSET, bits.len() as u32, (bits.len() as u64).div_ceil(8), |out| {
                for (i, &b) in bits.iter().enumerate() {
                    if b != 0 {
                        out[i >> 3] |= 1 << (i & 7);
                    }
                }
            }),
            None => self.entry(None, SequenceType::BITSET as u8),
        }
    }

    fn branch_open(&mut self, count: u32) -> Option<u32> {
        let at = self.reserve(WORD_SZ, u64::from(WORD_SZ) * (1 + u64::from(count)));
        if let Some(at) = at {
            self.word(at, count, RESERVED);
        }
        at
    }

    /// A branch ends on a word, so what follows it starts where the encoder
    /// starts it.
    fn branch_close(&mut self, at: Option<u32>) -> Option<u32> {
        if at.is_none() || self.error.is_some() {
            return at;
        }
        let end = align(self.lim, WORD_SZ);
        if end as usize > self.buf.len() {
            return self.spoil(Errno::EOVERFLOW);
        }
        self.buf[self.lim as usize..end as usize].fill(0);
        self.lim = end;
        at
    }

    /// Writing a different number of children than `count` is a bug.
    pub fn branch(&mut self, count: u32, fill: impl FnOnce(&mut Self)) -> &mut Self {
        let at = self.branch_open(count);
        // A branch that did not fit still counts its children, against a base
        // nothing is written at.
        let outer = core::mem::replace(
            &mut self.frame,
            Frame {
                base: at.unwrap_or(ROOT),
                count,
                written: 0,
                root: false,
            },
        );
        fill(self);
        crate::ensure!(self.frame.written == self.frame.count, ENOTRECOVERABLE, self.frame.count, self.frame.written);
        self.frame = outer;
        let at = self.branch_close(at);
        self.entry(at, SpecialType::BRANCH as u8)
    }

    pub fn no_branch(&mut self) -> &mut Self {
        self.entry(None, SpecialType::BRANCH as u8)
    }

    /// Its root's payload copied, placed as its root's type places it.
    pub fn graft(&mut self, packet: &[u8]) -> &mut Self {
        let desc = (packet.len() >= HDR_SZ as usize)
            .then(|| GET_LEN(packet, 0).ok().filter(|&m| m == MAGIC).and_then(|_| GET_DESC(packet, WORD_SZ).ok()))
            .flatten();
        let Some((payload_sz, type_)) = desc else {
            self.spoil(Errno::EBADMSG);
            return self.entry(None, SpecialType::NIL as u8);
        };
        let unit = ALIGN_SZ(type_).ok().filter(|u| payload_sz.is_multiple_of(*u));
        let at = match unit {
            _ if payload_sz as usize > packet.len() - HDR_SZ as usize => self.spoil(Errno::EBADMSG),
            _ if payload_sz == 0 => None,
            None => self.spoil(Errno::EBADMSG),
            Some(unit) => self.reserve(unit, u64::from(payload_sz)).inspect(|&at| {
                let (at, from) = (at as usize, HDR_SZ as usize);
                self.buf[at..at + payload_sz as usize].copy_from_slice(&packet[from..from + payload_sz as usize]);
            }),
        };
        self.entry(at, type_)
    }

    /// The length, header included and to the word, or why it was refused.
    pub fn finish(mut self) -> Result<u32, MarshalError> {
        let f = self.frame;
        crate::ensure!(f.written == f.count, ENOTRECOVERABLE, f.count, f.written);
        // The root branch `new` opened, closed as `branch` closes one.
        if !f.root {
            self.branch_close(Some(f.base));
        }
        if let Some(e) = self.error {
            return Err(MarshalError::new(e));
        }
        let end = align(self.lim, WORD_SZ);
        if end as usize > self.buf.len() {
            return Err(MarshalError::new(Errno::EOVERFLOW));
        }
        self.buf[self.lim as usize..end as usize].fill(0);
        self.word(0, MAGIC, RESERVED);
        self.word(WORD_SZ, self.lim - HDR_SZ, self.root_type as u32);
        Ok(end)
    }

    /// `finish`'s length, or none when the packet was refused: for a room
    /// that fits a packet or not.
    pub fn fitted(self) -> Option<u32> {
        self.finish().ok()
    }
}

#[cfg(test)]
#[path = "encode.test.rs"]
mod test;
