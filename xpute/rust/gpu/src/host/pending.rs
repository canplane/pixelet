// xpute-gpu/host/pending.rs

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use xpute_host::mem::Memory;

use crate::spec::RESULT;

/// A read or a done the device answers at a later poll.
pub(super) struct Pending {
    pub(super) state: Arc<AtomicU8>,
    pub(super) word: u32,
    pub(super) ticket: u32,
    /// A mapping's buffer, range and address: its done word has a result
    /// beside it, where a done's has none.
    pub(super) read: Option<(wgpu::Buffer, u64, u64, u32)>,
}

/// The request asked with `ticket` answered at the done word `word` (spec
/// RESULT): `result` into the word after it first, where the request has
/// one, then the ticket, with release.
pub(super) fn answer(memory: &mut Memory<'_>, word: u32, ticket: u32, result: Option<u32>) {
    if let Some(r) = result {
        memory.set_word(word + 4, r);
    }
    memory.store_release(word, ticket);
}

pub(super) const WAITING: u8 = 0;
pub(super) const ANSWERED: u8 = 1;
pub(super) const REFUSED: u8 = 2;

impl Pending {
    /// Its answer written into `memory`: whether it still waits.
    pub(super) fn poll(&self, memory: &mut Memory<'_>) -> bool {
        let state = self.state.load(Ordering::Acquire);
        if state == WAITING {
            return true;
        }
        let result = self.read.as_ref().map(|(buffer, offset, size, address)| {
            if state != ANSWERED {
                return RESULT::REFUSED;
            }
            let slice = buffer.slice(*offset..*offset + *size);
            let copied = slice.get_mapped_range().map(|range| {
                memory.write(*address as usize, &range);
            });
            buffer.unmap();
            if copied.is_ok() {
                RESULT::OK
            } else {
                RESULT::REFUSED
            }
        });
        answer(memory, self.word, self.ticket, result);
        false
    }
}
