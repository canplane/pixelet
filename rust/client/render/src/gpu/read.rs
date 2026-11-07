// pxl-render/gpu/read.rs

//! What a stage last read, to tell a frame that reads the same.

use crate::gpu::api::{bytes, room, write_buffer_bytes, Gpu};

/// A stage's whole input as it last read it, byte for byte: what the stage
/// makes is one function of it, so a frame that reads the same stands on
/// what it made — the cast's rays and the decor generated over them (stage 1),
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

    /// Whether all it read so far read as the last time.
    pub(crate) fn same(&self) -> bool {
        self.same
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

/// Whether a buffer of `unit`-byte slots holds what was last written to it,
/// so a frame writes again only the slots its writer says changed.
#[derive(Default)]
pub(crate) struct Written {
    held: bool,
    /// The changed slots, sorted, kept for their room.
    slots: Vec<u32>,
}

impl Written {
    /// `now` written over `buf` from 0: where the buffer holds what was last
    /// written, only the runs of the `unit`-byte slots `changed` names that
    /// `now` holds; else, or where those runs do not fit the stream, the
    /// whole of it, one record however large. Whether anything was written.
    pub(crate) fn write(
        &mut self,
        g: &Gpu,
        buf: u32,
        now: &[u8],
        unit: usize,
        changed: impl Iterator<Item = u32>,
    ) -> bool {
        if self.held {
            let n = (now.len() / unit) as u32;
            self.slots.clear();
            self.slots.extend(changed.filter(|&s| s < n));
            self.slots.sort_unstable();
            self.slots.dedup();
            if self.slots.is_empty() {
                return false;
            }
            let need: usize = runs(&self.slots)
                .map(|(a, b)| write_buffer_bytes(&now[a as usize * unit..b as usize * unit]))
                .sum();
            if room(need) {
                for (a, b) in runs(&self.slots) {
                    let (a, b) = (a as usize * unit, b as usize * unit);
                    g.write_buf(buf, a as u32, &now[a..b]);
                }
                return true;
            }
        }
        g.upload_buf(buf, 0, now);
        self.held = true;
        true
    }

    /// The buffer no longer holds what was written: the next write is whole.
    pub(crate) fn forget(&mut self) {
        self.held = false;
    }
}

/// The runs of sorted `slots`, each from its first to past its last.
fn runs(slots: &[u32]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let mut k = 0;
    core::iter::from_fn(move || {
        let first = *slots.get(k)?;
        let mut end = first + 1;
        k += 1;
        while slots.get(k) == Some(&end) {
            end += 1;
            k += 1;
        }
        Some((first, end))
    })
}
