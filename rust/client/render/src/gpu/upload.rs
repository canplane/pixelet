// pxl-render/gpu/upload.rs

//! The draw list's uploads on the device, found by the list's handle, as the renderer and its
//! passes read them.

use pxl_global::Global;
use xpute_kit::abi::word::{join64, split64};
use xpute_link::mem;

use crate::frame::shader::record::UPLOAD_LAYOUT;
use crate::gpu::api::{Gpu, Layout, buf_entry, buffer_usage, need, note_retry, release, room};
use crate::gpu::draw;

const EMPTY: u32 = 0;

/// A power of two so a position wraps by masking, at least twice `live` so
/// open addressing never fills past half.
const fn slots_of(live: usize) -> usize {
    (2 * live).next_power_of_two()
}

/// The uploads, found by the list's handle in an open-addressed table in the uploads' fixed
/// range.
/// The heap holds the bytes and the device holds a copy, so a lost device is restored from the
/// addresses.
pub struct UploadTable {
    handle: &'static mut [u32],
    /// Each upload's address, two words a slot, its low word first.
    at: &'static mut [u32],
    len: &'static mut [u32],
    vertex_cnt: &'static mut [u32],
    idx_cnt: &'static mut [u32],
    buf: &'static mut [u32],
    /// How many of the upload's bytes its buffer holds: all once the upload is on the device,
    /// fewer while it is put a chunk at a time (`put`).
    resident_sz: &'static mut [u32],
    group: &'static mut [u32],
    /// The most the owners can hold live (pxl-core link/layout.rs `LIVE_UPLOAD_CAP`).
    live: u32,
    pub(crate) cnt: u32,
    pub(crate) bytes: u64,
    missing: u32,
    /// The layout every upload's group is in (record.rs `UPLOAD_LAYOUT`), made on the device
    /// as the first is asked for (`layout`).
    layout: Option<Layout>,
}

pub(crate) fn buf_size(len: u32) -> u32 {
    ((len + 3) & !3).max(4)
}

fn mem(at: u64, n: u32) -> &'static [u8] {
    // SAFETY: a heap block the draw list named, live until its free is listed.
    unsafe { mem::bytes(at, n) }
}

/// The table's columns, in words a slot: handle, address (two), length, the vertex and index
/// counts, buffer, the bytes written and group.
const UPLOAD_COLUMN_CNT: usize = 9;

/// The table's columns for `live` uploads, then the draw list's (gpu/draw.rs `lists_sz`).
pub const fn uploads_sz(live: usize) -> u32 {
    (UPLOAD_COLUMN_CNT * slots_of(live) * 4) as u32 + draw::lists_sz(live)
}

/// Lays the table for `live` uploads, then the draw list's, in `room`, `uploads_sz(live)`
/// zeroed, the renderer's alone for as long as the core runs.
pub fn set_uploads(room: &'static mut [u32], live: usize) {
    let slots = slots_of(live);
    let mut room = room;
    let mut column = |words: usize| {
        let (c, rest) = core::mem::take(&mut room).split_at_mut(words * slots);
        room = rest;
        c
    };
    UPLOADS.set(UploadTable {
        handle: column(1),
        at: column(2),
        len: column(1),
        vertex_cnt: column(1),
        idx_cnt: column(1),
        buf: column(1),
        resident_sz: column(1),
        group: column(1),
        live: live as u32,
        cnt: 0,
        bytes: 0,
        missing: 0,
        layout: None,
    });
    draw::set_lists(room, live);
}

fn home(handle: u32, slot_cnt: usize) -> usize {
    (handle.wrapping_mul(0x9e37_79b1) >> (32 - slot_cnt.trailing_zeros())) as usize
}

impl UploadTable {
    /// Upload `i`'s address.
    fn addr(&self, i: usize) -> u64 {
        join64(self.at[2 * i], self.at[2 * i + 1])
    }

    fn set_addr(&mut self, i: usize, at: u64) {
        self.at[2 * i..2 * i + 2].copy_from_slice(&split64(at));
    }

    /// Whether upload `i` is on the device whole, which is when a reader may name its buffer.
    fn is_resident(&self, i: usize) -> bool {
        self.buf[i] != 0 && self.resident_sz[i] == self.len[i] & !3
    }

    pub(crate) fn lose(&mut self) {
        self.buf.fill(0);
        self.resident_sz.fill(0);
        self.group.fill(0);
        self.missing = self.cnt;
        self.layout = None;
    }

    /// Puts the missing uploads back on the device, as far as the stream and the belt have
    /// room.
    pub(crate) fn restore(&mut self, g: &Gpu) {
        if self.missing == 0 {
            return;
        }
        for i in 0..self.handle.len() {
            if self.handle[i] != EMPTY && !self.is_resident(i) && !self.put(g, i) {
                return;
            }
        }
    }

