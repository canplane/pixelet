// pxl-render/gpu/upload.rs

//! The draw list's uploads on the device by the list's handle, as the
//! renderer and its passes read them.

use xpute_core::global::Global;
use xpute_core::mem;

use crate::frame::shader::record::UPLOAD_LAYOUT;
use crate::gpu::api::{buf_entry, buffer_usage, release, room, write_buffer_bytes, Gpu, Layout};
use crate::gpu::draw;

const EMPTY: u32 = 0;

/// A power of two so a position wraps by masking, at least twice `live` so
/// open addressing never fills past half.
const fn slots_of(live: usize) -> usize {
    (2 * live).next_power_of_two()
}

/// Uploads by the list's handle: open addressing in `.fixed.uploads`. The heap
/// is the source and the device a copy, so a lost device is restored from the
/// addresses.
pub struct Uploads {
    handle: &'static mut [u32],
    at: &'static mut [u32],
    len: &'static mut [u32],
    vertex_cnt: &'static mut [u32],
    idx_cnt: &'static mut [u32],
    buf: &'static mut [u32],
    group: &'static mut [u32],
    /// The most the owners can hold live (pxl-core door/layout.rs `MAX_LIVE_UPLOADS`).
    live: u32,
    pub(crate) cnt: u32,
    pub(crate) bytes: u64,
    missing: u32,
    /// The layout every upload's group is in (record.rs `UPLOAD_LAYOUT`),
    /// made on the device as the first is grouped.
    layout: Option<Layout>,
}

pub(crate) fn buf_size(len: u32) -> u32 {
    ((len + 3) & !3).max(4)
}

fn mem(at: u32, n: u32) -> &'static [u8] {
    // SAFETY: a heap block the draw list named, live until its free is listed.
    unsafe { core::slice::from_raw_parts(mem::at::<u8>(at as usize), n as usize) }
}

/// Handle, address, length, the two counts, buffer, group.
const UPLOAD_COLUMNS: usize = 7;

/// The table's columns for `live` uploads, then the draw list's (gpu/draw.rs `lists_bytes`).
pub const fn uploads_bytes(live: usize) -> u32 {
    (UPLOAD_COLUMNS * slots_of(live) * 4) as u32 + draw::lists_bytes(live)
}

/// # Safety
/// `at` must address `uploads_bytes(live)`, 4-aligned and zeroed, the
/// renderer's alone for as long as the module runs.
pub unsafe fn set_uploads(at: u32, live: usize) {
    let slots = slots_of(live);
    // SAFETY: the caller's range, one column of `slots` words after another.
    let column = |k: usize| unsafe {
        core::slice::from_raw_parts_mut(mem::at::<u32>(at as usize + k * slots * 4), slots)
    };
    UPLOADS.set(Uploads {
        handle: column(0),
        at: column(1),
        len: column(2),
        vertex_cnt: column(3),
        idx_cnt: column(4),
        buf: column(5),
        group: column(6),
        live: live as u32,
        cnt: 0,
        bytes: 0,
        missing: 0,
        layout: None,
    });
    // SAFETY: the range's rest past the table, the lists' alone.
    unsafe { draw::set_lists(at as usize + UPLOAD_COLUMNS * slots * 4, live) };
}

fn home(handle: u32, slots: usize) -> usize {
    (handle.wrapping_mul(0x9e37_79b1) >> (32 - slots.trailing_zeros())) as usize
}

impl Uploads {
    pub(crate) fn lose(&mut self) {
        self.buf.fill(0);
        self.group.fill(0);
        self.missing = self.cnt;
        self.layout = None;
    }

    /// As far as the turn's stream has room.
    pub(crate) fn restore(&mut self, g: &Gpu) {
        if self.missing == 0 {
            return;
        }
        for i in 0..self.handle.len() {
            if self.handle[i] != EMPTY && self.buf[i] == 0 && !self.put(g, i) {
                return;
            }
        }
    }

