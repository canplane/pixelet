// pxl-kernel/layer/kept.rs

//! What of a layer's tiles goes to the GPU off the packet it came in: per
//! frame, the packet the GPU does not hold yet, kept until the turn's stream
//! takes it and freed then; a frame taken again replaces it.

use xpute_guest::mem::heap::Block;

use crate::io::fetch::{body, release};

pub struct Kept(Vec<Option<Block>>);

impl Kept {
    pub fn new(frames: u32) -> Kept {
        Kept(vec![None; frames as usize])
    }

    /// The packet a tile came in, kept for the frame `record` took it into,
    /// or freed where none did.
    pub fn keep(&mut self, record: Option<u32>, block: Block) {
        match record {
            Some(record) => release(self.0[record as usize].replace(block)),
            None => release(Some(block)),
        }
    }

    /// Whether the GPU does not hold frame `record`'s tile yet.
    pub fn waiting(&self, record: u32) -> bool {
        self.0[record as usize].is_some()
    }

    /// Each waiting packet to `send` with its frame, freed once sent, while
    /// `send` takes them.
    pub fn send(&mut self, send: &mut dyn FnMut(u32, &[u8]) -> bool) {
        for (record, slot) in self.0.iter_mut().enumerate() {
            let Some(block) = *slot else { continue };
            if !send(record as u32, body(block)) {
                return;
            }
            release(slot.take());
        }
    }
}
