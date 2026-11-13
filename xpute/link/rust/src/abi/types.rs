// xpute-link/abi/types.rs
//
// GENERATED from link/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["xpute:link", "types"];

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
        let t = c.type_();
        if t == ScalarType::U8 as u8
            || t == ScalarType::I8 as u8
            || t == ScalarType::U16 as u8
            || t == ScalarType::I16 as u8
            || t == ScalarType::U32 as u8
            || t == ScalarType::I32 as u8
            || t == ScalarType::F32 as u8
            || t == ScalarType::F64 as u8
        {
            return c.get_number().map(|v| v as i32).map(Token::Int);
        }
        if t == SequenceType::STR as u8 {
            return c.get_str_ref().map(str::to_owned).map(Token::Str);
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
