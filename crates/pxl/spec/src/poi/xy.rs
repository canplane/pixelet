// pxl-spec/poi/xy.rs

//! The current transport's XY quadtree. Branching belongs to this address
//! scheme, not to the meaning of a POI summary or entity.

pub const ROOT_DEPTH: u32 = 15;
pub const MAX_DEPTH: u32 = 23;
pub const CHILDREN: usize = 4;
const HALF_WORLD: f64 = 8_388_608.0;

pub fn depth(key: u64) -> Option<u32> {
    if key == 0 {
        return None;
    }
    let bits = 63 - key.leading_zeros();
    (bits.is_multiple_of(2) && bits / 2 <= MAX_DEPTH).then_some(bits / 2)
}

pub fn parse(name: &str) -> Option<u64> {
    if name.is_empty() || name.len() > MAX_DEPTH as usize {
        return None;
    }
    name.bytes().try_fold(1u64, |key, c| (b'0'..=b'3').contains(&c).then(|| key * 4 + (c - b'0') as u64))
}

pub fn name(key: u64) -> String {
    let Some(d) = depth(key) else { return String::new() };
    (0..d).rev().map(|s| (b'0' + ((key >> (s * 2)) & 3) as u8) as char).collect()
}

pub fn children(key: u64) -> Vec<u64> {
    if depth(key).is_none_or(|d| d >= MAX_DEPTH) {
        return Vec::new();
    }
    (0..CHILDREN as u64).map(|q| key * 4 + q).collect()
}

pub fn contains(parent: u64, child: u64) -> bool {
    match (depth(parent), depth(child)) {
        (Some(p), Some(c)) if p <= c => child >> (2 * (c - p)) == parent,
        _ => false,
    }
}

pub fn bounds(key: u64) -> [f64; 4] {
    let mut x = -HALF_WORLD;
    let mut y = -HALF_WORLD;
    let mut span = HALF_WORLD * 2.0;
    for s in (0..depth(key).expect("validated XY key")).rev() {
        span *= 0.5;
        let q = (key >> (s * 2)) & 3;
        if q & 1 != 0 {
            x += span;
        }
        if q & 2 == 0 {
            y += span;
        }
    }
    [x, y, x + span, y + span]
}

pub fn key_at(position: [f64; 3], depth: u32) -> Option<u64> {
    let [x, y, z] = position;
    if depth > MAX_DEPTH || !z.is_finite() || !(-HALF_WORLD..HALF_WORLD).contains(&x) || !(-HALF_WORLD..HALF_WORLD).contains(&y) {
        return None;
    }
    let mut key = 1;
    let (mut x0, mut y0, mut span) = (-HALF_WORLD, -HALF_WORLD, HALF_WORLD * 2.0);
    for _ in 0..depth {
        span *= 0.5;
        let east = x >= x0 + span;
        let north = y >= y0 + span;
        key = key * 4 + u64::from(east) + 2 * u64::from(!north);
        if east {
            x0 += span;
        }
        if north {
            y0 += span;
        }
    }
    Some(key)
}
