//! The lattice word: @pxl/shared/spec/space/pvf3/ivec.ts's layout, X in the
//! top 24 bits, Y in the next 24, Z in the low 16, each lane signed. What a
//! chunk, a page and a cell are keyed by; the fractional half of a PVF3
//! is not here yet (issue/20260914-what-stays-in-javascript.md's first
//! move brings it).

/// The three lanes of a word, signed.
pub fn lanes(v: u64) -> (i64, i64, i64) {
    ((v as i64) >> 40, ((v << 24) as i64) >> 40, ((v << 48) as i64) >> 48)
}

/// The X and Y lanes of a word, signed.
pub fn lanes_xy(v: u64) -> (i64, i64) {
    ((v as i64) >> 40, ((v << 24) as i64) >> 40)
}

/// The word of (x, y, z), each lane truncated to its width.
pub fn gather(x: i64, y: i64, z: i64) -> u64 {
    ((x as u64 & 0xff_ffff) << 40) | ((y as u64 & 0xff_ffff) << 16) | (z as u64 & 0xffff)
}

/// The word of (x, y, 0).
pub fn gather_xy(x: i64, y: i64) -> u64 {
    gather(x, y, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_round_trips_its_signed_lanes() {
        for (x, y, z) in [(0, 0, 0), (1, 2, 3), (-1, -2, -3), (8_388_607, -8_388_608, 32_767), (-8_388_608, 8_388_607, -32_768)] {
            assert_eq!(lanes(gather(x, y, z)), (x, y, z));
            assert_eq!(lanes_xy(gather_xy(x, y)), (x, y));
        }
        // ivec.ts's layout, pinned: X at the top.
        assert_eq!(gather(1, 0, 0), 1 << 40);
        assert_eq!(gather(0, 1, 0), 1 << 16);
        assert_eq!(gather(0, 0, 1), 1);
    }
}
