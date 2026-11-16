// pxl-gpu-host/belt.rs

//! This is the staging belt that a guest's buffer and texture writes go
//! through: a pool of chunks. The host writes into a mapped chunk, and the
//! uploads' encoder copies from it into the target. Once submitted, a chunk
//! is mapped again, and it is free when that mapping resolves. The queue
//! resolves the mapping only after the work that copies from the chunk is
//! done, so the mapping is the chunk's own fence, as `on_submitted_work_done`
//! is a frame's. The writes are packed as pxl-gpu-link `Pack` says.
//!
//! The pool holds one chunk more than the frame latency
//! (`Config::frame_latency`), as a page's does. The device's block tells the
//! guest how many of them are free, and the guest admits the writes that may
//! wait only as far as they hold (pxl-gpu-guest stream.rs `stage`). A write
//! that cannot wait is recorded however few are free, and where a run finds
//! none, the host makes one chunk more for it, mapped as it is made, and lets
//! it go once it is back, so the pool returns to its size; neither host waits
//! inside a record. A chunk the device cannot make holds the device lost, as
//! a refused mapping does.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use pxl_gpu_link::pack::Pack;
use pxl_gpu_link::spec::ROOM_ALIGNMENT;

/// The chunk is mapped, and holds nothing.
const FREE: u8 = 0;
/// The chunk holds writes not yet submitted.
const OPEN: u8 = 1;
/// The chunk is submitted, and is being mapped again.
const BACK: u8 = 2;
/// Its mapping was refused: the device is gone.
const GONE: u8 = 3;

struct Chunk {
    buffer: wgpu::Buffer,
    /// The mapping's callback sets this, on whichever thread polls the device.
    state: Arc<AtomicU8>,
}

pub(super) struct Belt {
    device: wgpu::Device,
    /// The pool's chunks first, then any made for a run that found none free.
    chunks: Vec<Chunk>,
    /// How many chunks the pool holds.
    pool_cnt: usize,
    chunk_sz: u64,
    /// Where the next room goes, since the last submit.
    pack: Pack,
    /// This is the chunk being written.
    cur: usize,
}

impl Belt {
    pub(super) fn new(device: &wgpu::Device, chunk_sz: u64, chunk_cnt: u32) -> Belt {
        assert!(
            chunk_sz > 0 && chunk_sz.is_multiple_of(u64::from(ROOM_ALIGNMENT)),
            "[gpu] an upload chunk of {chunk_sz} bytes"
        );
        let chunks = (0..chunk_cnt)
            .map(|_| Chunk {
                buffer: device.create_buffer(&chunk_desc(chunk_sz)),
                state: Arc::new(AtomicU8::new(FREE)),
            })
            .collect();
        Belt {
            device: device.clone(),
            chunks,
            pool_cnt: chunk_cnt as usize,
            chunk_sz,
            pack: Pack::new(chunk_sz),
            cur: 0,
        }
    }

    /// This answers the room for the next `want` bytes of a write, or for as
    /// many whole rows of `row_sz` bytes of them as the current chunk has left
    /// (`Pack::take`): the chunk, the offset in it and the byte count. A room
    /// that begins a fresh chunk takes a free chunk of the pool, or one made
    /// for it where none is free. It answers none where the device is gone:
    /// a chunk's mapping refused, or a chunk not made.
    pub(super) fn take(&mut self, want: u64, row_sz: u64) -> Option<(wgpu::Buffer, u64, u64)> {
        if self.is_gone() {
            return None;
        }
        let (fresh, at, n) = self.pack.take(want, row_sz);
        if fresh {
            self.let_go();
            let free = self.chunks[..self.pool_cnt]
                .iter()
                .position(|c| c.state.load(Ordering::Acquire) == FREE);
            self.cur = match free {
                Some(k) => k,
                None => {
                    let chunk = self.make_chunk()?;
                    self.chunks.push(chunk);
                    self.chunks.len() - 1
                }
            };
            self.chunks[self.cur].state.store(OPEN, Ordering::Relaxed);
        }
        Some((self.chunks[self.cur].buffer.clone(), at, n))
    }

    /// Unmaps each chunk written. Call it before the uploads' encoder is
    /// submitted.
    pub(super) fn finish(&mut self) {
        for c in self.open() {
            c.buffer.unmap();
        }
    }

    /// Maps again each chunk the uploads' encoder copies from, to be free once
    /// the queue's work is done, and begins the next room at a fresh chunk.
    /// Call it after that encoder is submitted.
    pub(super) fn recall(&mut self) {
        self.pack.submit();
        for c in self.open() {
            c.state.store(BACK, Ordering::Relaxed);
            let state = c.state.clone();
            c.buffer.map_async(wgpu::MapMode::Write, .., move |r| {
                state.store(if r.is_ok() { FREE } else { GONE }, Ordering::Release);
            });
        }
        self.let_go();
    }

    /// How many of the pool's chunks are free, which the device's block tells
    /// the guest (spec `DEVICE`).
    pub(super) fn free_cnt(&self) -> u32 {
        self.chunks[..self.pool_cnt]
            .iter()
            .filter(|c| c.state.load(Ordering::Acquire) == FREE)
            .count() as u32
    }

    /// Whether a chunk's mapping was refused, which says the device is gone
    /// (lib.rs `lost`): a refused mapping is a lost device (@pxl/gpu/host
    /// belt.ts `GONE`).
    pub(super) fn is_gone(&self) -> bool {
        self.chunks
            .iter()
            .any(|c| c.state.load(Ordering::Acquire) == GONE)
    }

    /// One chunk more, mapped as it is made, or none where the device cannot
    /// make it (an error its scope caught).
    fn make_chunk(&self) -> Option<Chunk> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let buffer = self.device.create_buffer(&chunk_desc(self.chunk_sz));
        // wgpu answers an error scope at once on a native backend.
        if pollster::block_on(scope.pop()).is_some() {
            buffer.destroy();
            return None;
        }
        Some(Chunk {
            buffer,
            state: Arc::new(AtomicU8::new(FREE)),
        })
    }

    /// Lets go each chunk past the pool that is back, so the pool returns to
    /// its size. Call it only where no room is being written.
    fn let_go(&mut self) {
        let mut k = self.pool_cnt;
        while k < self.chunks.len() {
            if self.chunks[k].state.load(Ordering::Acquire) == FREE {
                self.chunks.swap_remove(k).buffer.destroy();
            } else {
                k += 1;
            }
        }
    }

    fn open(&self) -> impl Iterator<Item = &Chunk> {
        self.chunks
            .iter()
            .filter(|c| c.state.load(Ordering::Relaxed) == OPEN)
    }
}

/// A chunk of `chunk_sz` bytes, mapped as it is made.
const fn chunk_desc(chunk_sz: u64) -> wgpu::BufferDescriptor<'static> {
    wgpu::BufferDescriptor {
        label: Some("upload chunk"),
        size: chunk_sz,
        usage: wgpu::BufferUsages::MAP_WRITE.union(wgpu::BufferUsages::COPY_SRC),
        mapped_at_creation: true,
    }
}
