// pxl-render/gpu/ray/count.rs

//! Counts what the eye's cast and the light's walked and reused, while the
//! `ray_counts` setting is on; pxl-engine `count` lays the counts out. A
//! counted frame clears the counters before its passes run, and gathers what
//! they counted into one array after them. One readback of the array is out
//! at a time, and a frame that counts while one is out is not read. The
//! counts land a frame or two later, named by the renderer's number of the
//! frame that counted them.

use pxl_engine::{count, node_cache, raster, ray};
use xpute_core::mem::off_of;

use crate::gpu::api::{buffer_usage, bytes, signaled, word_bytes, Gpu, RESULT};

const WORDS: usize = count::WORDS as usize;

/// The counts of each observer, the eye and the light, in `RayCount::counts`
/// (`count_at`): whether it cast, 1 or 0; how many rays it has, the eye's
/// pixels or the light's slots; and, from `LANES_AT`, the lanes pxl-engine
/// `count` lays out.
pub const CAST: usize = 0;
pub const RAYS: usize = 1;
pub const LANES_AT: usize = 2;
const OBSERVER_COUNTS: usize = LANES_AT + count::LANES as usize;
pub const RAY_COUNTS: usize = count::OBSERVERS as usize * OBSERVER_COUNTS;

/// Where `observer`'s count `k` is.
pub const fn count_at(observer: u32, k: usize) -> usize {
    observer as usize * OBSERVER_COUNTS + k
}

/// One frame's counts, which land a frame or two after it. `frame` is the
/// renderer's number of that frame (renderer.rs `FrameStat::frame`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayCount {
    pub frame: f64,
    pub counts: [f64; RAY_COUNTS],
}

/// What the CPU knows of a counted frame as it encodes it, kept until its
/// counts land: the number the renderer will give the frame (renderer.rs
/// `FrameStat::frame`), and whether each observer cast and how many rays it
/// has.
#[derive(Clone, Copy, Default)]
pub(super) struct Frame {
    pub number: f64,
    pub casts: [bool; count::OBSERVERS as usize],
    pub rays: [u32; count::OBSERVERS as usize],
}

/// The buffers the passes count into. Every walk counts in the node cache's
/// head; the eye's pixels and the light's slots count in their misses' args
/// (`args`); the boxes each raster draws count in its draw's args (`boxes`).
pub(super) struct CountBuffer {
    pub node_cache: u32,
    pub args: [u32; count::OBSERVERS as usize],
    pub boxes: [u32; count::OBSERVERS as usize],
}

/// The counts' array on the GPU and its readback. One copy is out at a time: it is `copied`
/// from the frame that encodes it until the next frame asks to map it, then `mapped` until
/// the mapping lands. `copying` is what the CPU knew of the frame whose copy is out, and
/// `ray_counts` holds the counts that landed until they are taken.
pub(super) struct Counter {
    counts: u32,
    read: u32,
    on: bool,
    frame: Frame,
    copying: Frame,
    copied: bool,
    mapped: bool,
    read_counts: Box<[u32; WORDS]>,
    /// The runtime writes the mapping's fence and result here. The fence
    /// holds `ticket`, the number the last mapping was asked with, once that
    /// mapping lands.
    mapping: Box<[u32; 2]>,
    ticket: u32,
    ray_counts: Option<RayCount>,
}

impl Counter {
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
            copying: Frame::default(),
            copied: false,
            mapped: false,
            read_counts: Box::new([0; WORDS]),
            mapping: Box::new([0; 2]),
            ticket: 0,
            ray_counts: None,
        }
    }

    /// Starts this frame's counting, which runs while `on` and an observer
    /// casts. It tells the walks which observers count and clears their
    /// counts, both in the node cache's head, so it runs after the frame has
    /// written that head. Where neither this frame nor the one before
    /// counts, the head is left as it is.
    pub(super) fn begin(&mut self, g: &Gpu, on: bool, node_cache: u32, frame: Frame) {
        let was = self.on;
        self.on = on && frame.casts.contains(&true);
        self.frame = frame;
        if !self.on && !was {
            return;
        }
        // A bit for each observer that counts. The eye counts only in a full cast, so the
        // pixels a `px` frame casts again count nothing.
        let mut header = [0u32; 1 + (count::OBSERVERS * count::WALKS) as usize];
        for (o, &cast) in frame.casts.iter().enumerate() {
            header[0] |= u32::from(self.on && cast) << o;
        }
        g.write_buf(
            node_cache,
            word_bytes(node_cache::COUNT_ON as usize),
            bytes(&header),
        );
        if !self.on {
            return;
        }
        g.write_buf(self.counts, 0, bytes(&[0u32; WORDS]));
    }

    /// Gathers the counts of each observer that cast into the array and copies it out for
    /// the readback. It runs after the frame's dispatches, and only where this frame counts
    /// and no copy is out.
    pub(super) fn copy(&mut self, g: &Gpu, encoder: u32, from: &CountBuffer) {
        if !self.on || self.copied || self.mapped {
            return;
        }
        self.copied = true;
        for o in 0..count::OBSERVERS {
            let k = o as usize;
            if !self.frame.casts[k] {
                continue;
            }
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
                    count::VOXEL,
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
        self.copying = self.frame;
    }

    /// Keeps the counts whose mapping landed, and asks to map the copy the
    /// last frame made. It runs once the frame that encoded that copy was
    /// submitted, as a buffer maps only what a submitted copy wrote into it.
    pub(super) fn back(&mut self, g: &Gpu) {
        let signaled = signaled(&self.mapping[0], self.ticket);
        if self.mapped && signaled {
            self.mapped = false;
            if self.mapping[1] == RESULT::OK {
                let (l, f) = (&self.read_counts, self.copying);
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
                self.ray_counts = Some(RayCount {
                    frame: f.number,
                    counts,
                });
            }
        }
        if self.copied && !self.mapped && signaled {
            (self.copied, self.mapped) = (false, true);
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

    pub(super) fn take(&mut self) -> Option<RayCount> {
        self.ray_counts.take()
    }

    /// The frame that encoded the copy was dropped, so no copy is out.
    pub(super) fn on_dropped(&mut self) {
        self.copied = false;
    }
}
