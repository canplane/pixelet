use super::halve;

/// The halves sum to the count, each within its cap.
#[test]
fn halves_sum_within_caps() {
    let mut h = 0x1234_5678u32;
    let mut next = || {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        h
    };
    for _ in 0..100_000 {
        let caps = [next() % 3000, next() % 3000];
        let n = next() % (caps[0] + caps[1] + 1);
        let a = halve(n, caps, next());
        assert!(a <= caps[0] && n - a <= caps[1], "{caps:?} {n} {a}");
    }
}
