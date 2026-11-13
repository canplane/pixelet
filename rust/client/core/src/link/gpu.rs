// pxl-core/link/gpu.rs

//! The GPU's device as the core holds it (xpute gpu/wit/gpu.wit). One task
//! lends the device's block to `device.lost`, asks the device ready, and once
//! the device is lost asks again; a device refused, as a hidden page's may be,
//! is asked again once the app is shown. Another task hears each completion a
//! record asked for, and asks for a frame to take it in.

use pxl_render::gpu::api;
use xpute_guest::link;
use xpute_link::Handle;
use xpute_gpu_guest::abi::gpu;
use xpute_gpu_link::spec::DEVICE_URI;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::link::life;
use crate::sys::sched::{executor, invalidate};

/// Opens the GPU's device and starts its tasks.
pub(super) fn boot() {
    let device = link::open(&xpute_link::uri(DEVICE_URI)).or_bug(Errno::ENODEV);
    executor::spawner().spawn(hold(device).or_bug(Errno::ENOSPC));
    executor::spawner().spawn(settle(device).or_bug(Errno::ENOSPC));
}

#[embassy_executor::task]
async fn hold(device: Handle) {
    let mut block = None;
    loop {
        // The block is lent before the device is asked ready.
        let lost = gpu::device_lost(device, api::lend_block(block.take()));
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
