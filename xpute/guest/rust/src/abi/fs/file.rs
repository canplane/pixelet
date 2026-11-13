// xpute-guest/abi/fs/file.rs
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use crate::link;
use crate::wait;
use core::future::Future;
use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::abi::types::Lent;
use xpute_link::Lend;
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// `[method]file.size`
pub const FILE_SIZE: Method = 0;
/// `[method]file.read`
pub const FILE_READ: Method = 1;
/// `[method]file.write`
pub const FILE_WRITE: Method = 2;
/// `[method]file.changed`
pub const FILE_CHANGED: Method = 3;
/// `[method]file.close`
pub const FILE_CLOSE: Method = 4;

pub fn file_size(file_: Handle) -> impl Future<Output = Result<u32, Errno>> {
    let sent = wait::wait(file_, FILE_SIZE, None, ());
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|v| v as u32)
    }
}

pub fn file_read<L: Lend>(file_: Handle, buf: L) -> impl Future<Output = (L, Result<u32, Errno>)> {
    let mut buf = buf;
    let (buf_at_, buf_len_) = Lend::span(&mut buf);
    let sent = wait::wait(
        file_,
        FILE_READ,
        Some((1, &|w_: &mut PacketWriter| {
            Lent {
                at: buf_at_,
                len: buf_len_,
            }
            .write(w_);
        })),
        buf,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, xpute_link::answer(r).map(|v| v as u32))
    }
}

pub fn file_write<L: Lend>(file_: Handle, buf: L) -> impl Future<Output = (L, Result<(), Errno>)> {
    let mut buf = buf;
    let (buf_at_, buf_len_) = Lend::span(&mut buf);
    let sent = wait::wait(
        file_,
        FILE_WRITE,
        Some((1, &|w_: &mut PacketWriter| {
            Lent {
                at: buf_at_,
                len: buf_len_,
            }
            .write(w_);
        })),
        buf,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, xpute_link::answer(r).map(|_| ()))
    }
}

pub fn file_changed(file_: Handle) -> impl Future<Output = Result<(), Errno>> {
    let sent = wait::wait(file_, FILE_CHANGED, None, ());
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|_| ())
    }
}

pub fn file_close(file_: Handle) -> impl Future<Output = Result<(), Errno>> {
    let sent = wait::wait(file_, FILE_CLOSE, None, ());
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|_| ())
    }
}
