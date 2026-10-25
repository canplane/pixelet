// xpute-gpu/host/admit.rs

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::surface::Frame;
use super::GpuHost;

/// A frame admitted and not yet done, counted until the queue's callback
/// that holds it ends: called once the work is done, or let go uncalled, as
/// wgpu lets a gone device's callbacks go. Taken off as it drops, so a frame
/// never stands forever.
pub(super) struct Unfinished(Arc<AtomicU32>);

impl Unfinished {
    /// `count`'s next frame, and the callback the queue is handed for it.
    pub(super) fn count(count: &Arc<AtomicU32>) -> impl FnOnce() + Send + 'static {
        count.fetch_add(1, Ordering::Relaxed);
        let frame = Unfinished(count.clone());
        move || drop(frame)
    }
}

impl Drop for Unfinished {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl GpuHost {
    /// Whether this turn may draw a frame: while fewer than its frame
    /// latency are unfinished, so each surface's next texture is free and
    /// taking it does not block. Asked once a turn, before it draws.
    pub fn admit(&mut self) -> bool {
        self.admitted = self.unfinished.load(Ordering::Relaxed) < self.frame_latency;
        self.admitted
    }

    /// Each surface's texture this turn drew, shown; an admitted frame
    /// unfinished until the queue has done the work submitted with it.
    pub fn present(&mut self) {
        for s in &mut self.surfaces {
            if let Some(Frame::Shown(frame)) = s.frame.take() {
                self.queue.present(frame);
            }
        }
        if core::mem::take(&mut self.admitted) {
            self.queue.on_submitted_work_done(Unfinished::count(&self.unfinished));
        }
    }
}
