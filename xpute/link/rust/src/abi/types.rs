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

    /// Stores a record of these fields, each borrowed, as `store` stores one.
    pub fn store_of(w_: &mut _rt::Lower, at_: usize, buf: u64, nbyte: u32) {
        w_.u64(at_, buf);
        w_.u32(at_ + 8, nbyte);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Lent, _rt::Errno> {
        Ok(Lent {
            buf: r.u64(at)?,
            nbyte: r.u32(at + 8)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ErrorPacket {
    pub file: String,
    pub line: u32,
    pub msg: Option<String>,
}

impl ErrorPacket {
    pub const SZ: usize = 24;
    pub const ALIGN: usize = 4;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.str(at, &self.file);
        w.u32(at + 8, self.line);
        if let Some(v) = &self.msg {
            w.u8(at + 12, 1);
            w.str(at + 12 + 4, v);
        }
    }

    /// Stores a record of these fields, each borrowed, as `store` stores one.
    pub fn store_of(w_: &mut _rt::Lower, at_: usize, file: &str, line: u32, msg: Option<&str>) {
        w_.str(at_, file);
        w_.u32(at_ + 8, line);
        if let Some(v) = &msg {
            w_.u8(at_ + 12, 1);
            w_.str(at_ + 12 + 4, v);
        }
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<ErrorPacket, _rt::Errno> {
        Ok(ErrorPacket {
            file: r.str(at).map(str::to_owned)?,
            line: r.u32(at + 8)?,
            msg: r.case(at + 12, 2).and_then(|d| {
                if d == 0 {
                    Ok(None)
                } else {
                    r.str(at + 12 + 4).map(str::to_owned).map(Some)
                }
            })?,
        })
    }
}

mod _rt {
    pub(super) use crate::{Lift, Lower};
    pub(super) use xpute_kit::status::errno::Errno;
}
