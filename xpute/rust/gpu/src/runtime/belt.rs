// xpute-gpu/runtime/belt.rs

//! The staging belt that a core's buffer writes go through: a fixed pool of
//! chunks. The runtime writes into a mapped chunk, and the uploads' encoder
//! copies from it into the target buffer. Once submitted, a chunk is mapped
//! again, and it is free when that mapping resolves. The queue resolves the
//! mapping only after the work that copies from the chunk is done, so the
//! mapping is the chunk's own fence, as `on_submitted_work_done` is a
//! frame's. The pool holds a chunk for each frame that may be unfinished
//! (`Config::frame_latency`) and one for the turn being written, and it never
//! grows, however far the GPU lags.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Mapped, and holding nothing.
const FREE: u8 = 0;
/// Holding writes not yet submitted.
const OPEN: u8 = 1;
/// Submitted, and being mapped again.
const BACK: u8 = 2;
/// Its mapping was refused, because the device is gone.
const GONE: u8 = 3;

struct Chunk {
    buffer: wgpu::Buffer,
    /// Set by the mapping's callback, on whichever thread polls the device.
    state: Arc<AtomicU8>,
}

pub(super) struct Belt {
    chunks: Vec<Chunk>,
    chunk_bytes: u64,
    /// The chunk being written, and its first byte not yet taken.
    cur: Option<(usize, u64)>,
}

impl Belt {
    pub(super) fn new(device: &wgpu::Device, chunk_bytes: u64, chunk_cnt: u32) -> Belt {
        assert!(
            chunk_bytes > 0 && chunk_bytes.is_multiple_of(wgpu::MAP_ALIGNMENT),
            "[gpu] an upload chunk of {chunk_bytes} bytes"
        );
        let chunks = (0..chunk_cnt)
            .map(|_| Chunk {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("upload chunk"),
                    size: chunk_bytes,
                    usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: true,
                }),
                state: Arc::new(AtomicU8::new(FREE)),
            })
            .collect();
        Belt {
            chunks,
            chunk_bytes,
            cur: None,
        }
    }

    /// Room for the next `want` bytes of a write, or for as many of them as
    /// the current chunk has left: the chunk, the offset in it and the byte
    /// count. None where no chunk is free.
    pub(super) fn take(&mut self, want: u64) -> Option<(wgpu::Buffer, u64, u64)> {
        let (k, at) = match self.cur {
            Some((k, at)) if at < self.chunk_bytes => (k, at),
            _ => {
                let k = self
                    .chunks
                    .iter()
                    .position(|c| c.state.load(Ordering::Acquire) == FREE)?;
                self.chunks[k].state.store(OPEN, Ordering::Relaxed);
                (k, 0)
            }
        };
        let n = want.min(self.chunk_bytes - at);
        // Each mapped range begins at the mapping's alignment, so the next
        // piece does too.
        let next = (at + n).next_multiple_of(wgpu::MAP_ALIGNMENT);
        self.cur = Some((k, next.min(self.chunk_bytes)));
        Some((self.chunks[k].buffer.clone(), at, n))
    }

    /// Unmaps each chunk written. Call it before the uploads' encoder is
    /// submitted.
    pub(super) fn finish(&mut self) {
        for c in self.open() {
            c.buffer.unmap();
        }
    }

    /// Maps again each chunk the uploads' encoder copies from, to be free once
    /// the queue's work is done. Call it after that encoder is submitted.
    pub(super) fn recall(&mut self) {
        self.cur = None;
        for c in self.open() {
            c.state.store(BACK, Ordering::Relaxed);
            let state = c.state.clone();
            c.buffer.map_async(wgpu::MapMode::Write, .., move |r| {
                state.store(if r.is_ok() { FREE } else { GONE }, Ordering::Release);
            });
        }
    }

    /// Whether a chunk is on its way back, so that waiting for the queue
    /// frees one.
    pub(super) fn is_returning(&self) -> bool {
        self.chunks
            .iter()
            .any(|c| c.state.load(Ordering::Acquire) == BACK)
    }

    fn open(&self) -> impl Iterator<Item = &Chunk> {
        self.chunks
            .iter()
            .filter(|c| c.state.load(Ordering::Relaxed) == OPEN)
    }
}
