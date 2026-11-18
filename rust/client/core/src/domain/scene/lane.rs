// pxl-core/domain/scene/lane.rs

//! Instances laid out as lanes, in the layout the decor and impostor
//! shaders read (pxl-content `generation::drawing`), for the instances the CPU
//! writes: the clouds' patches (domain/scene/environment/cloud.rs) and the
//! ghost's one decor (domain/scene/indicator/ghost.rs). A buffer holds the
//! group count, then each group's instance count, then each group's lanes,
//! then the index list.

use pxl_content::generation::drawing::LANE_CNT;

/// Where one group's lanes lie in a buffer: `drawing::LANE_CNT` f32 lanes
/// (x, y, z, spin, scale), then one flag byte an instance, padded to a whole
/// word.
#[derive(Clone, Copy)]
pub(crate) struct GroupRange {
    pub first: usize,
    pub cnt: usize,
}

impl GroupRange {
    pub(crate) const fn word_cnt(cnt: usize) -> usize {
        LANE_CNT as usize * cnt + cnt.div_ceil(4)
    }

    /// The word of instance `i` in `lane`, where lane 0 is x, 1 y, 2 z,
    /// 3 spin and 4 scale.
    pub(crate) const fn f32_at(self, lane: usize, i: usize) -> usize {
        self.first + lane * self.cnt + i
    }

    pub(crate) const fn flags(self) -> usize {
        self.first + LANE_CNT as usize * self.cnt
    }
}

/// Each group's first lane word and instance count, read from the
/// buffer's header.
pub(crate) fn groups(words: &[u32]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let groups = words.first().map_or(0, |&g| g as usize);
    let mut at = 1 + groups;
    words
        .get(1..1 + groups)
        .unwrap_or(&[])
        .iter()
        .map(move |&n| {
            let (first, n) = (at, n as usize);
            at += GroupRange::word_cnt(n);
            (first as u32, n as u32)
        })
}
