// pxl-spec/space/hierarchy/morton.rs

//! A node's one key at any zoom: its index's digits in two's complement, as the signed
//! indices are, interleaved a zoom at a time from the coarsest in `child_idx`'s order,
//! left-aligned, then a 1 that marks where they end. An axis has digits from the zoom its
//! extent reaches (`zoom.rs` `EXTENT`): X and Y from z1, and Z, with 16 bits to X's 24, from
//! z9. So a 3D key equals the 2D one down to z8, and an axis's first digit is its sign. A
//! coarser key's digits prefix a finer one's over the same ground, in any number of axes.

use super::tree::Node;
use crate::space::coord::zoom::{Zoom, EXTENT, UNIT_ZOOM};
use core::fmt;

const LANES: [u32; 3] = EXTENT;

/// The digits axis `axis` has at `zoom`: one a zoom from where its extent reaches.
#[inline]
const fn digits(axis: usize, zoom: Zoom) -> u32 {
    zoom.saturating_sub(UNIT_ZOOM - LANES[axis])
}

/// A key's digits at `zoom`.
pub const fn bits<const D: usize>(zoom: Zoom) -> u32 {
    let (mut n, mut axis) = (0, 0);
    while axis < D {
        n += digits(axis, zoom);
        axis += 1;
    }
    n
}

/// The axes with a digit at `zoom`, the order they are read in.
fn axes<const D: usize>(zoom: Zoom) -> impl Iterator<Item = usize> {
    (0..D)
        .rev()
        .filter(move |&i| digits(i, zoom) > digits(i, zoom - 1))
}

/// The key of the node at `zoom` whose index is `at`.
pub fn key_of<const D: usize>(zoom: Zoom, at: [i32; D]) -> u128 {
    let n = bits::<D>(zoom);
    ended(digits_of(zoom, &at), n)
}

/// `n` digits from the top, then the 1 that ends them.
const fn ended(digits: u128, n: u32) -> u128 {
    // Two shifts: no digits at all would shift by the word's width.
    ((digits << (127 - n)) << 1) | (1 << (127 - n))
}

/// The zoom axis `axis`'s first digit is at.
const fn first_zoom(axis: usize) -> Zoom {
    (UNIT_ZOOM - LANES[axis] + 1) as Zoom
}

/// `at`'s digits at `zoom`, coarsest first: zooms with the same axes are a
/// run of each axis's next low bits, interleaved in `axes`' order.
fn digits_of<const D: usize>(zoom: Zoom, at: &[i32; D]) -> u128 {
    let mut key = 0u128;
    let mut z = 1;
    while z <= zoom {
        let next = (0..D)
            .map(first_zoom)
            .filter(|&s| s > z)
            .fold(zoom + 1, Zoom::min);
        let (len, k) = ((next - z), axes::<D>(z).count() as u32);
        let mut run = 0u128;
        for (p, i) in axes::<D>(z).enumerate() {
            // The axis's digits at zooms `z..next`, finest lowest.
            let lane = ((at[i] >> (zoom + 1 - next)) as u32 as u64) & ((1u64 << len) - 1);
            run |= u128::from(spread(lane, k)) << (k - 1 - p as u32);
        }
        key = (key << (len * k)) | run;
        z = next;
    }
    key
}

/// `v`'s bits `n` apart, lowest first: bit `j` to bit `n · j`.
#[inline]
fn spread(v: u64, n: u32) -> u64 {
    match n {
        1 => v,
        2 => {
            let mut x = v & 0xffff_ffff;
            x = (x | (x << 16)) & 0x0000_ffff_0000_ffff;
            x = (x | (x << 8)) & 0x00ff_00ff_00ff_00ff;
            x = (x | (x << 4)) & 0x0f0f_0f0f_0f0f_0f0f;
            x = (x | (x << 2)) & 0x3333_3333_3333_3333;
            (x | (x << 1)) & 0x5555_5555_5555_5555
        }
        _ => {
            let mut x = v & 0x1f_ffff;
            x = (x | (x << 32)) & 0x001f_0000_0000_ffff;
            x = (x | (x << 16)) & 0x001f_0000_ff00_00ff;
            x = (x | (x << 8)) & 0x100f_00f0_0f00_f00f;
            x = (x | (x << 4)) & 0x10c3_0c30_c30c_30c3;
            (x | (x << 2)) & 0x1249_2492_4924_9249
        }
    }
}

/// Whether `key` is a key, which `node_of` reads back: a 1 ends its digits.
pub const fn is_key(key: u128) -> bool {
    key != 0
}

/// The digits `key` holds, above its end bit; a non-key refused.
fn len(key: u128) -> u32 {
    xpute_kit::ensure!(is_key(key), EINVAL);
    127 - key.trailing_zeros()
}

/// `key_of`'s inverse: the zoom, and each axis's index at it, signed.
pub fn node_of<const D: usize>(key: u128) -> Node<D> {
    let n = len(key);
    let (mut zoom, mut read, mut at) = (0, 0, [0i32; D]);
    while read < n {
        zoom += 1;
        for i in axes::<D>(zoom) {
            read += 1;
            at[i] = (at[i] << 1) | ((key >> (128 - read)) & 1) as i32;
        }
    }
    (
        zoom,
        std::array::from_fn(|i| signed(at[i], digits(i, zoom))),
    )
}

/// A key written one character a zoom, coarsest first, each character being the digits that
/// zoom adds, so a coarser node's name prefixes a finer one's and the length is the zoom. In
/// 2D it is a quadkey; in 3D an octkey, the same as the quadkey down to z8. It is written into
/// `out` rather than built, so reading a key allocates nothing.
pub(crate) fn write_name<const D: usize>(out: &mut impl fmt::Write, key: u128) -> fmt::Result {
    let n = len(key);
    let (mut zoom, mut read) = (0, 0);
    while read < n {
        zoom += 1;
        let w = bits::<D>(zoom) - bits::<D>(zoom - 1);
        let d = (key >> (128 - read - w)) & ((1 << w) - 1);
        out.write_char(char::from(b'0' + d as u8))?;
        read += w;
    }
    Ok(())
}

pub fn name<const D: usize>(key: u128) -> String {
    let mut out = String::new();
    write_name::<D>(&mut out, key).expect("a String takes any write");
    out
}

/// `write_name`'s inverse; `None` for what no key writes.
pub fn key_of_name<const D: usize>(name: &str) -> Option<u128> {
    let (mut digits, mut n) = (0u128, 0u32);
    for (z, c) in name.bytes().enumerate() {
        let zoom = Zoom::try_from(z + 1).ok()?;
        let w = bits::<D>(zoom) - bits::<D>(zoom - 1);
        let d = c.checked_sub(b'0').filter(|&d| u32::from(d) < 1 << w)?;
        if n + w > 127 {
            return None;
        }
        (digits, n) = ((digits << w) | u128::from(d), n + w);
    }
    Some(ended(digits, n))
}

/// `v`'s low `width` bits as a two's complement number.
fn signed(v: i32, width: u32) -> i32 {
    match width {
        0 => 0,
        w => v.wrapping_shl(32 - w) >> (32 - w),
    }
}

const _: () = assert!(
    EXTENT[0] == UNIT_ZOOM && EXTENT[1] == EXTENT[0],
    "X and Y digit from z1"
);

#[cfg(test)]
#[path = "morton.test.rs"]
mod test;
