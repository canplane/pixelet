// pxl-content/halve.rs

//! wgsl-port: a count handed down a pyramid, the one rule by which the decor, the caves
//! and the cumulus hand theirs down. A node's count is halved along each side in turn, the
//! first half taking its share by a hash's weight and each half held within its room, so the
//! halves sum to the count exactly and a reader goes down only the half it asks, one step a
//! side. The seed is what makes the result irregular.

/// The first half's part of `n` (under 2^16); the rest is the second's. It is `n` times the
/// weight in `w`'s top 16 bits, out of 2^16, with its fraction rounded up where it passes the
/// draw in `w`'s low 16 bits, so a lone count goes either way as the weight says. Each half is
/// held within its cap (`caps`, which together hold at least `n`).
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

/// The word `halve` takes: the first half's share, out of 2^16, over the low 16 bits of the
/// `draw` that rounds it.
pub const fn weight(share: u32, draw: u32) -> u32 {
    share << 16 | draw & 0xffff
}

/// Quarter `q`'s part (`dx + 2dy`) of `n` spread over four quarters of `cap` each: halved
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
#[path = "halve.test.rs"]
mod test;
