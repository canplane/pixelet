// pxl-render/gpu/ray/count.rs

//! What the eye's cast and the light's walked and reused, counted where the
//! `ray_counts` setting asks (pxl-engine `count`): where each pass
//! counts cleared each counted frame, gathered into one array after the
//! frame's dispatches where no mapping of it is out (`copied`), mapped back
//! (`mapped`), and handed on as it lands with what the CPU knew of the
//! frame that counted it (`Frame`), by the renderer's number of that frame
//! (`RayCounts`), as a GPU read is. Every light cast, counted or not, the
//! light's owed work rides the same copy (`count::OWED`), and what lands
//! tells whether the light is owed a cast (`owed`).

use pxl_engine::{count, node_cache, raster, ray};
use xpute_guest::mem::off_of;

use crate::gpu::api::{buffer_usage, bytes, signaled, word_bytes, Gpu, RESULT};

const WORDS: usize = count::WORDS as usize;

/// A counted frame's counts, by index, an observer's each (`count_at`):
/// whether it cast, 1 or 0 (`CAST`), its pixels or slots (`RAYS`), and its
/// lanes (pxl-engine `count`, from `LANES_AT`).
pub const CAST: usize = 0;
pub const RAYS: usize = 1;
pub const LANES_AT: usize = 2;
const OBSERVER_COUNTS: usize = LANES_AT + count::LANES as usize;
pub const RAY_COUNTS: usize = count::OBSERVERS as usize * OBSERVER_COUNTS;

/// Where `observer`'s count `k` is.
pub const fn count_at(observer: u32, k: usize) -> usize {
    observer as usize * OBSERVER_COUNTS + k
}

/// One frame's counts as they land, a frame or two after it: the renderer's
/// number of it (renderer.rs `FrameStat::frame`) and its counts
/// (`count_at`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayCounts {
    pub frame: f64,
    pub counts: [f64; RAY_COUNTS],
}

/// What the CPU knew of a counted frame: the renderer's number it will
/// take (renderer.rs `FrameStat::frame`), and an observer's each, whether it
/// cast, whether its holes' queue ran, its cast or its queue alone (`queues`),
/// and its pixels or slots.
#[derive(Clone, Copy, Default)]
pub(super) struct Frame {
    pub number: f64,
    pub casts: [bool; count::OBSERVERS as usize],
    pub queues: [bool; count::OBSERVERS as usize],
    pub rays: [u32; count::OBSERVERS as usize],
}

/// Where the passes counted, an observer's each: the walks' in the observed
/// node cache's head (`node_cache`), the pixels' or slots' in their walk again's
/// args (`args`), the boxes in the raster's draw (`boxes`).
pub(super) struct Counts {
    pub node_cache: u32,
    pub args: [u32; count::OBSERVERS as usize],
    pub boxes: [u32; count::OBSERVERS as usize],
}

/// The observers, as the readback keeps them (pxl-engine `count::OBSERVERS`).
const OBSERVERS: usize = count::OBSERVERS as usize;

/// The one readback's slot and each observer's owed work as the CPU knows them, a pure state
/// its frames step (`copy`, `ask`, `land`, `dropped`, `off`) and read (`wants_frame`, `wakes`),
/// so every case is tested on the CPU (count.test.rs). One copy is out at a time, from the
/// frame that encodes it (`copied`) to the mapping the next frame asks (`mapped`) and its
/// landing. An observer's queue run, the eye's cast or its queue alone, the light's cast, is a
/// generation of its own (`cast`), and its owed work (`count::OWED`) rides the copy its frame
/// makes, named by that generation (`carried`); where none is made, the observer stays owed
/// (`owed`). A copy of an older generation than an observer's last never clears what a newer
/// run owes. So every run's work is read back or owed, and either asks for the frame that goes
/// on with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Readback {
    /// A copy encoded, its mapping asked by the next frame.
    pub copied: bool,
    /// Its mapping out, until it lands.
    pub mapped: bool,
    /// Whether the copy out holds the lanes, the `ray_counts` setting's.
    pub counted: bool,
    /// Each observer's generation: its queue's runs, and each time it is let go, so far.
    pub cast: [u32; OBSERVERS],
    /// The generation whose owed work the copy out holds, an observer's each, if any.
    pub carried: [Option<u32>; OBSERVERS],
    /// Whether each observer is owed a run of its queue.
    pub owed: [bool; OBSERVERS],
}

