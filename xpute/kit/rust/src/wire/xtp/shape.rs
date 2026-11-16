// xpute-kit/wire/xtp/shape.rs

//! A Rust type that names the shape of an XTP packet, so that the compiler
//! checks what is written and what is read. A tuple is a branch whose
//! children are its elements, in order, and a number or a bool is a scalar
//! leaf. A `Vec<T>` stands for a packed array of `T`, and a `Vec<String>` for
//! a STRS leaf; both are read in place in the packet, with no vector built. A
//! read refuses (EBADMSG) a child of any type but the one named, and a branch
//! of any other length. This module is built on XTP's public writer and
//! reader, and XTP does not depend on it.

use crate::status::errno::Errno;
use crate::status::error::XputeError;

use super::{ArrayElem, Cursor, HDR_SZ, NodeValue, PacketWriter, StrList, TreeReader, WORD_SZ};

const WORD: u64 = WORD_SZ as u64;

fn refused<T>() -> Result<T, XputeError> {
    Err(XputeError::new(Errno::EBADMSG))
}

pub trait Shape {
    /// What a read returns, which points into the packet where it can.
    type Read<'a>;
    type Write<'a>;

    /// An upper bound on the bytes `v` takes in a packet, not counting its
    /// entry in its parent's table.
    fn most(v: &Self::Write<'_>) -> u64;
    fn write(v: &Self::Write<'_>, w: &mut PacketWriter<'_>);
    fn read(c: Cursor<'_>) -> Result<Self::Read<'_>, XputeError>;
}

impl<T: ArrayElem + 'static> Shape for Vec<T> {
    type Read<'a> = &'a [T];
    type Write<'a> = &'a [T];

    fn most(v: &&[T]) -> u64 {
        WORD + (size_of_val(*v) as u64).next_multiple_of(WORD)
    }
    fn write(v: &&[T], w: &mut PacketWriter<'_>) {
        w.array(Some(*v));
    }
    fn read(c: Cursor<'_>) -> Result<&[T], XputeError> {
        c.get_array::<T>()
    }
}

impl Shape for Vec<String> {
    type Read<'a> = StrList<'a>;
    type Write<'a> = &'a [&'a str];

    fn most(v: &&[&str]) -> u64 {
        let text: u64 = v.iter().map(|s| s.len() as u64).sum();
        WORD + (4 * (v.len() as u64 + 1) + text).next_multiple_of(WORD)
    }
    fn write(v: &&[&str], w: &mut PacketWriter<'_>) {
        w.strs(Some(*v));
    }
    fn read(c: Cursor<'_>) -> Result<StrList<'_>, XputeError> {
        c.get_strs()
    }
}

macro_rules! scalar {
    ($($t:ident => $value:ident),* $(,)?) => {
        $(impl Shape for $t {
            type Read<'a> = $t;
            type Write<'a> = $t;

            fn most(_: &$t) -> u64 {
                WORD
            }
            fn write(v: &$t, w: &mut PacketWriter<'_>) {
                w.$t(Some(*v));
            }
            fn read(c: Cursor<'_>) -> Result<$t, XputeError> {
                match c.get()? {
                    NodeValue::$value(v) => Ok(v),
                    _ => refused(),
                }
            }
        })*
    };
}

scalar!(
    u8 => U8, i8 => I8, u16 => U16, i16 => I16, u32 => U32, i32 => I32, u64 => U64, i64 => I64,
    f32 => F32, f64 => F64, bool => Bool,
);

macro_rules! branch {
    ($($s:ident $i:tt),+) => {
        impl<$($s: Shape),+> Shape for ($($s,)+) {
            type Read<'a> = ($($s::Read<'a>,)+);
            type Write<'a> = ($($s::Write<'a>,)+);

            fn most(v: &Self::Write<'_>) -> u64 {
                let cnt = [$($i),+].len() as u64;
                WORD * (1 + cnt) $(+ $s::most(&v.$i))+
            }
            fn write(v: &Self::Write<'_>, w: &mut PacketWriter<'_>) {
                w.branch([$($i),+].len() as u32, |w| {
                    $($s::write(&v.$i, w);)+
                });
            }
            fn read(c: Cursor<'_>) -> Result<Self::Read<'_>, XputeError> {
                let b = c.as_branch()?;
                if b.len as usize != [$($i),+].len() {
                    return refused();
                }
                Ok(($($s::read(b.at($i)?)?,)+))
            }
        }
    };
}

branch!(A 0);
branch!(A 0, B 1);
branch!(A 0, B 1, C 2);
branch!(A 0, B 1, C 2, D 3);
branch!(A 0, B 1, C 2, D 3, E 4);
branch!(A 0, B 1, C 2, D 3, E 4, F 5);
branch!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
branch!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
branch!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8);
branch!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9);
branch!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10);
branch!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11);

/// This encodes `v` as a packet of shape `S`. A packet that needs more than
/// `cap` bytes is EOVERFLOW.
pub fn encode<S: Shape>(v: S::Write<'_>, cap: u32) -> Result<Vec<u8>, XputeError> {
    let most = u64::from(HDR_SZ) + S::most(&v);
    let mut buf = vec![0u8; most.min(u64::from(cap)) as usize];
    let mut w = PacketWriter::root(&mut buf);
    S::write(&v, &mut w);
    let n = w.finish()?;
    buf.truncate(n as usize);
    Ok(buf)
}

/// Reads a packet of shape `S`. Its arrays and strings are read in place.
pub fn get_deep<S: Shape>(pkt: &[u8]) -> Result<S::Read<'_>, XputeError> {
    S::read(TreeReader::new(pkt)?.read())
}

#[cfg(test)]
#[path = "shape.test.rs"]
mod test;
