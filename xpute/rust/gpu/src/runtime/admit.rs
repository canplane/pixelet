// xpute-gpu/runtime/admit.rs

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use xpute_runtime::mem::Memory;

use super::surface::Frame;
use super::GpuRuntime;
use crate::spec::DEVICE;

/// A `FrameGuard` is a frame drawn and not yet done. It stays counted until
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

/// The rule `admit_frame` keeps, for whoever asks beside the running stream.
pub(super) struct Admission {
    unfinished_cnt: Arc<AtomicU32>,
    frame_latency: u32,
}

impl Admission {
    pub(super) fn admits(&self) -> bool {
        crate::admit::admits(
            self.unfinished_cnt.load(Ordering::Relaxed),
            self.frame_latency,
        )
    }
}

impl GpuRuntime {
    pub(super) fn admission(&self) -> Admission {
        Admission {
            unfinished_cnt: self.unfinished_cnt.clone(),
            frame_latency: self.frame_latency,
        }
    }

    /// This says whether this turn may draw a frame. It may while fewer frames
    /// than the frame latency are unfinished.
    pub fn admit_frame(&self) -> bool {
        self.admission().admits()
    }

    /// Counts a frame the stream drew as unfinished until the queue has done
    /// the work submitted with it.
    pub(super) fn count_frame(&self) {
        self.queue
            .on_submitted_work_done(FrameGuard::count(&self.unfinished_cnt));
    }

    /// Writes the uncaptured errors and the unfinished frames into the
    /// device's block (spec DEVICE), as a page's runtime does. It runs before
    /// each turn, after the queue's callbacks that end frames have run.
    pub(super) fn tell_counts(&self, mem: &mut Memory<'_>) {
        if let Some(addr) = self.block {
            mem.store_release(
                addr + 4 * DEVICE::ERRORS_AT as u32,
                self.error_cnt.load(Ordering::Relaxed),
            );
            mem.store_release(
                addr + 4 * DEVICE::UNFINISHED_AT as u32,
                self.unfinished_cnt.load(Ordering::Relaxed),
            );
        }
    }

    /// Shows each surface's texture that this turn drew.
    pub fn present(&mut self) {
        for s in &mut self.surfaces {
            if let Some(Frame::Surface(frame)) = s.frame.take() {
                self.queue.present(frame);
            }
        }
    }
}
