// xpute-kit/wire/xtp/encode.rs

use crate::abi::word::field_set32;
use crate::status::errno::Errno;
use crate::status::error::XputeError;

use super::cursor::{ArrayElem, bytes_of};
use super::spec::{
    ALIGN_SZ, BranchNode, DESC_TYPE_MASK, DESC_TYPE_SHAMT, ELEM_SZ, GET_DESC, GET_LEN, GraftNode,
    HDR_SZ, MAGIC, MAX_PKT_SZ, Node, Numeric, RESERVED, ScalarNode, ScalarType, SequenceNode,
    SequenceType, SequenceVal, SpecialType, WORD_SZ,
};
use super::view::NodeView;

pub struct TreeEncoderConfig {
    /// The size of the first buffer tried; it doubles each time the packet
    /// does not fit.
    pub init_cap: u32,
    /// The largest packet allowed; MAX_PKT_SZ when none is given.
    pub max_cap: Option<u32>,
}

/// This encodes a tree by walking it onto `PacketWriter`, as the C++ `encode`
/// (cpp/xpute/kit/wire/xtp/view.hpp) does. The first buffer has `init_cap`
/// bytes; each time the packet overflows it, the tree is walked again into
/// one twice as large, up to `max_cap`.
#[derive(Default)]
pub struct TreeEncoder;

impl TreeEncoder {
    pub fn new() -> TreeEncoder {
        TreeEncoder
    }

    /// A `max_cap` below HDR_SZ or above MAX_PKT_SZ is EINVAL; a packet larger
    /// than `max_cap` is EOVERFLOW.
    pub fn encode<'a, V: NodeView<'a>>(
        &mut self,
        view: &V,
        cfg: TreeEncoderConfig,
    ) -> Result<Vec<u8>, XputeError> {
        let max_cap = cfg.max_cap.unwrap_or(MAX_PKT_SZ);
        if !(HDR_SZ..=MAX_PKT_SZ).contains(&max_cap) {
            return Err(XputeError::new(Errno::EINVAL));
        }
        let mut cap = HDR_SZ.max(cfg.init_cap.min(max_cap));
        loop {
            let mut buf = vec![0u8; cap as usize];
            let mut w = PacketWriter::root(&mut buf);
            walk(&mut w, view.node());
            match w.finish() {
                Ok(n) => {
                    buf.truncate(n as usize);
                    return Ok(buf);
                }
                Err(e) if e.errnum == Errno::EOVERFLOW && cap < max_cap => {
                    cap = if cap > max_cap / 2 { max_cap } else { cap * 2 }
                }
                Err(e) => return Err(e),
            }
        }
    }
}

pub fn encoder() -> TreeEncoder {
    TreeEncoder::new()
}

