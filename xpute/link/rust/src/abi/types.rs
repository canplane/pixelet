// xpute-link/abi/types.rs
//
// GENERATED from link/wit (xpute-link-gen wit.rs)

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Int(i32),
    Str(String),
}

impl Token {
    pub const SZ: usize = 12;
    pub const ALIGN: usize = 4;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        match self {
            Token::Int(v) => {
                w.u8(at, 0);
                w.i32(at + 4, *v);
            }
            Token::Str(v) => {
                w.u8(at, 1);
                w.str(at + 4, v);
            }
        }
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Token, _rt::Errno> {
        match r.case(at, 2)? {
            0 => r.i32(at + 4).map(Token::Int),
            1 => r.str(at + 4).map(str::to_owned).map(Token::Str),
            _ => Err(_rt::Errno::EBADMSG),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lent {
    pub buf: u64,
    pub nbyte: u32,
}

impl Lent {
    pub const SZ: usize = 16;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u64(at, self.buf);
        w.u32(at + 8, self.nbyte);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Lent, _rt::Errno> {
        Ok(Lent {
            buf: r.u64(at)?,
            nbyte: r.u32(at + 8)?,
        })
    }
}

mod _rt {
    pub(super) use crate::{Lift, Lower};
    pub(super) use xpute_kit::status::errno::Errno;
}
