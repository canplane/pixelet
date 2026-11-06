// pxl-kernel/domain/layer/upload.rs

//! What of a layer's tiles goes to the GPU off the packet it came in: per
//! slot, the packet the GPU does not hold yet, kept until the turn's stream
//! takes it and freed then, and the device it went to; a slot taken again
//! replaces it. A packet is freed unkept past the layer's bound (`keep`),
//! and freed unsent past a full stream once nothing wants it (`send`).

use xpute_guest::mem::heap::Block;

use crate::sys::io::fetch::{body, release};

#[derive(Clone, Copy)]
enum Upload {
    Empty,
    Waiting(Block),
    /// Sent to the device of this generation (`device_generation`).
    Sent(u32),
}

/// What became of a packet offered to the turn's stream.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Uptake {
    /// Taken whole: freed.
    Taken,
    /// Taken in part, the rest a later turn's, and the next offered.
    Part,
    /// The stream ran out: it waits, and nothing more is offered this turn.
    Full,
}

pub struct Uploads {
    slots: Vec<Upload>,
    /// The slots `Waiting`.
    waiting: u32,
}

impl Uploads {
    pub fn new(slots: u32) -> Uploads {
        Uploads {
            slots: vec![Upload::Empty; slots as usize],
            waiting: 0,
        }
    }

    /// The packet a tile came in, kept for the slot `slot` took it into
    /// while fewer than `most` are kept besides; freed where no slot took
    /// it or `most` are, the slot then holding nothing to send.
    pub fn keep(&mut self, slot: Option<u32>, block: Block, most: u32) {
        let Some(slot) = slot else {
            return release(Some(block));
        };
        let slot = &mut self.slots[slot as usize];
        match *slot {
            Upload::Waiting(was) => release(Some(was)),
            _ if self.waiting >= most => {
                *slot = Upload::Empty;
                return release(Some(block));
            }
            _ => self.waiting += 1,
        }
        *slot = Upload::Waiting(block);
    }

    /// Slot `slot`'s packet, if one is kept, freed, leaving the slot
    /// nothing to send.
    pub fn forget(&mut self, slot: u32) {
        let slot = &mut self.slots[slot as usize];
        if let Upload::Waiting(was) = core::mem::replace(slot, Upload::Empty) {
            release(Some(was));
            self.waiting -= 1;
        }
    }

    /// The packets waiting.
    pub fn cnt(&self) -> u32 {
        self.waiting
    }

    /// Whether slot `slot`'s packet is still to be sent.
    pub fn waiting(&self, slot: u32) -> bool {
        matches!(self.slots[slot as usize], Upload::Waiting(_))
    }

    /// The device slot `slot`'s tile was sent to, once it was.
    pub fn sent_on(&self, slot: u32) -> Option<u32> {
        match self.slots[slot as usize] {
            Upload::Sent(on) => Some(on),
            _ => None,
        }
    }

    /// Each waiting packet to `send` with its slot, freed once taken whole
    /// by the device `on`, until the stream runs out; past that, each one
    /// nothing wants any more (`wanted`) is freed unsent, so the packets
    /// given back do not wait on the stream when the slots are short.
    pub fn send(
        &mut self,
        on: u32,
        send: &mut dyn FnMut(u32, &[u8]) -> Uptake,
        wanted: &dyn Fn(u32) -> bool,
    ) {
        if self.waiting == 0 {
            return;
        }
        let mut full = false;
        for slot in 0..self.slots.len() {
            let Upload::Waiting(block) = self.slots[slot] else {
                continue;
            };
            if full {
                if !wanted(slot as u32) {
                    release(Some(block));
                    self.slots[slot] = Upload::Empty;
                    self.waiting -= 1;
                }
                continue;
            }
            match send(slot as u32, body(block)) {
                Uptake::Full => full = true,
                Uptake::Part => {}
                Uptake::Taken => {
                    release(Some(block));
                    self.slots[slot] = Upload::Sent(on);
                    self.waiting -= 1;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "upload.test.rs"]
mod test;
