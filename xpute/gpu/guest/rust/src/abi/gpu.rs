// xpute-gpu-guest/abi/gpu.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use core::future::Future;
use std::rc::Rc;
use xpute_guest::link;
use xpute_guest::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::abi::types::Lent;
use xpute_link::Lend;
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `xpute:gpu/gpu` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x79d455;

/// `run`
pub const RUN: Method = 0;
/// `[method]device.lost`
pub const DEVICE_LOST: Method = 0;
/// `[method]device.ready`
pub const DEVICE_READY: Method = 1;
/// `[method]device.settled`
pub const DEVICE_SETTLED: Method = 2;

pub fn run<L: Lend>(records: L) -> impl Future<Output = (L, Result<(), Errno>)> {
    let mut records = records;
    let (records_at_, records_len_) = Lend::span(&mut records);
    let sent = wait::wait(
        HANDLE,
        RUN,
        Some((1, &|w_: &mut PacketWriter| {
            Lent {
                at: records_at_,
                len: records_len_,
            }
            .write(w_);
        })),
        records,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, xpute_link::answer(r).map(|_| ()))
    }
}

pub fn device_lost<L: Lend>(
    device_: Handle,
    block: L,
) -> impl Future<Output = (L, Result<(), Errno>)> {
    let mut block = block;
    let (block_at_, block_len_) = Lend::span(&mut block);
    let sent = wait::wait(
        device_,
        DEVICE_LOST,
        Some((1, &|w_: &mut PacketWriter| {
            Lent {
                at: block_at_,
                len: block_len_,
            }
            .write(w_);
        })),
        block,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, xpute_link::answer(r).map(|_| ()))
    }
}

pub fn device_ready(device_: Handle) -> impl Future<Output = Result<u32, Errno>> {
    let sent = wait::wait(device_, DEVICE_READY, None, ());
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|v| v as u32)
    }
}

pub fn device_settled(device_: Handle) -> impl Future<Output = Result<(), Errno>> {
    let sent = wait::wait(device_, DEVICE_SETTLED, None, ());
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|_| ())
    }
}
