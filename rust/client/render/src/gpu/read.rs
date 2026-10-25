// pxl-render/gpu/read.rs

//! What a stage last read, to tell a frame that reads the same.

use crate::gpu::api::bytes;

/// A stage's whole input as it last read it, byte for byte: what the stage
/// makes is one function of it, so a frame that reads the same stands on
/// what it made — the cast's rays and the decor grown over them (stage 1),
/// the frame (stage 2). Each lane is read with its length, so no two
/// sets read alike; what changed without words to read is marked so.
#[derive(Default)]
pub(crate) struct Read {
    bytes: Vec<u8>,
    at: usize,
    same: bool,
}

impl Read {
    pub(crate) fn begin(&mut self) {
        (self.at, self.same) = (0, true);
    }

    /// Whether `lane` reads otherwise than it did at this place last time.
    pub(crate) fn take(&mut self, lane: &[u8]) -> bool {
        let end = self.at + lane.len();
        let moved = self.bytes.get(self.at..end) != Some(lane);
        if moved {
            self.same = false;
            if self.bytes.len() < end {
                self.bytes.resize(end, 0);
            }
            self.bytes[self.at..end].copy_from_slice(lane);
        }
        self.at = end;
        moved
    }

    /// Whether `lane`, with its length, moved: what its writer needs to
    /// write again, and no more.
    pub(crate) fn read<T: Copy>(&mut self, lane: &[T]) -> bool {
        self.take(bytes(&[lane.len() as u32])) | self.take(bytes(lane))
    }

    /// What this frame changed of the stage's input that has no words, as
    /// an image come or a table drawn.
    pub(crate) fn mark(&mut self, changed: bool) {
        self.same &= !changed;
    }

    /// Whether all of it read as the last time.
    pub(crate) fn end(&mut self) -> bool {
        let same = self.same && self.at == self.bytes.len();
        self.bytes.truncate(self.at);
        same
    }

    /// The rays no longer hold what was read: the next frame casts.
    pub(crate) fn forget(&mut self) {
        self.bytes.clear();
    }
}
