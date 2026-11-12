// xpute-core/abi/link.rs
//
// GENERATED from wit (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::cell::Cell;
use std::rc::Rc;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Int(i32),
    Str(String),
}

impl Token {
    /// Writes its payload alone, whose node's type says its case.
    pub fn write(&self, w: &mut PacketWriter) {
        match self {
            Token::Int(v) => {
                w.i32(Some((*v) as i32));
            }
            Token::Str(v) => {
                w.str(Some(&*(*v)));
            }
        }
    }

    pub fn read(c: Cursor<'_>) -> Result<Token, MarshalError> {
        if let Ok(v) = c.get_number().map(|v| v as i32) {
            return Ok(Token::Int(v));
        }
        if let Ok(v) = c.get_str_ref().map(str::to_owned) {
            return Ok(Token::Str(v));
        }
        Err(MarshalError::new(Errno::EBADMSG))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lent {
    pub at: u32,
    pub len: u32,
}

impl Lent {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(2, |w| {
            w.u32(Some(self.at as u32));
            w.u32(Some(self.len as u32));
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<Lent, MarshalError> {
        let b = c.as_branch()?;
        Ok(Lent {
            at: b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?,
            len: b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?,
        })
    }
}
