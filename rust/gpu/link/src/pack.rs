// pxl-gpu-link/pack.rs

//! How a host's staging belt packs a run's writes into its chunks, one rule
//! that the guest reads to know how many chunks its open stream needs
//! (pxl-gpu-guest stream.rs `stage`) and the device's host reads to place
//! them (pxl-gpu-host belt.rs), and that a page's host holds alike
//! (@pxl/gpu/host belt.ts). The writes staged since the host last submitted
//! begin at a fresh chunk. A buffer's bytes go as far as the chunk has room
//! and go on in the next, and a texture's rows, each padded to the copy's
//! row alignment, go whole, those that do not fit going on in the next
//! chunk. Each room begins at the spec's `ROOM_ALIGNMENT`, and a texture's
//! rows are padded to its `ROW_ALIGNMENT`.

use crate::spec::{ROOM_ALIGNMENT, ROW_ALIGNMENT};

/// Where the belt's next room goes, for chunks of `chunk_sz` bytes: the
/// first byte not yet taken of the chunk being written, none where the next
/// room begins a fresh chunk, and the chunks begun since `new`.
#[derive(Clone, Copy)]
pub struct Pack {
    chunk_sz: u64,
    at: Option<u64>,
    chunk_cnt: u32,
}

impl Pack {
    pub const fn new(chunk_sz: u64) -> Pack {
        Pack {
            chunk_sz,
            at: None,
            chunk_cnt: 0,
        }
    }

    /// The chunks begun since `new`.
    pub const fn chunk_cnt(&self) -> u32 {
        self.chunk_cnt
    }

    /// Whether the chunks begun are no more than `free_chunk_cnt`, the chunks
    /// the device's block says are free.
    pub const fn fits(&self, free_chunk_cnt: u32) -> bool {
        self.chunk_cnt <= free_chunk_cnt
    }

    /// The room for the next `want` bytes of a write, or for as many whole
    /// rows of `row_sz` bytes of them as the chunk being written has left:
    /// whether it begins a fresh chunk, the offset in the chunk and the byte
    /// count. A buffer's write is rows of one byte.
    pub fn take(&mut self, want: u64, row_sz: u64) -> (bool, u64, u64) {
        xpute_kit::ensure!(
            row_sz > 0 && row_sz <= self.chunk_sz,
            EINVAL,
            row_sz,
            self.chunk_sz
        );
        let (fresh, at) = match self.at {
            Some(at) if self.chunk_sz - at >= row_sz => (false, at),
            _ => {
                self.chunk_cnt += 1;
                (true, 0)
            }
        };
        let left = self.chunk_sz - at;
        let n = want.min(left - left % row_sz);
        let next = (at + n).next_multiple_of(u64::from(ROOM_ALIGNMENT));
        self.at = Some(next.min(self.chunk_sz));
        (fresh, at, n)
    }

    /// The host submits what was staged: the next room begins a fresh chunk.
    pub fn submit(&mut self) {
        self.at = None;
    }

    /// Lays out a buffer's write of `n` bytes.
    pub fn buffer(&mut self, n: u64) {
        let mut done = 0;
        while done < n {
            done += self.take(n - done, 1).2;
        }
    }

    /// Lays out a texture's write whose rows are `row_sz` bytes apart, of
    /// `size` texels: a row of texels for each row, which is a row of blocks
    /// for every format but the block-compressed, whose rows it over-counts.
    /// An empty `size` writes nothing.
    pub fn texture(&mut self, row_sz: u64, size: [u32; 3]) {
        if size.contains(&0) {
            return;
        }
        let stride = row_sz.next_multiple_of(u64::from(ROW_ALIGNMENT));
        let rows = u64::from(size[1]);
        for _ in 0..size[2] {
            let mut done = 0;
            while done < rows {
                done += self.take((rows - done) * stride, stride).2 / stride;
            }
        }
    }
}
