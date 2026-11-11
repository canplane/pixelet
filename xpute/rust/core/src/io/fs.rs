// xpute-core/io/fs.rs

//! This module is the core's end of the file device (abi/fs.json): a file's
//! path, a descriptor, and what its completions carry. The file calls are
//! `Door`'s (io/door.rs), and `FileTable` keeps each descriptor and its owner
//! (io/file.rs).

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter};

use crate::abi::number::integer;

#[path = "fs.spec.rs"]
pub mod spec;

pub type Fd = u32;

/// A token of a file's path: a number the program gives a fixed name, or
/// variable data in its own type. xpute gives no token a meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token<'a> {
    U32(u32),
    Str(&'a str),
}

impl<'a> Token<'a> {
    /// Reads a branch's child: a string, or a number that is a u32.
    fn of(c: &Cursor<'a>) -> Result<Token<'a>, MarshalError> {
        match c.get_str_ref() {
            Ok(s) => Ok(Token::Str(s)),
            Err(_) => Ok(Token::U32(integer(c.get_number()?)?)),
        }
    }
}

/// A file's path as a packet carries it, a branch of its tokens, read in
/// place. Each token was checked when the path was read.
#[derive(Clone, Copy, Debug)]
pub struct Path<'a> {
    pkt: &'a [u8],
    base: u32,
    len: u32,
}

impl<'a> Path<'a> {
    /// Reads the path at `c`, a node of `pkt`.
    pub fn read(pkt: &'a [u8], c: Cursor<'a>) -> Result<Path<'a>, MarshalError> {
        let b = c.as_branch()?;
        let mut len = 0;
        for child in b.iter() {
            Token::of(&child?)?;
            len += 1;
        }
        Ok(Path {
            pkt,
            base: b.base(),
            len,
        })
    }

    /// Writes `tokens` as a path: a branch of a u32 or a string each.
    pub fn write(w: &mut PacketWriter, tokens: &[Token]) {
        w.branch(tokens.len() as u32, |w| {
            for t in tokens {
                match *t {
                    Token::U32(v) => w.u32(Some(v)),
                    Token::Str(s) => w.str(Some(s)),
                };
            }
        });
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, i: u32) -> Option<Token<'a>> {
        (i < self.len).then(|| {
            let b = BranchCursor::new(self.pkt, self.base).or_bug(Errno::ENOTRECOVERABLE);
            let c = b.at(i).or_bug(Errno::ENOTRECOVERABLE);
            Token::of(&c).or_bug(Errno::ENOTRECOVERABLE)
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = Token<'a>> + 'a {
        let path = *self;
        (0..path.len).filter_map(move |i| path.get(i))
    }
}
