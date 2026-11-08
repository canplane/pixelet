// pxl-render/gpu/read.rs

//! What a stage last read, so a frame that reads the same input keeps what the stage made.

use crate::gpu::api::{bytes, room, write_buffer_bytes, Gpu};

/// A stage's whole input as it last read it, byte for byte. What a stage makes is a function
/// of its input alone, so a frame that reads the same input keeps what the stage made: the
/// cast's rays and the decor generated over them (stage 1), and the frame (stage 2). Each lane
/// is read with its length, so two different sets of lanes never read alike. A change that has
/// no words to read is marked instead (`mark`).
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
        let changed = self.bytes.get(self.at..end) != Some(lane);
        if changed {
            self.same = false;
            if self.bytes.len() < end {
                self.bytes.resize(end, 0);
            }
            self.bytes[self.at..end].copy_from_slice(lane);
        }
        self.at = end;
        changed
    }

    /// Whether `lane`, read with its length, changed. Its writer writes it again only when it
    /// did.
    pub(crate) fn read<T: Copy>(&mut self, lane: &[T]) -> bool {
        self.take(bytes(&[lane.len() as u32])) | self.take(bytes(lane))
    }

    /// Whether everything read so far reads as it did the last time.
    pub(crate) fn same(&self) -> bool {
        self.same
    }

    /// Marks a change to the stage's input that has no words to read, such as an image that
    /// arrived or a table that was drawn.
    pub(crate) fn mark(&mut self, changed: bool) {
        self.same &= !changed;
    }

    /// Ends the read. Returns whether the whole input read as it did the last time.
    pub(crate) fn end(&mut self) -> bool {
        let same = self.same && self.at == self.bytes.len();
        self.bytes.truncate(self.at);
        same
    }

    /// Drops what was read, for when what the stage made is gone. The next frame reads as
    /// changed.
    pub(crate) fn forget(&mut self) {
        self.bytes.clear();
    }
}

/// Whether a buffer of `unit`-byte slots holds what was last written to it,
/// so a frame writes again only the slots its writer says changed.
#[derive(Default)]
pub(crate) struct SlotMirror {
    in_sync: bool,
    /// The changed slots, sorted. The vector is kept to reuse its allocation.
    slots: Vec<u32>,
}

impl SlotMirror {
    /// Writes `now` over `buf` from byte 0. When the buffer holds what was last written, only
    /// the runs of `unit`-byte slots that `changed` names and `now` holds are written.
    /// Otherwise, or when those runs do not fit the stream, the whole of `now` is written as
    /// one record however large. Returns whether anything was written.
    pub(crate) fn write(
        &mut self,
        g: &Gpu,
        buf: u32,
        now: &[u8],
        unit: usize,
        changed: impl Iterator<Item = u32>,
    ) -> bool {
        if self.in_sync {
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
        self.in_sync = true;
        true
    }

    /// Called when the buffer no longer holds what was written, so the next write is whole.
    pub(crate) fn forget(&mut self) {
        self.in_sync = false;
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