impl Readback {
    /// After a frame's dispatches, where it counted its lanes (`counted`) or an observer's
    /// queue ran (`ran`), a generation of its own each: whether its counts are copied, which
    /// they are where none are out. A run whose work is copied owes nothing until it lands; one
    /// whose work is not stays owed.
    pub fn copy(&mut self, counted: bool, ran: [bool; OBSERVERS]) -> bool {
        let copy = (counted || ran.contains(&true)) && !self.copied && !self.mapped;
        if copy {
            (self.copied, self.counted) = (true, counted);
        }
        for (o, &ran) in ran.iter().enumerate() {
            if ran {
                self.cast[o] = self.cast[o].wrapping_add(1);
                self.owed[o] = !copy;
            }
            if copy {
                self.carried[o] = ran.then_some(self.cast[o]);
            }
        }
        copy
    }

    /// As a frame begins, the copy the frame before made asked to be mapped, the mapping
    /// before it landed (`signaled`): whether to ask.
    pub fn ask(&mut self, signaled: bool) -> bool {
        let ask = self.copied && !self.mapped && signaled;
        if ask {
            (self.copied, self.mapped) = (false, true);
        }
        ask
    }

    /// Whether the copy out holds observer `o`'s owed work of its last generation.
    fn current(&self, o: usize) -> bool {
        self.carried[o] == Some(self.cast[o])
    }

    /// The mapping out landed, refused where not `ok`, each observer's owed work read as
    /// `owed`: an observer whose last generation it holds is owed where that owes work or the
    /// mapping was refused; an older one's reading changes nothing. Whether its lanes are
    /// taken.
    pub fn land(&mut self, ok: bool, owed: [u32; OBSERVERS]) -> bool {
        self.mapped = false;
        for (o, &owed) in owed.iter().enumerate() {
            if self.current(o) {
                self.owed[o] = !ok || owed != 0;
            }
        }
        ok && self.counted
    }

    /// The frame dropped, its copy with it: what that copy held owed again.
    pub fn dropped(&mut self) {
        if self.copied {
            self.copied = false;
            for o in 0..OBSERVERS {
                self.owed[o] |= self.carried[o].is_some();
            }
        }
    }

    /// Observer `o` let go, the light's cast released or no world to cast in: a generation of
    /// its own, owing nothing, as its next run casts afresh.
    pub fn off(&mut self, o: usize) {
        self.cast[o] = self.cast[o].wrapping_add(1);
        self.owed[o] = false;
    }

    /// Whether the frame after this one must run: a copy holding an observer's last owed work
    /// waits on it to be mapped, or an observer that may run (`on`) is owed a run.
    pub fn wants_frame(&self, on: [bool; OBSERVERS]) -> bool {
        (0..OBSERVERS).any(|o| (self.copied && self.current(o)) || (on[o] && self.owed[o]))
    }

    /// Whether the mapping out, which holds an observer's last owed work, landed for a frame to
    /// read (`signaled`), its fence's store having woken the turn.
    pub fn wakes(&self, signaled: bool) -> bool {
        self.mapped && signaled && (0..OBSERVERS).any(|o| self.current(o))
    }
}

/// The array and its readback (`Readback`); whether this frame counts (`on`), what the CPU
/// knew of the frame copied (`copied_frame`), and the counts that landed, until taken
/// (`take`).
pub(super) struct Counter {
    counts: u32,
    read: u32,
    on: bool,
    frame: Frame,
    copied_frame: Frame,
    slot: Readback,
    read_counts: Box<[u32; WORDS]>,
    /// The mapping's fence and result, the host's, and the ticket it
    /// was last asked with, the kernel's: one mapping out at a time.
    mapping: Box<[u32; 2]>,
    ticket: u32,
    ray_counts: Option<RayCounts>,
}

impl Counter {
    /// Its buffers, made with the cast's: a few words.
    pub(super) fn new(g: &Gpu) -> Counter {
        Counter {
            counts: g.buf(
                word_bytes(WORDS),
                buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
            ),
            read: g.buf(
                word_bytes(WORDS),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            ),
            on: false,
            frame: Frame::default(),
            copied_frame: Frame::default(),
            slot: Readback::default(),
            read_counts: Box::new([0; WORDS]),
            mapping: Box::new([0; 2]),
            ticket: 0,
            ray_counts: None,
        }
    }

    /// This frame's counting where `on` and an observer casts (`frame`):
    /// the walks told to count in the node cache's head (`node_cache`) and their
    /// counts cleared there, after the head is written, and the array
    /// cleared.
    /// The head is left as it is where neither this frame nor the one
    /// before counts.
    pub(super) fn begin(&mut self, g: &Gpu, on: bool, node_cache: u32, frame: Frame) {
        let was = self.on;
        self.on = on && frame.casts.contains(&true);
        self.frame = frame;
        if !self.on && !was {
            return;
        }
        let mut head = [0u32; 1 + (count::OBSERVERS * count::WALKS) as usize];
        head[0] = u32::from(self.on);
        g.write_buf(
            node_cache,
            word_bytes(node_cache::COUNT_ON as usize),
            bytes(&head),
        );
        if !self.on {
            return;
        }
        g.write_buf(self.counts, 0, bytes(&[0u32; WORDS]));
    }

