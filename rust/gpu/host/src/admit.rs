// pxl-gpu-host/admit.rs

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use xpute_link::mem;

use super::GpuHost;
use super::surface::Frame;
use pxl_gpu_link::admit;
use pxl_gpu_link::spec::device;

/// A `FrameGuard` is a frame drawn and not yet done. It stays counted until
/// the queue's callback that holds it ends, either called once the work is done
/// or dropped uncalled, as wgpu drops a lost device's callbacks. The count is
/// taken off as the guard drops, so no frame stays counted forever.
pub(super) struct FrameGuard(Arc<AtomicU32>);

impl FrameGuard {
    /// Counts a frame on `count`, and returns the callback to hand the queue
    /// for it.
    pub(super) fn count(count: &Arc<AtomicU32>) -> impl FnOnce() + Send + use<> {
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

impl GpuHost {
    /// This says whether a run may draw a frame. It may while fewer frames
    /// than the frame latency are unfinished.
    pub fn admit_frame(&self) -> bool {
        admit::admits(
            self.unfinished_cnt.load(Ordering::Relaxed),
            self.frame_latency,
        )
    }

    /// Counts a frame the stream drew as unfinished until the queue has done
    /// the work submitted with it.
    pub(super) fn count_frame(&self) {
        self.queue
            .on_submitted_work_done(FrameGuard::count(&self.unfinished_cnt));
    }

    /// Writes the uncaptured errors and the unfinished frames into the
    /// device's block (spec `DEVICE`), as a page's host does. It runs at each
    /// `poll`, after the queue's callbacks that end frames have run.
    pub(super) fn tell_counts(&self) {
        if let Some(addr) = self.block {
            mem::store_release(
                addr + 4 * device::ERRORS_AT as u64,
                self.error_cnt.load(Ordering::Relaxed),
            );
            mem::store_release(
                addr + 4 * device::UNFINISHED_AT as u64,
                self.unfinished_cnt.load(Ordering::Relaxed),
            );
        }
    }

    /// Shows each surface's texture that the last run drew.
    pub fn present(&mut self) {
        for s in &mut self.surfaces {
            if let Some(Frame::Surface(frame)) = s.frame.take() {
                self.queue.present(frame);
            }
        }
    }
}
