// pxl-kernel/world/chunk/table/mesh.rs

//! The mesh slots' chains: a tier holds its slots at once, is written a part at a time, and lands whole or not at all.

use pxl_render::gpu::arena::{arena_epoch, arena_write};
use pxl_render::mesh::chunk_mesher::box_bounds;

use pxl_spec::space::coord::v3::Aabb;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_runtime::mem::base::ptr_at;

use crate::world::chunk::build::{BuildEvent, Builder};
use crate::world::chunk::frontier::pool_of;
use crate::world::chunk::mesh_slot::{capacity, group_box_words, slot_at, Pool, FINE_BOXES};
use pxl_spec::space::coord::zoom::units;

use super::*;

pub struct Meshes {
    pub(super) holder: &'static mut [Nth],
    /// Next slot in a tier's chain; a tier past one slot continues in any free slot of its pool.
    pub(super) next: &'static mut [Nth],
    /// The tier the open bake is landing; all its slots are held at once in `reserve`.
    pub(super) landing: Option<Landing>,
    pub(super) epoch: u32,
    pub chained: u32,
    pub(super) fine_peak: u32,
    pub(super) coarse_peak: u32,
    pub(super) waits: u32,
    pub(super) wait_zooms: u32,
}

pub(super) fn free_at(meshes: &Meshes, at: usize) -> bool {
    meshes.holder[at].is_none()
}

pub(super) fn claimable(meshes: &Meshes, pool: Pool) -> u32 {
    pool.range().filter(|&at| free_at(meshes, at)).count() as u32
}

/// `k` free slots of `pool`, all or none, chained in order. Taking some and
/// waiting for the rest would let two tiers deadlock on each other's slots.
pub(super) fn claim(meshes: &mut Meshes, pool: Pool, k: u32, holder: u32) -> Option<usize> {
    if claimable(meshes, pool) < k {
        return None;
    }
    let (mut first, mut last): (Option<usize>, Option<usize>) = (None, None);
    let mut left = k;
    for at in pool.range() {
        if left == 0 {
            break;
        }
        if !free_at(meshes, at) {
            continue;
        }
        meshes.holder[at] = Nth::of(holder as usize);
        meshes.next[at] = Nth::NONE;
        match last {
            Some(p) => meshes.next[p] = Nth::of(at),
            None => first = Some(at),
        }
        last = Some(at);
        left -= 1;
    }
    first
}

pub(super) fn let_go(meshes: &mut Meshes, at: usize, id: u32) {
    let mut slot = Some(at);
    while let Some(at) = slot {
        slot = meshes.next[at].take();
        if meshes.holder[at] == Nth::of(id as usize) {
            meshes.holder[at] = Nth::NONE;
        }
    }
}

/// Drops the tiers `keep` rejects, and their slots unless a kept or landing tier shares them.
pub(super) fn prune(s: &mut Slot, meshes: &mut Meshes, keep: impl Fn(&Tier) -> bool, landing: Option<usize>) {
    for t in s.tiers.iter().filter(|t| !keep(t)) {
        if landing != Some(t.slot.index) && !s.tiers.iter().any(|k| keep(k) && k.slot.index == t.slot.index) {
            let_go(meshes, t.slot.index, s.id);
        }
    }
    s.tiers.retain(keep);
}

/// A slot holds at most the drawn tier and the one just built.
pub(super) const HELD: usize = 2;

#[derive(Clone, Copy)]
pub(super) struct Held<T: Copy> {
    pub(super) items: [Option<T>; HELD],
    pub(super) len: usize,
}

impl<T: Copy> Held<T> {
    pub(super) const EMPTY: Held<T> = Held { items: [None; HELD], len: 0 };

    pub(super) fn iter(&self) -> impl Iterator<Item = &T> {
        self.items[..self.len].iter().flatten()
    }

    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.items[..self.len].iter_mut().flatten()
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }

    pub(super) fn push(&mut self, t: T) {
        xpute_core::ensure!(self.len < HELD, ENOTRECOVERABLE, self.len);
        self.items[self.len] = Some(t);
        self.len += 1;
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let mut n = 0;
        for k in 0..self.len {
            if let Some(t) = self.items[k].take().filter(|t| keep(t)) {
                self.items[n] = Some(t);
                n += 1;
            }
        }
        self.len = n;
    }

    pub(super) fn clear(&mut self) {
        self.retain(|_| false);
    }
}

pub(super) fn chained(meshes: &Meshes, at: usize, n: u32) -> Option<usize> {
    (0..n).try_fold(at, |at, _| meshes.next[at].get())
}

pub(super) struct Landing {
    pub(super) id: u32,
    pub(super) generation: u32,
    pub(super) first: usize,
    pub(super) slots: u32,
    /// Slots of the tier it is rebuilt over, which it writes over.
    pub(super) over: u32,
    pub(super) bounds: Option<Aabb>,
}

