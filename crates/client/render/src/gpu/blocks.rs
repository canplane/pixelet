// pxl-render/gpu/blocks.rs

//! The draw list's blocks on the device by the list's handle, and its
//! records by pipeline, as the renderer and its passes read them.

use xpute_guest::global::Global;
use xpute_guest::mem::base::ptr_at;

use crate::frame::draw::DRAW_WORDS;
use crate::frame::shader::record::{BLOCK_LAYOUT, PIPELINES};
use crate::gpu::api::{buffer_entry, buffer_usage, release, room, write_buffer_bytes, Gpu};

/// Records sorted by pipeline with a stable counting sort, so a pass walks
/// only its own records.
pub struct Draws<'a> {
    pub(crate) records: &'a [[u32; DRAW_WORDS]],
    order: &'a [u32],
    starts: [u32; PIPELINES as usize + 1],
}

impl<'a> Draws<'a> {
    pub(crate) fn sort(records: &'a [[u32; DRAW_WORDS]], order: &'a mut Vec<u32>) -> Draws<'a> {
        let mut starts = [0u32; PIPELINES as usize + 1];
        for d in records {
            if let Some(n) = starts.get_mut(d[4] as usize + 1) {
                *n += 1;
            }
        }
        for p in 0..PIPELINES as usize {
            starts[p + 1] += starts[p];
        }
        order.clear();
        order.resize(starts[PIPELINES as usize] as usize, 0);
        let mut next = starts;
        for (i, d) in records.iter().enumerate() {
            if let Some(at) = next.get_mut(d[4] as usize).filter(|_| d[4] < PIPELINES) {
                order[*at as usize] = i as u32;
                *at += 1;
            }
        }
        Draws { records, order, starts }
    }

    pub fn of(&self, pipeline: u32) -> impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]> + Clone + 'a {
        let p = (pipeline as usize).min(PIPELINES as usize);
        let end = if p < PIPELINES as usize { self.starts[p + 1] } else { self.starts[p] };
        let records = self.records;
        self.order[self.starts[p] as usize..end as usize].iter().map(move |&i| &records[i as usize])
    }
}

const EMPTY: u32 = 0;

/// A power of two so a position wraps by masking, at least twice `live` so
/// open addressing never fills past half.
pub const fn block_slots(live: usize) -> usize {
    (2 * live).next_power_of_two()
}

/// Blocks by the list's handle: open addressing in `.fixed.blocks`. The heap
/// is the source and the device a copy, so a lost device is restored from the
/// addresses.
pub struct Blocks {
    handle: &'static mut [u32],
    at: &'static mut [u32],
    len: &'static mut [u32],
    vertex_cnt: &'static mut [u32],
    index_cnt: &'static mut [u32],
    buffer: &'static mut [u32],
    group: &'static mut [u32],
    pub(crate) count: u32,
    pub(crate) bytes: u64,
    missing: u32,
    /// The layout every block's group is in (record.rs `BLOCK_LAYOUT`),
    /// made on the device as the first is grouped.
    layout: u32,
}

pub(crate) fn buffer_size(len: u32) -> u32 {
    ((len + 3) & !3).max(4)
}

fn memory(at: u32, n: u32) -> &'static [u8] {
    // SAFETY: a heap block the draw list named, live until its free is listed.
    unsafe { core::slice::from_raw_parts(ptr_at::<u8>(at as usize), n as usize) }
}

/// Handle, address, length, the two counts, buffer, group.
const BLOCK_LANES: usize = 7;

/// The table's lanes, then the draw list's (frame/draw.rs `lanes_bytes`),
/// room for the most blocks live, half the slots.
pub const fn blocks_bytes(slots: usize) -> u32 {
    (BLOCK_LANES * slots * 4) as u32 + crate::frame::draw::lanes_bytes(slots / 2)
}

/// # Safety
/// `at` must address `blocks_bytes(slots)`, 4-aligned and zeroed, the
/// renderer's alone for as long as the module runs.
pub unsafe fn set_blocks(at: u32, slots: usize) {
    xpute_kit::ensure!(slots.is_power_of_two(), EINVAL, slots);
    // SAFETY: the caller's range, one lane of `slots` words after another.
    let lane = |k: usize| unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize + k * slots * 4), slots) };
    BLOCKS.set(Blocks {
        handle: lane(0),
        at: lane(1),
        len: lane(2),
        vertex_cnt: lane(3),
        index_cnt: lane(4),
        buffer: lane(5),
        group: lane(6),
        count: 0,
        bytes: 0,
        missing: 0,
        layout: 0,
    });
    // SAFETY: the range's rest past the table, the lists' alone.
    unsafe { crate::frame::draw::set_lanes(at + (BLOCK_LANES * slots * 4) as u32, slots / 2) };
}

fn home(handle: u32, slots: usize) -> usize {
    (handle.wrapping_mul(0x9e37_79b1) >> (32 - slots.trailing_zeros())) as usize
}

impl Blocks {
    pub(crate) fn lose(&mut self) {
        self.buffer.fill(0);
        self.group.fill(0);
        self.missing = self.count;
        self.layout = 0;
    }

