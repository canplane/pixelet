// pxl-core/domain/scene/column.rs

//! Instances laid out as columns, in the layout the decor and impostor
//! shaders read (pxl-content generation.rs `drawn`), for the instances the CPU
//! writes: the clouds' patches (domain/scene/environment/cloud.rs) and the
//! ghost's one decor (domain/scene/indicator/ghost.rs). A buffer holds the
//! group count, then each group's instance count, then each group's columns,
//! then the index list.

use pxl_content::generation::drawn::COLUMNS;

/// Where one group's columns lie in a buffer: `drawn::COLUMNS` f32 columns
/// (x, y, z, spin, scale), then one flag byte an instance, padded to a whole
/// word.
#[derive(Clone, Copy)]
pub struct GroupColumns {
    pub first: usize,
    pub cnt: usize,
}

impl GroupColumns {
    pub const fn words(cnt: usize) -> usize {
        COLUMNS as usize * cnt + cnt.div_ceil(4)
    }

    /// The word of instance `i` in `column`, where column 0 is x, 1 y, 2 z,
    /// 3 spin and 4 scale.
    pub const fn f32_at(self, column: usize, i: usize) -> usize {
        self.first + column * self.cnt + i
    }

    pub const fn flags(self) -> usize {
        self.first + COLUMNS as usize * self.cnt
    }
}

/// Each group's first column word and instance count, read from the
/// buffer's header.
pub fn groups(words: &[u32]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let groups = words.first().map_or(0, |&g| g as usize);
    let mut at = 1 + groups;
    words
        .get(1..1 + groups)
        .unwrap_or(&[])
        .iter()
        .map(move |&n| {
            let (first, n) = (at, n as usize);
            at += GroupColumns::words(n);
            (first as u32, n as u32)
        })
}
