// pxl-content/share.rs

//! gpu-portable: a count handed down a pyramid, the one rule the decor, the
//! caves and the cumulus hand theirs by: a node's count halved along each
//! side in turn, the first half its share by a hash's weight, each half
//! within its room, so the halves sum to the count exactly and a reader goes
//! down the one half it asks, a step a side. What makes it irregular is the
//! seed.

/// The first half's part of `n` (under 2^16), the rest the second's: `n` by
/// the weight in `w`'s top 16 bits of 2^16, its fraction rounded up where it
/// passes the draw in `w`'s low 16, so a lone count goes either way as the
/// weight says; held within each half's cap (`caps`, together at least
/// `n`).
pub fn halve(n: u32, caps: [u32; 2], w: u32) -> u32 {
    let scaled = n * (w >> 16);
    let share = (scaled >> 16) + ((scaled & 0xffff) > (w & 0xffff)) as u32;
    let least = n.saturating_sub(caps[1]);
    if share < least {
        least
    } else if share > caps[0] {
        caps[0]
    } else {
        share
    }
}

/// Quarter `q` (`dx + 2dy`)'s part of `n` over four of `cap` each: halved
/// across by `w[0]`, then along by `w[1]` (`halve`).
pub fn quarter(n: u32, cap: u32, w: [u32; 2], q: u32) -> u32 {
    let a = halve(n, [2 * cap, 2 * cap], w[0]);
    let n = if q & 1 == 0 { a } else { n - a };
    let b = halve(n, [cap, cap], w[1]);
    if q & 2 == 0 {
        b
    } else {
        n - b
    }
}

#[cfg(test)]
#[path = "share.test.rs"]
mod test;
