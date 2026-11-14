// xpute-gpu-native-host/pending.rs

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use xpute_link::mem;

use super::timed;
use xpute_gpu_link::spec::result;

/// A `Pending` is a request the device answers at a later poll: a `MAP_ASYNC`
/// read, or a `QUEUE_DONE`.
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
/// `RESULT`). Where the request has a result, it goes into the word after the
/// fence first; the ticket is then stored with release, so a guest that sees
/// the ticket sees the result.
pub(super) fn signal(word: u32, ticket: u32, result: Option<u32>) {
    if let Some(r) = result {
        // SAFETY: the result word the guest lent with the fence, which it
        // reads only once the ticket is stored.
        unsafe { mem::set_word(word + 4, r) };
    }
    mem::store_release(word, ticket);
}

pub(super) const WAITING: u8 = 0;
pub(super) const ANSWERED: u8 = 1;
pub(super) const REFUSED: u8 = 2;

impl Pending {
    /// Writes the answer into the guest's memory once there is one, and says
    /// whether the request still waits. A read's landing slower than `slow` is
    /// said as `map` (`timed`), as a page's is.
    pub(super) fn poll(&self, slow: Option<u32>) -> bool {
        let state = self.state.load(Ordering::Acquire);
        if state == WAITING {
            return true;
        }
        let result = self.read.as_ref().map(|(buffer, offset, size, addr)| {
            if state != ANSWERED {
                return result::REFUSED;
            }
            let copied = timed(slow, "map", || {
                let slice = buffer.slice(*offset..*offset + *size);
                let copied = slice.get_mapped_range().map(|range| {
                    // SAFETY: where the guest lent the read to land, which it
                    // reads only once the ticket is stored.
                    unsafe { mem::write(*addr, &range) };
                });
                buffer.unmap();
                copied
            });
            if copied.is_ok() {
                result::OK
            } else {
                result::REFUSED
            }
        });
        signal(self.word, self.ticket, result);
        false
    }
}