    /// False, the upload still missing, where the turn's stream has no room.
    fn put(&mut self, g: &Gpu, i: usize) -> bool {
        if !room(write_buffer_bytes(mem(self.at[i], self.len[i] & !3))) {
            return false;
        }
        // Copied from too, a palette kept as it was drawn (gpu/pass/skinned_mesh.rs
        // `keep`).
        let buf = g.buf(
            buf_size(self.len[i]),
            buffer_usage::VERTEX
                | buffer_usage::INDEX
                | buffer_usage::STORAGE
                | buffer_usage::COPY_DST
                | buffer_usage::COPY_SRC,
        );
        g.write_buf(buf, 0, mem(self.at[i], self.len[i] & !3));
        self.buf[i] = buf;
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

    /// None until the upload is on the device.
    pub fn get(&self, handle: u32) -> Option<(u32, u32, u32)> {
        self.find(handle)
            .filter(|&i| self.buf[i] != 0)
            .map(|i| (self.buf[i], self.vertex_cnt[i], self.idx_cnt[i]))
    }

    pub(crate) fn insert(
        &mut self,
        g: Option<&Gpu>,
        handle: u32,
        at: u32,
        len: u32,
        vertex_cnt: u32,
        idx_cnt: u32,
    ) {
        self.remove(handle);
        // The owners' summed bound itself, not the slots' rounded half, so an owner left out of it shows.
        xpute_kit::ensure!(self.cnt < self.live, ENOTRECOVERABLE, self.live);
        let mut i = home(handle, self.handle.len());
        while self.handle[i] != EMPTY {
            i = (i + 1) & (self.handle.len() - 1);
        }
        self.handle[i] = handle;
        self.at[i] = at;
        self.len[i] = len;
        self.vertex_cnt[i] = vertex_cnt;
        self.idx_cnt[i] = idx_cnt;
        self.buf[i] = 0;
        self.group[i] = 0;
        self.cnt += 1;
        self.bytes += buf_size(len) as u64;
        self.missing += 1;
        if let Some(g) = g {
            self.put(g, i);
        }
    }

    /// Where the turn's stream has no room, the buffer goes and the upload is put
    /// again whole once there is.
    pub(crate) fn rewrite(&mut self, g: Option<&Gpu>, handle: u32, at: u32, from: u32, len: u32) {
        let Some(i) = self.find(handle) else { return };
        self.at[i] = at;
        let (Some(g), buf @ 1..) = (g, self.buf[i]) else {
            return;
        };
        let bytes = len.min(self.len[i].saturating_sub(from)) & !3;
        if room(write_buffer_bytes(mem(at + from, bytes))) {
            g.write_buf(buf, from, mem(at + from, bytes));
        } else {
            release(self.group[i]);
            release(buf);
            (self.buf[i], self.group[i]) = (0, 0);
            self.missing += 1;
        }
    }

    pub(crate) fn remove(&mut self, handle: u32) {
        let Some(mut i) = self.find(handle) else {
            return;
        };
        release(self.group[i]);
        if self.buf[i] == 0 {
            self.missing = self.missing.saturating_sub(1);
        }
        release(self.buf[i]);
        self.cnt -= 1;
        self.bytes -= buf_size(self.len[i]) as u64;
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
                let stays = if i <= j {
                    i < k && k <= j
                } else {
                    i < k || k <= j
                };
                if !stays {
                    self.handle[i] = self.handle[j];
                    self.at[i] = self.at[j];
                    self.len[i] = self.len[j];
                    self.vertex_cnt[i] = self.vertex_cnt[j];
                    self.idx_cnt[i] = self.idx_cnt[j];
                    self.buf[i] = self.buf[j];
                    self.group[i] = self.group[j];
                    i = j;
                    break;
                }
            }
        }
    }

    /// For a draw binding it as a vertex buffer: the ghost's boxes are read through it (box.wgsl).
    pub fn group(&mut self, g: &Gpu, handle: u32) -> Option<u32> {
        let i = self.find(handle).filter(|&i| self.buf[i] != 0)?;
        if self.group[i] == 0 {
            let layout = *self.layout.get_or_insert_with(|| g.layout(&UPLOAD_LAYOUT));
            let id = g.bind_group(
                layout,
                &[buf_entry(0, self.buf[i]), buf_entry(1, self.buf[i])],
            );
            self.group[i] = id;
        }
        Some(self.group[i])
    }

    /// The buffer `handle`'s bytes are on the device in, and its size in
    /// bytes, as `get` finds it: what a copy of it takes.
    pub fn buf_of(&self, handle: u32) -> Option<(u32, u32)> {
        self.find(handle)
            .filter(|&i| self.buf[i] != 0)
            .map(|i| (self.buf[i], buf_size(self.len[i])))
    }

    /// The group `group` made for `handle`, for a draw, which makes none.
    pub fn group_of(&self, handle: u32) -> Option<u32> {
        let i = self.find(handle)?;
        (self.group[i] != 0).then_some(self.group[i])
    }
}

static UPLOADS: Global<Uploads> = Global::new();

pub(crate) fn uploads() -> &'static mut Uploads {
    UPLOADS.get(|| xpute_kit::bug!(ENOTRECOVERABLE))
}
