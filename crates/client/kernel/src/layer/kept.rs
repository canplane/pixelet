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

pub struct Kept(Vec<Frame>);

impl Kept {
    pub fn new(frames: u32) -> Kept {
        Kept(vec![Frame::Empty; frames as usize])
    }

    /// The packet a tile came in, kept for the frame `record` took it into,
    /// or freed where none did.
    pub fn keep(&mut self, record: Option<u32>, block: Block) {
        let Some(record) = record else { return release(Some(block)) };
        if let Frame::Waiting(was) = core::mem::replace(&mut self.0[record as usize], Frame::Waiting(block)) {
            release(Some(was));
        }
    }

    /// Whether frame `record`'s packet is still to be sent.
    pub fn waiting(&self, record: u32) -> bool {
        matches!(self.0[record as usize], Frame::Waiting(_))
    }

    /// The device frame `record`'s tile was sent to, once it was.
    pub fn sent_on(&self, record: u32) -> Option<u32> {
        match self.0[record as usize] {
            Frame::Sent(on) => Some(on),
            _ => None,
        }
    }

    /// Each waiting packet to `send` with its frame, freed once taken whole
    /// by the device `on`, until the stream runs out.
    pub fn send(&mut self, on: u32, send: &mut dyn FnMut(u32, &[u8]) -> Offered) {
        for (record, frame) in self.0.iter_mut().enumerate() {
            let Frame::Waiting(block) = *frame else { continue };
            match send(record as u32, body(block)) {
                Offered::Full => return,
                Offered::Kept => {}
                Offered::Taken => {
                    release(Some(block));
                    *frame = Frame::Sent(on);
                }
            }
        }
    }
}
