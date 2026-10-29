// pxl-kernel/domain/scene/lane.rs

//! Instances as lanes, in the layout the decor and impostor shaders read
//! (pxl-spec-core scatter.rs `drawn`), as the CPU writes them: the clouds'
//! patches (domain/scene/environment/cloud.rs) and the ghost's one decor
//! (domain/scene/indicator/ghost.rs). Each is its group count, each
//! group's instance count, then each group's lanes, then the index list.

use pxl_spec_core::scatter::drawn::LANES;

/// A group's lanes in one: `drawn::LANES` f32 lanes (x, y, z, spin,
/// scale), then byte flags padded to a word.
#[derive(Clone, Copy)]
pub struct GroupLanes {
    pub first: usize,
    pub cnt: usize,
}

impl GroupLanes {
    pub const fn words(cnt: usize) -> usize {
        LANES as usize * cnt + cnt.div_ceil(4)
    }

    /// Lanes: 0 x, 1 y, 2 z, 3 spin, 4 scale.
    pub const fn f32_at(self, lane: usize, i: usize) -> usize {
        self.first + lane * self.cnt + i
    }

    pub const fn flags(self) -> usize {
        self.first + LANES as usize * self.cnt
    }
}

/// Each group's first lane word and count, from its header.
pub fn groups(words: &[u32]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let groups = words.first().map_or(0, |&g| g as usize);
    let mut at = 1 + groups;
    words
        .get(1..1 + groups)
        .unwrap_or(&[])
        .iter()
        .map(move |&n| {
            let (first, n) = (at, n as usize);
            at += GroupLanes::words(n);
            (first as u32, n as u32)
        })
}
