// pxl-kernel/layer/kept.rs

//! What of a layer's tiles goes to the GPU off the packet it came in: per
//! frame, the packet the GPU does not hold yet, kept until the turn's stream
//! takes it and freed then, and the device it went to; a frame taken again
//! replaces it.

use xpute_guest::mem::heap::Block;

use crate::io::fetch::{body, release};

#[derive(Clone, Copy)]
enum Frame {
    Empty,
    Waiting(Block),
    /// Sent to the device of this generation (`device_generation`).
    Sent(u32),
}

/// What became of a packet offered to the turn's stream.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Offered {
    /// Taken whole: freed.
    Taken,
    /// Taken in part, the rest a later turn's: kept, and the next offered.
    Kept,
    /// The stream ran out: kept, and nothing more offered this turn.
    Full,
}

pub struct Kept {
    frames: Vec<Frame>,
    /// The frames `Waiting`.
    waiting: u32,
}

impl Kept {
    pub fn new(frames: u32) -> Kept {
        Kept {
            frames: vec![Frame::Empty; frames as usize],
            waiting: 0,
        }
    }

    /// The packet a tile came in, kept for the frame `record` took it into
    /// while fewer than `most` are kept besides; freed where no frame took
    /// it or `most` are, the frame then holding nothing to send.
    pub fn keep(&mut self, record: Option<u32>, block: Block, most: u32) {
        let Some(record) = record else { return release(Some(block)) };
        let frame = &mut self.frames[record as usize];
        match *frame {
            Frame::Waiting(was) => release(Some(was)),
            _ if self.waiting >= most => {
                *frame = Frame::Empty;
                return release(Some(block));
            }
            _ => self.waiting += 1,
        }
        *frame = Frame::Waiting(block);
    }

    /// The packets kept.
    pub fn count(&self) -> u32 {
        self.waiting
    }

    /// Whether frame `record`'s packet is still to be sent.
    pub fn waiting(&self, record: u32) -> bool {
        matches!(self.frames[record as usize], Frame::Waiting(_))
    }

    /// The device frame `record`'s tile was sent to, once it was.
    pub fn sent_on(&self, record: u32) -> Option<u32> {
        match self.frames[record as usize] {
            Frame::Sent(on) => Some(on),
            _ => None,
        }
    }

    /// Each waiting packet to `send` with its frame, freed once taken whole
    /// by the device `on`, until the stream runs out.
    pub fn send(&mut self, on: u32, send: &mut dyn FnMut(u32, &[u8]) -> Offered) {
        for (record, frame) in self.frames.iter_mut().enumerate() {
            let Frame::Waiting(block) = *frame else { continue };
            match send(record as u32, body(block)) {
                Offered::Full => return,
                Offered::Kept => {}
                Offered::Taken => {
                    release(Some(block));
                    *frame = Frame::Sent(on);
                    self.waiting -= 1;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "kept.test.rs"]
mod test;
