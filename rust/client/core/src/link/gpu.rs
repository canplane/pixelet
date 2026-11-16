// pxl-core/link/gpu.rs

//! The GPU's device as the core holds it (spec/gpu/wit/gpu.wit). One task
//! lends the device's block to `device.lost`, asks the device ready, and once
//! the device is lost asks again. A device refused is asked again after a
//! wait that doubles with each refusal in a row (link/param.rs), or at once
//! when the app is shown again, the same on a page and on a device. Another
//! task hears each wake of the device (`settle`).

use core::future::poll_fn;
use core::pin::pin;
use core::task::Poll;

use pxl_gpu_guest::abi::gpu;
use pxl_render::gpu::api;
use xpute_guest::abi::clocks::monotonic_clock::wait_for;
use xpute_guest::clock::duration;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::Handle;

use crate::link::life;
use crate::link::param::{DEVICE_RETRY_BASE_MS, DEVICE_RETRY_CAP_MS};
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
        let mut wait_ms = DEVICE_RETRY_BASE_MS;
        loop {
            let since = life::show_cnt();
            if matches!(gpu::device_ready(device).await, Ok(Ok(()))) {
                break;
            }
            let mut shown = pin!(life::shown_after(since));
            let mut waited = pin!(wait_for(duration(wait_ms)));
            poll_fn(|cx| {
                if shown.as_mut().poll(cx).is_ready() || waited.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(());
                }
                Poll::Pending
            })
            .await;
            wait_ms = (wait_ms * 2.0).min(DEVICE_RETRY_CAP_MS);
        }
        api::on_device_ready();
        invalidate();
        let (back, _) = lost.await;
        block = Some(back);
        invalidate();
    }
}

/// Hears each wake of the device: a completion a record asked for, or chunks
/// of the belt come free. The entry the answer comes in reads what landed,
/// and asks for a frame only where a frame takes it in (link.rs `end`,
/// pxl-render `renderer::complete`) or where a write refused room may now
/// have it (pxl-render `api::take_retry`); a wake that brings neither draws
/// no frame.
#[embassy_executor::task]
async fn settle(device: Handle) {
    while gpu::device_settled(device).await.is_ok() {}
}
