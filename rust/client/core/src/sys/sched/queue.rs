// pxl-core/sys/sched/queue.rs

//! When the GPU runs out of work, as the frames' measured times foresee it. A frame handed over
//! while another runs waits behind it and shows an input that much older. So a frame that can
//! wait begins at the last turn that still reaches the GPU before it goes idle: the GPU stays
//! as busy, and a frame waits at most a turn behind the one it follows. A time measured too
//! short begins the frame sooner, which costs only the wait. One measured too long leaves the
//! GPU idle until the first turn that finds it empty.

#[derive(Clone, Copy, Default)]
pub struct Queue {
    /// When the GPU is foreseen to have finished every frame it was handed, on the core's clock.
    idle_at: f64,
    /// The GPU's time for the frame last measured.
    busy_ms: f64,
    /// The core's time for the frame last handed over.
    encode_ms: f64,
}

impl Queue {
    /// Whether a frame begun at `now` is due, with `unfinished` frames on the GPU and turns
    /// `paced_ms` apart. It is due where the next turn would begin it too late.
    pub fn due(&self, now: f64, unfinished: u32, paced_ms: f64) -> bool {
        unfinished == 0 || self.idle_at - now <= paced_ms + self.encode_ms
    }

    pub fn measure(&mut self, busy_ms: f64) {
        self.busy_ms = busy_ms;
    }

    /// A frame was handed to the GPU at `now`, after `encode_ms` on the core.
    pub fn submit(&mut self, now: f64, encode_ms: f64) {
        self.idle_at = self.idle_at.max(now) + self.busy_ms;
        self.encode_ms = encode_ms;
    }
}

#[cfg(test)]
#[path = "queue.test.rs"]
mod test;
