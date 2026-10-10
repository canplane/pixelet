// pxl-kernel/chunk/decor/instance.rs

//! A chunk's decor instances as lanes, and the block they are laid into.
//! `hidden` is a flag, so what the eraser is over stays in its block.

use std::collections::TryReserveError;

pub const HIDDEN_FLAG: u8 = 1;

pub use pxl_render::gpu::pass::decor::GROUPS as DECOR_GROUPS;

/// Grows rather than being a `.fixed` range: placed instances are unbounded,
/// and a fixed workspace would drop what someone placed. Where the heap
/// refuses, the cell is not built.
pub struct DecorScratch {
    pub(crate) instances: Vec<crate::layer::instance::Instance>,
    pub(crate) candidates: Vec<pxl_spec::procedural::Candidate>,
    pub(crate) groups: Vec<u16>,
    pub(crate) counts: [u32; DECOR_GROUPS],
    pub(crate) group_count: usize,
}

/// A single kind's scatter is thousands; growing a few at a time would ask
/// the heap every handful.
const MIN_GROWTH: usize = 1024;

impl Default for DecorScratch {
    fn default() -> DecorScratch {
        DecorScratch {
            instances: Vec::new(),
            candidates: Vec::new(),
            groups: Vec::new(),
            counts: [0; DECOR_GROUPS],
            group_count: 0,
        }
    }
}

impl DecorScratch {
    pub(crate) fn clear(&mut self) {
        self.instances.clear();
        self.groups.clear();
        self.counts = [0; DECOR_GROUPS];
        self.group_count = 0;
    }

    /// Doubles when full, so a build that fits earlier growth asks for nothing.
    pub(crate) fn push(&mut self, group: usize, instance: crate::layer::instance::Instance) -> Result<(), TryReserveError> {
        if self.instances.len() == self.instances.capacity() || self.groups.len() == self.groups.capacity() {
            let more = self.instances.len().max(MIN_GROWTH);
            self.instances.try_reserve(more)?;
            self.groups.try_reserve(more)?;
        }
        self.instances.push(instance);
        self.groups.push(group as u16);
        self.counts[group] += 1;
        Ok(())
    }
}

/// A block: group count, each group's instance count, then each group's
/// lanes: five f32 lanes (x, y, z, spin, scale) and byte flags padded to a word.
#[derive(Clone, Copy)]
pub struct GroupLanes {
    pub first: usize,
    pub count: usize,
}

impl GroupLanes {
    pub const fn words(count: usize) -> usize {
        5 * count + count.div_ceil(4)
    }

    /// Lanes: 0 x, 1 y, 2 z, 3 spin, 4 scale.
    pub const fn f32_at(self, lane: usize, i: usize) -> usize {
        self.first + lane * self.count + i
    }

    pub const fn flags(self) -> usize {
        self.first + 5 * self.count
    }
}

/// Each group's first lane word and count, from the block's header.
pub fn decor_groups(words: &[u32]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let groups = words.first().map_or(0, |&g| g as usize);
    let mut at = 1 + groups;
    words.get(1..1 + groups).unwrap_or(&[]).iter().map(move |&n| {
        let (first, n) = (at, n as usize);
        at += GroupLanes::words(n);
        (first as u32, n as u32)
    })
}