    /// As far as the turn's stream has room.
    pub(crate) fn restore(&mut self, g: &Gpu) {
        if self.missing == 0 {
            return;
        }
        for i in 0..self.handle.len() {
            if self.handle[i] != EMPTY && self.buffer[i] == 0 && !self.put(g, i) {
                return;
            }
        }
    }

    /// False, the block still missing, where the turn's stream has no room.
    fn put(&mut self, g: &Gpu, i: usize) -> bool {
        if !room(write_buffer_bytes(memory(self.at[i], self.len[i] & !3))) {
            return false;
        }
        let buffer = g.buffer(buffer_size(self.len[i]), buffer_usage::VERTEX | buffer_usage::INDEX | buffer_usage::STORAGE | buffer_usage::COPY_DST);
        g.write_buffer(buffer, 0, memory(self.at[i], self.len[i] & !3));
        self.buffer[i] = buffer;
        self.group[i] = 0;
        self.missing -= 1;
        true
    }

    fn find(&self, handle: u32) -> Option<usize> {
        if handle == EMPTY {
            return None;
        }
        let mut i = home(handle, self.handle.len());
        loop {
            match self.handle[i] {
                EMPTY => return None,
                h if h == handle => return Some(i),
                _ => i = (i + 1) & (self.handle.len() - 1),
            }
        }
    }

    /// None until the block is on the device.
    pub fn get(&self, handle: u32) -> Option<(u32, u32, u32)> {
        self.find(handle).filter(|&i| self.buffer[i] != 0).map(|i| (self.buffer[i], self.vertex_cnt[i], self.index_cnt[i]))
    }

    pub(crate) fn insert(&mut self, g: Option<&Gpu>, handle: u32, at: u32, len: u32, vertex_cnt: u32, index_cnt: u32) {
        self.remove(handle);
        // Slots are twice the most blocks the owners can hold, so this is that bound broken.
        xpute_kit::ensure!((self.count as usize) < self.handle.len() / 2, ENOTRECOVERABLE, self.handle.len() / 2);
        let mut i = home(handle, self.handle.len());
        while self.handle[i] != EMPTY {
            i = (i + 1) & (self.handle.len() - 1);
        }
        self.handle[i] = handle;
        self.at[i] = at;
        self.len[i] = len;
        self.vertex_cnt[i] = vertex_cnt;
        self.index_cnt[i] = index_cnt;
        self.buffer[i] = 0;
        self.group[i] = 0;
        self.count += 1;
        self.bytes += buffer_size(len) as u64;
        self.missing += 1;
        if let Some(g) = g {
            self.put(g, i);
        }
    }

    /// Where the turn's stream has no room, the buffer goes and the block is put
    /// again whole once there is.
    pub(crate) fn rewrite(&mut self, g: Option<&Gpu>, handle: u32, at: u32, from: u32, len: u32) {
        let Some(i) = self.find(handle) else { return };
        self.at[i] = at;
        let (Some(g), buffer @ 1..) = (g, self.buffer[i]) else { return };
        let bytes = len.min(self.len[i].saturating_sub(from)) & !3;
        if room(write_buffer_bytes(memory(at + from, bytes))) {
            g.write_buffer(buffer, from, memory(at + from, bytes));
        } else {
            release(self.group[i]);
            release(buffer);
            (self.buffer[i], self.group[i]) = (0, 0);
            self.missing += 1;
        }
    }

    pub(crate) fn remove(&mut self, handle: u32) {
        let Some(mut i) = self.find(handle) else { return };
        release(self.group[i]);
        if self.buffer[i] == 0 {
            self.missing = self.missing.saturating_sub(1);
        }
        release(self.buffer[i]);
        self.count -= 1;
        self.bytes -= buffer_size(self.len[i]) as u64;
        // Backward shift: an entry past the hole moves into it unless its home lies
        // between the hole and it, so no probe ever stops early.
        loop {
            let mut j = i;
            loop {
                j = (j + 1) & (self.handle.len() - 1);
                if self.handle[j] == EMPTY {
                    self.handle[i] = EMPTY;
                    return;
                }
                let k = home(self.handle[j], self.handle.len());
                let stays = if i <= j { i < k && k <= j } else { i < k || k <= j };
                if !stays {
                    self.handle[i] = self.handle[j];
                    self.at[i] = self.at[j];
                    self.len[i] = self.len[j];
                    self.vertex_cnt[i] = self.vertex_cnt[j];
                    self.index_cnt[i] = self.index_cnt[j];
                    self.buffer[i] = self.buffer[j];
                    self.group[i] = self.group[j];
                    i = j;
                    break;
                }
            }
        }
    }

    /// For a draw binding it as a vertex buffer: the ghost's boxes are read through it (box.wgsl).
    pub fn group(&mut self, g: &Gpu, handle: u32) -> Option<u32> {
        let i = self.find(handle).filter(|&i| self.buffer[i] != 0)?;
        if self.group[i] == 0 {
            if self.layout == 0 {
                self.layout = g.layout(&BLOCK_LAYOUT);
            }
            let id = g.bind_group(self.layout, &[buffer_entry(0, self.buffer[i])]);
            self.group[i] = id;
        }
        Some(self.group[i])
    }
}

static BLOCKS: Global<Blocks> = Global::new();

pub(crate) fn blocks() -> &'static mut Blocks {
    BLOCKS.get(|| xpute_kit::bug!(ENOTRECOVERABLE))
}
