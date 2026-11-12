// pxl-core/link/gpu.rs

//! The GPU's device as the core holds it (xpute wit/gpu/gpu.wit): one task
//! lends the device's block to `device.lost`, asks the device ready, and once
//! the device is lost asks again; a device refused, as a hidden page's may
//! be, is asked again once the app is shown. Another task hears each
//! completion a record asked for, and asks for a frame to take it in.

use core::future::{poll_fn, Future};
use core::pin::pin;
use core::task::Poll;

use pxl_render::gpu::api;
use xpute_core::link::{self, Handle};
use xpute_gpu::abi::gpu;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::link::life;
use crate::sys::sched::{executor, invalidate};

/// Opens the device and starts its tasks.
pub(super) fn boot() {
    let device = link::open(&link::uri(&["xpute:gpu", "device"])).or_bug(Errno::ENODEV);
    executor::spawner().spawn(hold(device).or_bug(Errno::ENOSPC));
    executor::spawner().spawn(settle(device).or_bug(Errno::ENOSPC));
}

#[embassy_executor::task]
async fn hold(device: Handle) {
    let mut block = None;
    loop {
        let mut lost = pin!(gpu::device_lost(device, api::lend_block(block.take())));
        // The block is lent before the device is asked ready.
        poll_fn(|cx| {
            _ = lost.as_mut().poll(cx);
            Poll::Ready(())
        })
        .await;
        loop {
            let since = life::shows();
            if gpu::device_ready(device).await.is_ok() {
                break;
            }
            life::shown_after(since).await;
        }
        api::on_device_ready();
        invalidate();
        let (back, _) = lost.await;
        block = Some(back);
        invalidate();
    }
}

#[embassy_executor::task]
async fn settle(device: Handle) {
    while gpu::device_settled(device).await.is_ok() {
        invalidate();
    }
}
