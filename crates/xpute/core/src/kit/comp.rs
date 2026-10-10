// xpute_core/kit/comp.rs

/// A comparator: negative, zero or positive, as `comp(a, b)` reads there.
pub type Comp<T> = Box<dyn Fn(&T, &T) -> i32>;

pub fn comp_numeric<N: PartialOrd>(a: N, b: N) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
}

/// JavaScript orders strings by UTF-16 code unit, not by code point as
/// `str`'s `<` does: past U+FFFF the two disagree ("\u{FF61}" is after
/// "\u{1F600}" in UTF-16, before it by code point).
pub fn comp_str(a: &str, b: &str) -> i32 {
    match a.encode_utf16().cmp(b.encode_utf16()) {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Greater => 1,
        core::cmp::Ordering::Equal => 0,
    }
}

pub fn make_comp<T: 'static, K: PartialOrd + 'static>(
    key_fn: impl Fn(&T) -> K + 'static,
    tie_fn: Option<Comp<T>>, // defaults to 0 if omitted (ties: implementation-defined ordering)
) -> Comp<T> {
    let tie = tie_fn.unwrap_or_else(|| Box::new(|_, _| 0));

    Box::new(move |a, b| {
        let ka = key_fn(a);
        let kb = key_fn(b);
        let c = comp_numeric(ka, kb);
        if c != 0 {
            c
        } else {
            tie(a, b)
        }
    })
}

#[cfg(test)]
#[path = "comp.test.rs"]
mod test;