    /// Puts upload `i` on the device, a write that may wait (`Gpu::write_buf_from`): as much
    /// of it as the stream and the belt have room for, going on from there the next time.
    /// Returns whether it is on the device whole; until then no reader names its buffer
    /// (`is_resident`).
    fn put(&mut self, g: &Gpu, i: usize) -> bool {
        if self.buf[i] == 0 {
            // The buffer is also copied from, so the marks keep each upload they read as the
            // last frame drew it (pass/mark.rs `CopyCache::keep`).
            self.buf[i] = g.buf(
                buf_size(self.len[i]),
                buffer_usage::VERTEX
                    | buffer_usage::INDEX
                    | buffer_usage::STORAGE
                    | buffer_usage::COPY_DST
                    | buffer_usage::COPY_SRC,
            );
            (self.resident_sz[i], self.group[i]) = (0, 0);
        }
        let bytes = mem(self.addr(i), self.len[i] & !3);
        self.resident_sz[i] =
            g.write_buf_from(self.buf[i], 0, bytes, self.resident_sz[i] as usize) as u32;
        if !self.is_resident(i) {
            return false;
        }
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

    /// The upload's buffer and its vertex and index counts, none until the upload is on the
    /// device.
    pub fn get(&self, handle: u32) -> Option<(u32, u32, u32)> {
        self.find(handle)
            .filter(|&i| self.is_resident(i))
            .map(|i| (self.buf[i], self.vertex_cnt[i], self.idx_cnt[i]))
    }

    pub(crate) fn insert(
        &mut self,
        g: Option<&Gpu>,
        handle: u32,
        at: u64,
        len: u32,
        vertex_cnt: u32,
        idx_cnt: u32,
    ) {
        self.remove(handle);
        // The check is against the owners' summed bound itself, not half the rounded slots, so
        // an owner left out of that bound is caught.
        xpute_kit::ensure!(self.cnt < self.live, ENOTRECOVERABLE, self.live);
        let mut i = home(handle, self.handle.len());
        while self.handle[i] != EMPTY {
            i = (i + 1) & (self.handle.len() - 1);
        }
        self.handle[i] = handle;
        self.set_addr(i, at);
        self.len[i] = len;
        self.vertex_cnt[i] = vertex_cnt;
        self.idx_cnt[i] = idx_cnt;
        self.buf[i] = 0;
        self.resident_sz[i] = 0;
        self.group[i] = 0;
        self.cnt += 1;
        self.bytes += buf_size(len) as u64;
        self.missing += 1;
        if let Some(g) = g {
            self.put(g, i);
        }
    }

    /// Writes the changed range of `handle`'s upload. When the frame's stream has no room, the
    /// buffer is released and the upload is put again whole once there is room. An upload
    /// still being put is put again from its start.
    pub(crate) fn rewrite(&mut self, g: Option<&Gpu>, handle: u32, at: u64, from: u32, len: u32) {
        let Some(i) = self.find(handle) else { return };
        self.set_addr(i, at);
        if !self.is_resident(i) {
            self.resident_sz[i] = 0;
            return;
        }
        let (Some(g), buf) = (g, self.buf[i]) else {
            return;
        };
        let bytes = len.min(self.len[i].saturating_sub(from)) & !3;
        let range = need().write_buf(mem(at + u64::from(from), bytes));
        if room(range) {
            g.write_buf(buf, from, mem(at + u64::from(from), bytes));
        } else {
            note_retry(range);
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
        if !self.is_resident(i) {
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
                    let at = self.addr(j);
                    self.set_addr(i, at);
                    self.len[i] = self.len[j];
                    self.vertex_cnt[i] = self.vertex_cnt[j];
                    self.idx_cnt[i] = self.idx_cnt[j];
                    self.buf[i] = self.buf[j];
                    self.resident_sz[i] = self.resident_sz[j];
                    self.group[i] = self.group[j];
                    i = j;
                    break;
                }
            }
        }
    }

    /// The bind group over `handle`'s buffer, made the first time it is asked for. The decor's
    /// and the impostors' instances, the characters' palettes and the zones' boxes are read
    /// through it.
    pub fn group(&mut self, g: &Gpu, handle: u32) -> Option<u32> {
        let i = self.find(handle).filter(|&i| self.is_resident(i))?;
        if self.group[i] == 0 {
            let id = g.bind_group(
                self.layout(g),
                &[buf_entry(0, self.buf[i]), buf_entry(1, self.buf[i])],
            );
            self.group[i] = id;
        }
        Some(self.group[i])
    }

    /// The layout every upload's group is in, made the first time it is asked for. A copy of
    /// an upload is grouped in it too (pass/mark.rs `CopyCache`).
    pub(crate) fn layout(&mut self, g: &Gpu) -> Layout {
        *self.layout.get_or_insert_with(|| g.layout(&UPLOAD_LAYOUT))
    }

    /// The buffer that holds `handle`'s bytes on the device, and its size in bytes, which is
    /// what a copy of it needs.
    pub fn buf_of(&self, handle: u32) -> Option<(u32, u32)> {
        self.find(handle)
            .filter(|&i| self.is_resident(i))
            .map(|i| (self.buf[i], buf_size(self.len[i])))
    }

    /// The group that `group` made for `handle`, for a draw, which cannot make one.
    pub fn group_of(&self, handle: u32) -> Option<u32> {
        let i = self.find(handle)?;
        (self.group[i] != 0).then_some(self.group[i])
    }
}

static UPLOADS: Global<UploadTable> = Global::new();

pub(crate) fn uploads() -> &'static mut UploadTable {
    UPLOADS.get(|| xpute_kit::bug!(ENOTRECOVERABLE))
}
