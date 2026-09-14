// @xpute/core/kit/comp.ts

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

pub fn comp_str(a: &str, b: &str) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
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
