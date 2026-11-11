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

/// A token of a file's path: an i32 or a string, each of which compares
/// exactly. xpute gives no token a meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Int(i32),
    Str(&'a str),
}

impl<'a> Token<'a> {
    /// Reads a branch's child: a string, or a number whose value is a whole
    /// i32, as a whole float's is. Any other child, a fraction, a bool, a
    /// branch or an integer past an i32, is EBADMSG.
    fn of(c: &Cursor<'a>) -> Result<Token<'a>, MarshalError> {
        match c.get_str_ref() {
            Ok(s) => Ok(Token::Str(s)),
            Err(_) => c
                .get_number()
                .and_then(integer)
                .map(Token::Int)
                .map_err(|_| MarshalError::new(Errno::EBADMSG)),
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

    /// Writes `tokens` as a path: a branch of an i32 or a string each.
    pub fn write(w: &mut PacketWriter, tokens: &[Token]) {
        w.branch(tokens.len() as u32, |w| {
            for t in tokens {
                match *t {
                    Token::Int(v) => w.i32(Some(v)),
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