    /// The counts gathered into the array and copied where none are out,
    /// after the frame's dispatches: each observer's that cast where the
    /// frame counts, and each observer's owed work where its queue ran
    /// (`Readback::copy`).
    pub(super) fn copy(&mut self, g: &Gpu, encoder: u32, from: &Counts) {
        let ran = self.frame.queues;
        if !self.slot.copy(self.on, ran) {
            return;
        }
        for o in 0..count::OBSERVERS {
            if ran[o as usize] {
                g.copy_buf(
                    encoder,
                    (from.node_cache, word_bytes(node_cache::owed_at(o) as usize)),
                    (self.counts, word_bytes((count::OWED + o) as usize)),
                    word_bytes(1),
                );
            }
        }
        for o in 0..count::OBSERVERS {
            let k = o as usize;
            if !self.on || !self.frame.casts[k] {
                continue;
            }
            // Each run where it was counted, into its lanes.
            for (buf, word, lane, words) in [
                (
                    from.node_cache,
                    node_cache::lane_at(o, count::BEAM),
                    count::BEAM,
                    count::WALKS,
                ),
                (
                    from.args[k],
                    ray::MISS_ARGS_COUNT,
                    count::REUSED,
                    count::RAYS,
                ),
                (from.boxes[k], raster::ARGS_INSTANCES, count::BOXES, 1),
            ] {
                let to = word_bytes(count::at(o, lane) as usize);
                let size = word_bytes(words as usize);
                g.copy_buf(
                    encoder,
                    (buf, word_bytes(word as usize)),
                    (self.counts, to),
                    size,
                );
            }
        }
        g.copy_buf(encoder, (self.counts, 0), (self.read, 0), word_bytes(WORDS));
        self.copied_frame = self.frame;
    }

    /// The counts copied last frame mapped back, and those that landed
    /// kept with what the CPU knew of their frame, until taken: after the
    /// frame that copied them was submitted.
    pub(super) fn back(&mut self, g: &Gpu) {
        let signaled = signaled(&self.mapping[0], self.ticket);
        if self.slot.mapped && signaled {
            let ok = self.mapping[1] == RESULT::OK;
            let owed =
                [count::EYE, count::LIGHT].map(|o| self.read_counts[(count::OWED + o) as usize]);
            if self.slot.land(ok, owed) {
                let (l, f) = (&self.read_counts, self.copied_frame);
                let mut counts = [0.0; RAY_COUNTS];
                for o in 0..count::OBSERVERS {
                    if !f.casts[o as usize] {
                        continue;
                    }
                    counts[count_at(o, CAST)] = 1.0;
                    counts[count_at(o, RAYS)] = f64::from(f.rays[o as usize]);
                    for lane in 0..count::LANES {
                        let c = f64::from(l[count::at(o, lane) as usize]);
                        counts[count_at(o, LANES_AT + lane as usize)] = c;
                    }
                }
                self.ray_counts = Some(RayCounts {
                    frame: f.number,
                    counts,
                });
            }
        }
        if self.slot.ask(signaled) {
            let addr = off_of(self.read_counts.as_ptr()) as u32;
            self.ticket = self.ticket.wrapping_add(1);
            g.map_read(
                self.read,
                0,
                word_bytes(WORDS),
                addr,
                &self.mapping,
                self.ticket,
            );
        }
    }

    /// Whether observer `o` is owed a run of its queue (`Readback::owed`).
    pub(super) fn owed(&self, o: u32) -> bool {
        self.slot.owed[o as usize]
    }

    /// Whether the frame after this one must run for an observer's owed work, each observer
    /// that may run `on` (`Readback::wants_frame`).
    pub(super) fn wants_frame(&self, on: [bool; OBSERVERS]) -> bool {
        self.slot.wants_frame(on)
    }

    /// Whether an observer's owed work landed for a frame to read (`Readback::wakes`).
    pub(super) fn owed_landed(&self) -> bool {
        self.slot.wakes(signaled(&self.mapping[0], self.ticket))
    }

    /// Observer `o` let go (`Readback::off`).
    pub(super) fn off(&mut self, o: u32) {
        self.slot.off(o as usize);
    }

    /// The counts that landed since last taken.
    pub(super) fn take(&mut self) -> Option<RayCounts> {
        self.ray_counts.take()
    }

    /// The frame its copy was encoded in was dropped.
    pub(super) fn on_dropped(&mut self) {
        self.slot.dropped();
    }
}

#[cfg(test)]
#[path = "count.test.rs"]
mod test;
