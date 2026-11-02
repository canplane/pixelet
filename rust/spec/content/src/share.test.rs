use super::halve;

/// The halves sum to the count, each within its room.
#[test]
fn halves_sum_within_rooms() {
    let mut h = 0x1234_5678u32;
    let mut next = || {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        h
    };
    for _ in 0..100_000 {
        let rooms = [next() % 3000, next() % 3000];
        let n = next() % (rooms[0] + rooms[1] + 1);
        let a = halve(n, rooms, next());
        assert!(a <= rooms[0] && n - a <= rooms[1], "{rooms:?} {n} {a}");
    }
}
