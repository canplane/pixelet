// pxl-content/pyramid.rs

//! gpu-portable: a count handed down a pyramid, the one rule the decor and
//! the caves hand theirs by: a node's count to its children by weights, each
//! within its room, whole by the largest remainder, so the children's counts
//! sum to the node's exactly and a node is known before anything under it is
//! asked. The weights are a hash's: what makes it irregular is the seed.

/// The children a count goes to: a cube's eight; a square's four leave the
/// rest none.
const K: usize = 8;

/// `n` handed to `K` children by `weights` (each under 2^15, `n` under 2^16),
/// none past its `rooms`: each its floor of `n` by its weight, then one more
/// to those with the largest remainders, the lower child first; a child at
/// its room keeps it and the rest goes round again. `n` past every room
/// together is cut to them.
pub fn share_out(n: u32, rooms: [u32; K], weights: [u32; K]) -> [u32; K] {
    let mut out = [0u32; K];
    let mut left = n;
    // The children at their rooms, a bit each (K under 32).
    let mut full = 0u32;
    let mut round = 0;
    while left > 0 && round <= K {
        let mut sum = 0u32;
        let mut k = 0;
        while k < K {
            if full >> k & 1 == 0 {
                sum += weights[k] + 1;
            }
            k += 1;
        }
        if sum == 0 {
            break;
        }
        // Floors, and the remainders kept.
        let mut rem = [0u32; K];
        let mut given = 0u32;
        let mut k = 0;
        while k < K {
            if full >> k & 1 == 0 {
                let w = weights[k] + 1;
                let g = left * w / sum;
                let room = rooms[k] - out[k];
                let g = if g > room { room } else { g };
                out[k] += g;
                given += g;
                rem[k] = left * w % sum;
            }
            k += 1;
        }
        left -= given;
        // One more each to the largest remainders, in order.
        let mut more = left;
        while more > 0 {
            let mut best = K;
            let mut k = 0;
            while k < K {
                if full >> k & 1 == 0 && out[k] < rooms[k] && rem[k] != u32::MAX && (best == K || rem[k] > rem[best]) {
                    best = k;
                }
                k += 1;
            }
            if best == K {
                break;
            }
            out[best] += 1;
            rem[best] = u32::MAX;
            more -= 1;
            left -= 1;
        }
        let mut k = 0;
        while k < K {
            if out[k] >= rooms[k] {
                full |= 1 << k;
            }
            k += 1;
        }
        round += 1;
    }
    out
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
