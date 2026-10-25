// pxl-kernel/domain/scene/decor/instance.rs

//! Instances as lanes, and the block they are laid into: clouds', drawn
//! by size (domain/scene/decor/cell.rs `present_block`).

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
