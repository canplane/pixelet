// pxl-kernel/chunk/table/mount.rs

//! `.fixed.mounts`: where each mounted chunk sits, the free mounts and the index by chunk, as lanes the table alone holds.

use core::mem::size_of;

use xpute_guest::mem::section::span_of;

use crate::chunk::mesh_slot::OCCUPANCY;
use crate::mem::fixed::{lane, range, Range};

use crate::chunk::Chunk;

/// An index in a `.fixed.mounts` lane, stored plus one so a zeroed lane reads as none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub(super) struct Nth(u32);

impl Nth {
    pub(super) const NONE: Nth = Nth(0);

    pub(super) fn of(n: usize) -> Nth {
        Nth(n as u32 + 1)
    }

    pub(super) fn get(self) -> Option<usize> {
        self.0.checked_sub(1).map(|n| n as usize)
    }

    pub(super) fn is_none(self) -> bool {
        self == Nth::NONE
    }

    pub(super) fn take(&mut self) -> Option<usize> {
        core::mem::take(self).get()
    }
}

pub(super) struct Free {
    pub(super) lane: &'static mut [u32],
    pub(super) len: usize,
}

impl Free {
    pub(super) fn pop(&mut self) -> Option<u32> {
        self.len = self.len.checked_sub(1)?;
        Some(self.lane[self.len])
    }

    pub(super) fn push(&mut self, id: u32) {
        self.lane[self.len] = id;
        self.len += 1;
    }
}

pub(super) struct Index {
    pub(super) lane: &'static mut [(Chunk, u32)],
    pub(super) len: usize,
}

impl Index {
    pub(super) fn insert(&mut self, n: usize, entry: (Chunk, u32)) {
        self.lane.copy_within(n..self.len, n + 1);
        self.lane[n] = entry;
        self.len += 1;
    }

    pub(super) fn remove(&mut self, n: usize) -> (Chunk, u32) {
        let entry = self.lane[n];
        self.lane.copy_within(n + 1..self.len, n);
        self.len -= 1;
        entry
    }
}

impl core::ops::Deref for Index {
    type Target = [(Chunk, u32)];

    fn deref(&self) -> &[(Chunk, u32)] {
        &self.lane[..self.len]
    }
}

/// `.fixed.mounts` lanes: slot holder, slot next, mount place, free mounts, index.
pub(super) const MOUNT_LANES: [u32; 5] = [
    (OCCUPANCY * size_of::<Nth>()) as u32,
    (OCCUPANCY * size_of::<Nth>()) as u32,
    (OCCUPANCY * size_of::<Nth>()) as u32,
    (OCCUPANCY * size_of::<u32>()) as u32,
    (OCCUPANCY * size_of::<(Chunk, u32)>()) as u32,
];

pub(super) const MOUNT_ALIGN: u32 = 8;

pub const MOUNTS_BYTES: u32 = span_of(&MOUNT_LANES, MOUNT_ALIGN);

/// # Safety
/// Each lane is taken once, by the table alone; zero is valid for every entry.
pub(super) unsafe fn mount_lane<T>(k: usize) -> &'static mut [T] {
    unsafe { lane(range(Range::Mounts).nth(&MOUNT_LANES, MOUNT_ALIGN, k), 0, OCCUPANCY) }
}
