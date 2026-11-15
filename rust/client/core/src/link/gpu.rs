// pxl-core/link/gpu.rs

//! The GPU's device as the core holds it (spec/gpu/wit/gpu.wit). One task
//! lends the device's block to `device.lost`, asks the device ready, and once
//! the device is lost asks again; a device refused, as a hidden page's may be,
//! is asked again once the app is shown. Another task hears each completion a
//! record asked for, and asks for a frame to take it in.

use pxl_gpu_guest::abi::gpu;
use pxl_render::gpu::api;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::Handle;

use crate::link::life;
use crate::sys::sched::{executor, invalidate};

/// Takes the GPU's device and starts its tasks.
pub(super) fn boot() {
    let device = gpu::get_device().or_bug(Errno::ENODEV);
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
            let since = life::show_cnt();
            if matches!(gpu::device_ready(device).await, Ok(Ok(()))) {
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