/// Holds every slot a tier of `boxes` fills at once, or records the need and
/// refuses; a rebuild reuses the old tier's slots and claims only the extra.
pub(super) fn reserve(slots: &mut [Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) -> bool {
    let BuildEvent::Reserve { id, generation, boxes } = e else { return false };
    // One bake is open at a time: a leftover landing is from one that closed
    // without landing.
    if let Some(l) = meshes.landing.take() {
        release(meshes, l);
    }
    let Some(i) = place[id as usize].get() else { return false };
    let s = &slots[i];
    let pool = pool_of(s.chunk);
    let k = boxes.div_ceil(capacity(s.chunk.cell_zoom())).max(1);
    let over = s.tiers.iter().next().map(|t| t.slot.index);
    let taken = match over {
        Some(first) => Some((first, (0..).take_while(|&n| chained(meshes, first, n).is_some()).count() as u32)),
        None => claim(meshes, pool, k, id).map(|first| (first, 0)),
    };
    let Some((first, held)) = taken else {
        return refuse(slots, i, meshes, generation, k);
    };
    if held > 0 && k > held {
        let last = chained(meshes, first, held - 1).unwrap_or(first);
        let more = claim(meshes, pool, k - held, id);
        match more {
            Some(more) => meshes.next[last] = Nth::of(more),
            None => return refuse(slots, i, meshes, generation, k),
        }
    }
    meshes.landing = Some(Landing {
        id,
        generation,
        first,
        slots: k,
        over: held,
        bounds: None,
    });
    true
}

/// Releases the landing once its bake is no longer open. Called after every
/// call that can close a bake.
pub(super) fn settle(meshes: &mut Meshes, builder: &Builder) {
    let open = builder.baking();
    if let Some(l) = meshes.landing.take_if(|l| open != Some((l.id, l.generation))) {
        release(meshes, l);
    }
}

pub(super) fn release(meshes: &mut Meshes, l: Landing) {
    if l.over == 0 {
        let_go(meshes, l.first, l.id);
    } else if let Some(last) = chained(meshes, l.first, l.over - 1) {
        if let Some(more) = meshes.next[last].take() {
            let_go(meshes, more, l.id);
        }
    }
}

pub(super) fn refuse(slots: &mut [Slot], i: usize, meshes: &mut Meshes, generation: u32, k: u32) -> bool {
    slots[i].need = Some((generation, k));
    slots[i].fresh = true;
    meshes.waits += 1;
    meshes.wait_zooms |= 1 << GROUND.tile_zoom(slots[i].chunk.cell_zoom());
    false
}

pub(super) fn part(slots: &[Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) {
    let BuildEvent::Part {
        id,
        generation,
        at,
        first,
        box_cnt,
        heights,
    } = e
    else {
        return;
    };
    let Some(l) = meshes.landing.as_ref().filter(|l| (l.id, l.generation) == (id, generation)) else {
        return;
    };
    let Some(i) = place[id as usize].get() else { return };
    let head = slot_at(l.first);
    let n = first / head.capacity;
    let Some(slot) = chained(meshes, l.first, n).map(slot_at) else { return };
    xpute_core::ensure!(first % head.capacity == 0 && box_cnt <= slot.capacity, ENOTRECOVERABLE, first, box_cnt);
    let box_words = group_box_words(slot.group);
    let asked = if slot.capacity == FINE_BOXES { &mut meshes.fine_peak } else { &mut meshes.coarse_peak };
    *asked = (*asked).max(first + box_cnt);
    // SAFETY: the block's boxes in `.fixed.bake`, just written by the bake, in
    // the slot group's format.
    let words = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(at as usize), (box_cnt * box_words) as usize) };
    let unit = units(slots[i].chunk.cell_zoom());
    let mut bounds = meshes.landing.as_ref().and_then(|l| l.bounds);
    for r in words.chunks_exact(box_words as usize) {
        let (min, size) = box_bounds(r, unit, heights);
        let b = Aabb {
            min,
            max: std::array::from_fn(|a| min[a] + size[a]),
        };
        bounds = Some(bounds.map_or(b, |a| a.join(b)));
    }
    if box_cnt > 0 {
        let before = arena_epoch();
        // SAFETY: the same words, as bytes.
        let bytes = unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), words.len() * 4) };
        // No device: the tier is asked for again once there is one.
        if !arena_write(slot.group as usize, slot.first_box * box_words * 4, bytes) {
            meshes.landing = None;
            return;
        }
        // The write that recreated the arena lost nothing written before it.
        if arena_epoch() != before && before == meshes.epoch {
            meshes.epoch = arena_epoch();
        }
    }
    if let Some(l) = meshes.landing.as_mut() {
        l.bounds = bounds;
    }
}

pub(super) fn land(slots: &mut [Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) {
    let BuildEvent::Tier { id, generation, box_cnt, heights } = e else {
        return;
    };
    let Some(i) = place[id as usize].get() else { return };
    let s = &slots[i];
    if s.tier(generation).is_some() {
        return;
    }
    let landing = match meshes.landing.take() {
        Some(l) if (l.id, l.generation) == (id, generation) => l,
        other => {
            meshes.landing = other;
            // An empty mesh reserved nothing; it takes one slot here.
            if box_cnt > 0 {
                return;
            }
            let over = s.tiers.iter().next().map(|t| t.slot.index);
            let taken = match over {
                Some(first) => Some(first),
                None => claim(meshes, pool_of(s.chunk), 1, id),
            };
            let Some(first) = taken else {
                refuse(slots, i, meshes, generation, 1);
                return;
            };
            Landing {
                id,
                generation,
                first,
                slots: 1,
                over: 0,
                bounds: None,
            }
        }
    };
    if landing.over > landing.slots {
        if let Some(last) = chained(meshes, landing.first, landing.slots - 1) {
            if let Some(tail) = meshes.next[last].take() {
                let_go(meshes, tail, id);
            }
        }
    }
    if landing.slots > 1 {
        meshes.chained += 1;
    }
    let slot = slot_at(landing.first);
    let s = &mut slots[i];
    let layout = s.layout;
    prune(s, meshes, |t| layout == Some(t.generation), Some(slot.index));
    // An older-generation tier on the same slots now shows these boxes.
    for t in s.tiers.iter_mut().filter(|t| t.slot.index == slot.index) {
        t.box_cnt = box_cnt;
        t.bounds = landing.bounds;
        t.heights = heights;
    }
    s.tiers.push(Tier {
        generation,
        bounds: landing.bounds,
        slot,
        box_cnt,
        heights,
    });
}
