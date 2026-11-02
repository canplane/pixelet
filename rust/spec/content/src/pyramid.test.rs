use super::share_out;

#[test]
fn shares_sum_to_the_count_within_rooms() {
    let mut h = 0x1234_5678u32;
    let mut next = || {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        h
    };
    for _ in 0..20_000 {
        let rooms = [0u32; 8].map(|_| next() % 40);
        let total: u32 = rooms.iter().sum();
        let n = next() % (total + 1);
        let weights = [0u32; 8].map(|_| next() % (1 << 15));
        let out = share_out(n, rooms, weights);
        assert_eq!(out.iter().sum::<u32>(), n, "{rooms:?} {weights:?} {n}");
        for k in 0..8 {
            assert!(out[k] <= rooms[k]);
        }
    }
}
