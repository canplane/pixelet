// pxl-core/domain/layer/upload.rs

//! The packets a layer's tiles arrived in, kept until the GPU copies the
//! tiles from them. For each slot it holds the packet the GPU does not have
//! yet, or the device the packet went to. A packet is freed once the turn's
//! stream takes it, and a slot that takes a new tile replaces its packet.
//! Past the layer's bound, a packet is freed without being kept (`keep`).

use xpute_core::mem::heap::Block;

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
    /// The stream took the packet, which is then freed.
    Taken,
    /// The stream has no room left. The packet waits, and nothing more is
    /// offered this turn.
    Full,
}

pub struct Uploads {
    slots: Vec<Upload>,
    /// The number of slots in `Waiting`.
    waiting: u32,
}

impl Uploads {
    pub fn new(slots: u32) -> Uploads {
        Uploads {
            slots: vec![Upload::Empty; slots as usize],
            waiting: 0,
        }
    }

    /// Keeps the packet a tile arrived in for slot `slot`, which took the
    /// tile, while fewer than `most` other packets are kept. Where no slot
    /// took the tile, or `most` packets are kept already, the packet is freed
    /// and the slot has nothing to send.
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

    /// The device generation slot `slot`'s tile was sent to, or none until it
    /// is sent.
    pub fn sent_on(&self, slot: u32) -> Option<u32> {
        match self.slots[slot as usize] {
            Upload::Sent(on) => Some(on),
            _ => None,
        }
    }

    /// Offers each waiting packet and its slot to `send` for the device of
    /// generation `on`, until the stream runs out. A packet the stream takes
    /// is freed.
    pub fn send(&mut self, on: u32, send: &mut dyn FnMut(u32, &[u8]) -> Uptake) {
        for slot in 0..self.slots.len() {
            let Upload::Waiting(block) = self.slots[slot] else {
                continue;
            };
            match send(slot as u32, body(block)) {
                Uptake::Full => return,
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
