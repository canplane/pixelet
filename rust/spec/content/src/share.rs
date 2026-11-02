// pxl-content/share.rs

//! gpu-portable: a count handed down a pyramid, the one rule the decor, the
//! caves and the cumulus hand theirs by: a node's count halved along each
//! side in turn, the first half its share by a hash's weight, each half
//! within its room, so the halves sum to the count exactly and a reader goes
//! down the one half it asks, a step a side. What makes it irregular is the
//! seed.

/// The first half's part of `n` (under 2^16), the rest the second's: `n` by
/// `w` of 2^16, held within each half's room (`rooms`, together at least
/// `n`).
#[expect(
    clippy::implicit_saturating_sub,
    reason = "rust-gpu has no `saturating_sub`"
)]
pub fn halve(n: u32, rooms: [u32; 2], w: u32) -> u32 {
    let share = (n * (w & 0xffff)) >> 16;
    let least = if n > rooms[1] { n - rooms[1] } else { 0 };
    let most = if n < rooms[0] { n } else { rooms[0] };
    if share < least {
        least
    } else if share > most {
        most
    } else {
        share
    }
}

#[cfg(test)]
#[path = "share.test.rs"]
mod test;
