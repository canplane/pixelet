// xpute-gpu/runtime/admit.rs

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use xpute_runtime::mem::Memory;

use super::surface::Frame;
use super::GpuRuntime;
use crate::device;

/// A frame admitted and not yet done, counted until the queue's callback
/// that holds it ends: called once the work is done, or let go uncalled, as
/// wgpu lets a gone device's callbacks go. Taken off as it drops, so a frame
/// never stands forever.
pub(super) struct FrameGuard(Arc<AtomicU32>);

impl FrameGuard {
    /// `count`'s next frame, and the callback the queue is handed for it.
    pub(super) fn count(count: &Arc<AtomicU32>) -> impl FnOnce() + Send + 'static {
        count.fetch_add(1, Ordering::Relaxed);
        let frame = FrameGuard(count.clone());
        move || drop(frame)
    }
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl GpuRuntime {
    /// Whether this turn may draw a frame: while fewer than its frame
    /// latency are unfinished, so each surface's next texture is free and
    /// taking it does not block. Asked once a turn, before it draws.
    pub fn admit_frame(&mut self) -> bool {
        self.admitted = crate::admit::admits(
            self.unfinished_cnt.load(Ordering::Relaxed),
            self.frame_latency,
        );
        self.admitted
    }

    /// The frames unfinished, told the core in the device's block (spec
    /// DEVICE) as a page's runtime tells them: before each turn, once the
    /// queue's callbacks that end frames have run.
    pub(super) fn tell_unfinished(&self, memory: &mut Memory<'_>) {
        if let Some(address) = self.block {
            memory.store_release(
                address + 4 * device::UNFINISHED_AT as u32,
                self.unfinished_cnt.load(Ordering::Relaxed),
            );
        }
    }

    /// Each surface's texture this turn drew, shown; an admitted frame
    /// unfinished until the queue has done the work submitted with it.
    pub fn present(&mut self) {
        for s in &mut self.surfaces {
            if let Some(Frame::Surface(frame)) = s.frame.take() {
                self.queue.present(frame);
            }
        }
        if core::mem::take(&mut self.admitted) {
            self.queue
                .on_submitted_work_done(FrameGuard::count(&self.unfinished_cnt));
        }
    }
}
