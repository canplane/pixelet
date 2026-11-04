// xpute-gpu/host/pending.rs

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use xpute_host::mem::Memory;

use crate::spec::DONE;

/// A read or a done the device answers at a later poll.
pub(super) struct Pending {
    pub(super) state: Arc<AtomicU8>,
    pub(super) word: u32,
    pub(super) ticket: u32,
    /// A mapping's buffer, range and address: its word is written whatever
    /// it holds, where a done's only while its ticket stands.
    pub(super) read: Option<(wgpu::Buffer, u64, u64, u32)>,
}

pub(super) const WAITING: u8 = 0;
pub(super) const ANSWERED: u8 = 1;
pub(super) const REFUSED: u8 = 2;

impl Pending {
    /// Its answer written into `memory`: whether it still waits.
    pub(super) fn poll(&self, memory: &mut Memory<'_>) -> bool {
        match self.state.load(Ordering::Acquire) {
            WAITING => true,
            ANSWERED => {
                if let Some((buffer, offset, size, address)) = &self.read {
                    let slice = buffer.slice(*offset..*offset + *size);
                    if let Ok(range) = slice.get_mapped_range() {
                        memory.write(*address as usize, &range);
                    }
                    buffer.unmap();
                    memory.set_word(self.word, self.ticket | DONE);
                } else if memory.word(self.word) == self.ticket {
                    memory.set_word(self.word, self.ticket | DONE);
                }
                false
            }
            _ => false,
        }
    }
}
