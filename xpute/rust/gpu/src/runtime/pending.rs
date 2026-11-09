// xpute-gpu/runtime/pending.rs

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use xpute_runtime::mem::Memory;

use crate::spec::RESULT;

/// A `Pending` is a request the device answers at a later poll: a MAP_ASYNC
/// read, or a QUEUE_DONE.
pub(super) struct Pending {
    pub(super) state: Arc<AtomicU8>,
    pub(super) word: u32,
    pub(super) ticket: u32,
    /// This holds a read's buffer, offset, size and the address it lands at. A
    /// read's fence has a result word after it; the fence of a request with no
    /// read has none.
    pub(super) read: Option<(wgpu::Buffer, u64, u64, u32)>,
}

/// Answers the request asked with `ticket` at the fence `word` (spec
/// RESULT). Where the request has a result, it goes into the word after the
/// fence first; the ticket is then stored with release, so a core that sees
/// the ticket sees the result.
pub(super) fn signal(mem: &mut Memory<'_>, word: u32, ticket: u32, result: Option<u32>) {
    if let Some(r) = result {
        mem.set_word(word + 4, r);
    }
    mem.store_release(word, ticket);
}

pub(super) const WAITING: u8 = 0;
pub(super) const ANSWERED: u8 = 1;
pub(super) const REFUSED: u8 = 2;

impl Pending {
    /// Writes the answer into `mem` once there is one, and says whether
    /// the request still waits.
    pub(super) fn poll(&self, mem: &mut Memory<'_>) -> bool {
        let state = self.state.load(Ordering::Acquire);
        if state == WAITING {
            return true;
        }
        let result = self.read.as_ref().map(|(buffer, offset, size, addr)| {
            if state != ANSWERED {
                return RESULT::REFUSED;
            }
            let slice = buffer.slice(*offset..*offset + *size);
            let copied = slice.get_mapped_range().map(|range| {
                mem.write(*addr as usize, &range);
            });
            buffer.unmap();
            if copied.is_ok() {
                RESULT::OK
            } else {
                RESULT::REFUSED
            }
        });
        signal(mem, self.word, self.ticket, result);
        false
    }
}