/// A node whose value does not match its type, such as a STR that holds an
/// array, makes `finish` refuse the packet.
fn walk(w: &mut PacketWriter<'_>, node: &Node) {
    match node {
        Node::Nil(_) => {
            w.nil();
        }
        Node::Scalar(n) => scalar_into(w, n),
        Node::Sequence(n) => seq_into(w, n),
        Node::Branch(BranchNode {
            val: Some(children),
        }) => {
            w.branch(children.len() as u32, |b| {
                children.iter().for_each(|c| walk(b, c));
            });
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
        (SequenceType::BITSET | SequenceType::STR | SequenceType::STRS, _)
        | (_, Some(SequenceVal::Str(_) | SequenceVal::Strs(_))) => {
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

fn strs_sz<S: AsRef<str>>(items: &[S]) -> u64 {
    4 * (items.len() as u64 + 1) + items.iter().map(|s| s.as_ref().len() as u64).sum::<u64>()
}

const fn align(n: u32, unit: u32) -> u32 {
    (n + unit - 1) & !(unit - 1)
}

/// The child table the writer's next entries go in. The root is treated as a
/// branch of one child whose entry is the header's root word: `root` marks it,
/// and its entry sets the root's type instead of writing a descriptor.
#[derive(Clone, Copy)]
struct Table {
    base: u32,
    cnt: u32,
    written: u32,
    root: bool,
}

/// Writes a packet in place, into bytes the caller holds, without building a
/// tree and without allocating. It is the one writer of the layout:
/// `TreeEncoder` walks its tree onto it, so both give the same bytes. A node
/// that does not fit does not fail its call: the writer goes on counting
/// entries, and `finish` refuses the packet (EOVERFLOW).
pub struct PacketWriter<'a> {
    buf: &'a mut [u8],
    table: Table,
    root_type: u8,
    lim: u32,
    /// This is the first error met; later ones are not kept.
    err: Option<Errno>,
}

impl<'a> PacketWriter<'a> {
    /// A writer for a packet whose root is any one node. The packet may take
    /// the whole buffer, up to MAX_PKT_SZ bytes.
    pub fn root(buf: &'a mut [u8]) -> PacketWriter<'a> {
        let cap = buf.len().min(MAX_PKT_SZ as usize);
        let mut w = PacketWriter {
            buf: &mut buf[..cap],
            table: Table {
                base: 0,
                cnt: 1,
                written: 0,
                root: true,
            },
            root_type: SpecialType::NIL as u8,
            lim: HDR_SZ,
            err: None,
        };
        match w.buf.get_mut(..HDR_SZ as usize) {
            Some(h) => h.fill(0),
            None => {
                w.spoil(Errno::EINVAL);
            }
        }
        w
    }

    /// A writer for a packet whose root is a branch of `cnt` children, which
    /// the calls that follow write.
    pub fn new(buf: &'a mut [u8], cnt: u32) -> PacketWriter<'a> {
        let mut w = PacketWriter::root(buf);
        let at = w.branch_open(cnt);
        w.root_type = SpecialType::BRANCH as u8;
        w.table = Table {
            base: at.unwrap_or(HDR_SZ),
            cnt,
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
        self.err.get_or_insert(errnum);
        None
    }

    /// `bytes` is a u64, as in the C++ writer, so that a size past what a u32
    /// holds reads as too large instead of wrapping to a small one.
    fn reserve(&mut self, unit: u32, bytes: u64) -> Option<u32> {
        if self.err.is_some() {
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
        let f = self.table;
        crate::ensure!(f.written < f.cnt, ENOSPC, f.cnt);
        if f.root {
            self.root_type = type_;
        } else if self.err.is_none() {
            self.word(
                f.base + WORD_SZ + f.written * WORD_SZ,
                at.map_or(0, |b| b - f.base),
                field_set32(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, type_ as u32),
            );
        }
        self.table.written += 1;
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

    /// Writes a sequence: its len slot, then `bytes` of payload that `fill`
    /// writes, padded to a word.
    fn seq(
        &mut self,
        type_: SequenceType,
        len: u32,
        bytes: u64,
        fill: impl FnOnce(&mut [u8]),
    ) -> &mut Self {
        let at = self
            .reserve(
                WORD_SZ,
                u64::from(WORD_SZ) + bytes.next_multiple_of(u64::from(WORD_SZ)),
            )
            .inspect(|&at| {
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

    pub fn str(&mut self, s: Option<&str>) -> &mut Self {
        let Some(s) = s else {
            return self.entry(None, SequenceType::STR as u8);
        };
        self.str_with(|room| {
            let out = room.get_mut(..s.len())?;
            out.copy_from_slice(s.as_bytes());
            Some(s.len())
        })
    }

    pub fn strs<S: AsRef<str>>(&mut self, items: Option<&[S]>) -> &mut Self {
        match items {
            Some(items) => match u32::try_from(strs_sz(items)) {
                Ok(sz) => self.seq(
                    SequenceType::STRS,
                    items.len() as u32,
                    u64::from(sz),
                    |out| strs_payload(items, out),
                ),
                Err(_) => {
                    self.spoil(Errno::EOVERFLOW);
                    self.entry(None, SequenceType::STRS as u8)
                }
            },
            None => self.entry(None, SequenceType::STRS as u8),
        }
    }

    /// Writes a STR whose bytes `write` puts into the room after the len slot.
    /// `write` returns how many bytes it wrote, or `None` when they did not
    /// fit, which spoils the packet.
    fn str_with(&mut self, write: impl FnOnce(&mut [u8]) -> Option<usize>) -> &mut Self {
        let base = align(self.lim, WORD_SZ) as usize;
        let payload = base + WORD_SZ as usize;
        if self.err.is_some() || payload > self.buf.len() {
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

    /// Writes an array of `type_` from its elements' little-endian bytes. A
    /// length that is not a whole number of elements makes `finish` refuse
    /// the packet.
    fn array_bytes(&mut self, type_: SequenceType, a: Option<&[u8]>) -> &mut Self {
        let Some(a) = a else {
            return self.entry(None, type_ as u8);
        };
        let elem = ELEM_SZ(type_) as usize;
        match u32::try_from(a.len() / elem) {
            Ok(len) if a.len() % elem == 0 => {
                self.seq(type_, len, a.len() as u64, |out| out.copy_from_slice(a))
            }
            _ => {
                self.spoil(Errno::EBADMSG);
                self.entry(None, type_ as u8)
            }
        }
    }

    pub fn array<T: ArrayElem>(&mut self, a: Option<&[T]>) -> &mut Self {
        self.array_bytes(T::TYPE, a.map(bytes_of))
    }

    /// `bits` holds one byte for each bit, nonzero for set. They are packed
    /// eight to a byte, the first in the lowest bit.
    pub fn bitset(&mut self, bits: Option<&[u8]>) -> &mut Self {
        match bits {
            Some(bits) => self.seq(
                SequenceType::BITSET,
                bits.len() as u32,
                (bits.len() as u64).div_ceil(8),
                |out| {
                    for (i, &b) in bits.iter().enumerate() {
                        if b != 0 {
                            out[i >> 3] |= 1 << (i & 7);
                        }
                    }
                },
            ),
            None => self.entry(None, SequenceType::BITSET as u8),
        }
    }

    fn branch_open(&mut self, cnt: u32) -> Option<u32> {
        let at = self.reserve(WORD_SZ, u64::from(WORD_SZ) * (1 + u64::from(cnt)));
        if let Some(at) = at {
            self.word(at, cnt, RESERVED);
        }
        at
    }

    /// A branch's extent ends on a word boundary, so a node after it starts on
    /// the next word even when it needs less alignment. Every language pads
    /// the same way, which keeps their bytes equal.
    fn branch_close(&mut self, at: Option<u32>) -> Option<u32> {
        if at.is_none() || self.err.is_some() {
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

    /// Writing a number of children other than `cnt` is a bug and panics.
    pub fn branch(&mut self, cnt: u32, fill: impl FnOnce(&mut Self)) -> &mut Self {
        let at = self.branch_open(cnt);
        // When the branch did not fit, its children are still counted, so that
        // a miscount is caught; the packet is spoiled, and nothing is written
        // at the stand-in base.
        let outer = core::mem::replace(
            &mut self.table,
            Table {
                base: at.unwrap_or(HDR_SZ),
                cnt,
                written: 0,
                root: false,
            },
        );
        fill(self);
        crate::ensure!(
            self.table.written == self.table.cnt,
            ENOTRECOVERABLE,
            self.table.cnt,
            self.table.written
        );
        self.table = outer;
        let at = self.branch_close(at);
        self.entry(at, SpecialType::BRANCH as u8)
    }

    pub fn no_branch(&mut self) -> &mut Self {
        self.entry(None, SpecialType::BRANCH as u8)
    }

    /// Copies the root of the encoded packet `pkt` in as this entry, aligned as
    /// its root's type requires. A malformed `pkt` makes `finish` refuse the
    /// packet (EBADMSG).
    pub fn graft(&mut self, pkt: &[u8]) -> &mut Self {
        let desc = (pkt.len() >= HDR_SZ as usize)
            .then(|| {
                GET_LEN(pkt, 0)
                    .ok()
                    .filter(|&m| m == MAGIC)
                    .and_then(|_| GET_DESC(pkt, WORD_SZ).ok())
            })
            .flatten();
        let Some((payload_sz, type_)) = desc else {
            self.spoil(Errno::EBADMSG);
            return self.entry(None, SpecialType::NIL as u8);
        };
        let unit = ALIGN_SZ(type_)
            .ok()
            .filter(|u| payload_sz.is_multiple_of(*u));
        let at = match unit {
            _ if payload_sz as usize > pkt.len() - HDR_SZ as usize => self.spoil(Errno::EBADMSG),
            _ if payload_sz == 0 => None,
            None => self.spoil(Errno::EBADMSG),
            Some(unit) => self.reserve(unit, u64::from(payload_sz)).inspect(|&at| {
                let (at, from) = (at as usize, HDR_SZ as usize);
                self.buf[at..at + payload_sz as usize]
                    .copy_from_slice(&pkt[from..from + payload_sz as usize]);
            }),
        };
        self.entry(at, type_)
    }

    /// Writes the header and returns the packet's length, header included and
    /// padded to a word, or the first error that spoiled the packet.
    pub fn finish(mut self) -> Result<u32, XputeError> {
        let f = self.table;
        crate::ensure!(f.written == f.cnt, ENOTRECOVERABLE, f.cnt, f.written);
        // The root branch that `new` opened is closed here, padded as `branch`
        // pads one.
        if !f.root {
            self.branch_close(Some(f.base));
        }
        if let Some(e) = self.err {
            return Err(XputeError::new(e));
        }
        let end = align(self.lim, WORD_SZ);
        if end as usize > self.buf.len() {
            return Err(XputeError::new(Errno::EOVERFLOW));
        }
        self.buf[self.lim as usize..end as usize].fill(0);
        self.word(0, MAGIC, RESERVED);
        self.word(
            WORD_SZ,
            self.lim - HDR_SZ,
            field_set32(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, self.root_type as u32),
        );
        Ok(end)
    }

    /// This answers `finish`'s length, or `None` when the packet was refused,
    /// for a caller that only needs to know whether the packet fit.
    pub fn fit_len(self) -> Option<u32> {
        self.finish().ok()
    }
}

#[cfg(test)]
#[path = "encode.test.rs"]
mod test;
