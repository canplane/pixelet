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
/// cast and its pixels or slots.
#[derive(Clone, Copy, Default)]
pub(super) struct Frame {
    pub number: f64,
    pub casts: [bool; count::OBSERVERS as usize],
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

/// The array and its readback; whether this frame counts (`on`), what
/// the CPU knew of the frame copied (`copied_frame`), whether that copy
/// counted (`copied_on`) and carried the light's owed work (`copied_owed`),
/// and the counts that landed, until taken (`take`).
pub(super) struct Counter {
    counts: u32,
    read: u32,
    on: bool,
    frame: Frame,
    copied_frame: Frame,
    copied_on: bool,
    copied_owed: bool,
    /// Whether the light is owed a cast: what the last light cast's copy
    /// read back where it was not none, and set where a light cast's work is
    /// not read back (`copy`, `on_dropped`), so a cast whose owed work no
    /// copy carried is followed by another. Cleared by each light cast whose
    /// copy carries it, until that copy lands.
    owed: bool,
    /// Whether a light cast ran since the copy out, so what it reads back
    /// is not the light's last.
    cast_since: bool,
    read_counts: Box<[u32; WORDS]>,
    /// The mapping's fence and result, the host's, and the ticket it
    /// was last asked with, the kernel's: one mapping out at a time.
    mapping: Box<[u32; 2]>,
    ticket: u32,
    copied: bool,
    mapped: bool,
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
            copied_on: false,
            copied_owed: false,
            owed: false,
            cast_since: false,
            read_counts: Box::new([0; WORDS]),
            mapping: Box::new([0; 2]),
            ticket: 0,
            copied: false,
            mapped: false,
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
    /// frame counts, and the light's owed work where it cast. A light cast
    /// whose owed work is not copied leaves the light owed a cast.
    pub(super) fn copy(&mut self, g: &Gpu, encoder: u32, from: &Counts) {
        let light = self.frame.casts[count::LIGHT as usize];
        if light && (self.copied || self.mapped) {
            (self.owed, self.cast_since) = (true, true);
        }
        if !(self.on || light) || self.copied || self.mapped {
            return;
        }
        if light {
            g.copy_buf(
                encoder,
                (from.node_cache, word_bytes(node_cache::OWED as usize)),
                (self.counts, word_bytes(count::OWED as usize)),
                word_bytes(1),
            );
            (self.owed, self.cast_since) = (false, false);
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
        (self.copied, self.copied_frame) = (true, self.frame);
        (self.copied_on, self.copied_owed) = (self.on, light);
    }

    /// The counts copied last frame mapped back, and those that landed
    /// kept with what the CPU knew of their frame, until taken: after the
    /// frame that copied them was submitted.
    pub(super) fn back(&mut self, g: &Gpu) {
        let landed = signaled(&self.mapping[0], self.ticket);
        if self.mapped && landed && self.mapping[1] != RESULT::OK {
            self.mapped = false;
            self.owed |= self.copied_owed;
        }
        if self.mapped && landed && self.copied_owed && !self.cast_since {
            self.owed = self.read_counts[count::OWED as usize] != 0;
        }
        if self.mapped && landed && !self.copied_on {
            self.mapped = false;
        }
        if self.mapped && landed {
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
            self.mapped = false;
        }
        if self.copied && landed {
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
            (self.copied, self.mapped) = (false, true);
        }
    }

    /// Whether the light is owed a cast (`owed`).
    pub(super) fn owed(&self) -> bool {
        self.owed
    }

    /// Whether the light's owed work waits on the next frame to be mapped
    /// back (`back`); once mapped, its fence wakes the turn (`owed_landed`).
    pub(super) fn owed_out(&self) -> bool {
        self.copied_owed && self.copied
    }

    /// Whether the light's owed work landed for a frame to read (`back`).
    pub(super) fn owed_landed(&self) -> bool {
        self.copied_owed && self.mapped && signaled(&self.mapping[0], self.ticket)
    }

    /// The counts that landed since last taken.
    pub(super) fn take(&mut self) -> Option<RayCounts> {
        self.ray_counts.take()
    }

    /// The frame its copy was encoded in was dropped.
    pub(super) fn on_dropped(&mut self) {
        if self.copied {
            self.owed |= self.copied_owed;
        }
        self.copied = false;
    }
}
