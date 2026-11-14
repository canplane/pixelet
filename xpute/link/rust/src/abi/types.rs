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
    pub at: u32,
    pub len: u32,
}

impl Lent {
    pub const SZ: usize = 8;
    pub const ALIGN: usize = 4;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u32(at, self.at);
        w.u32(at + 4, self.len);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Lent, _rt::Errno> {
        Ok(Lent {
            at: r.u32(at)?,
            len: r.u32(at + 4)?,
        })
    }
}

mod _rt {
    pub use crate::{Lift, Lower};
    pub use xpute_kit::status::errno::Errno;
}
