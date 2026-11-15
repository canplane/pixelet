// pxl-gpu-link/addr.rs

//! An address in the guest's memory as the stream carries it: a u64, the
//! link's address (link/wit/types.wit `lent`), in two words, its low word
//! first (xpute-kit `split64`), at the places the spec's `OPS` names.

use xpute_kit::abi::word::{join64, split64};

use crate::spec::OPS;

/// Moves every address the records `recs` hold from a block at `from` to the
/// same place in a block at `to`, as `OPS` finds them, the rest untouched.
pub fn rebase(recs: &mut [u32], from: u64, to: u64) {
    let mut k = 0;
    while k + 2 <= recs.len() {
        let (code, n) = (recs[k], recs[k + 1] as usize);
        let end = k + 2 + n;
        for a in OPS.iter().filter(|a| a.op == code) {
            let at = end - a.from_end as usize;
            let moved = join64(recs[at], recs[at + 1])
                .wrapping_sub(from)
                .wrapping_add(to);
            recs[at..at + 2].copy_from_slice(&split64(moved));
        }
        k = end;
    }
}
