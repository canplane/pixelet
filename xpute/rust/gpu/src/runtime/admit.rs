// xpute-gpu/runtime/admit.rs

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use xpute_runtime::mem::Memory;

use super::surface::Frame;
use super::GpuRuntime;
use crate::device;

/// A `FrameGuard` is a frame admitted and not yet done. It stays counted until
/// the queue's callback that holds it ends, either called once the work is done
/// or dropped uncalled, as wgpu drops a lost device's callbacks. The count is
/// taken off as the guard drops, so no frame stays counted forever.
pub(super) struct FrameGuard(Arc<AtomicU32>);

impl FrameGuard {
    /// Counts a frame on `count`, and returns the callback to hand the queue
    /// for it.
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
    /// This says whether this turn may draw a frame. It may while fewer frames
    /// than the frame latency are unfinished, so each surface's next texture is
    /// free and taking it does not block. Ask once a turn, before drawing:
    /// `present` counts the frame only where it was admitted.
    pub fn admit_frame(&mut self) -> bool {
        self.admitted = crate::admit::admits(
            self.unfinished_cnt.load(Ordering::Relaxed),
            self.frame_latency,
        );
        self.admitted
    }

    /// Writes the unfinished frames into the device's block (spec DEVICE),
    /// as a page's runtime does. It runs before each turn, after the queue's
    /// callbacks that end frames have run.
    pub(super) fn tell_unfinished(&self, memory: &mut Memory<'_>) {
        if let Some(address) = self.block {
            memory.store_release(
                address + 4 * device::UNFINISHED_AT as u32,
                self.unfinished_cnt.load(Ordering::Relaxed),
            );
        }
    }

    /// Shows each surface's texture that this turn drew. An admitted frame
    /// then counts as unfinished until the queue has done the work submitted
    /// with it.
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
