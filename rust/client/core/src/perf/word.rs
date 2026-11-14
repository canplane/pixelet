// pxl-core/perf/word.rs

//! Records laid out word for word as perf.json5 lays them, with every offset
//! checked at compile time.

use core::mem::size_of;

/// A record laid out as a perf.json5 section. Implemented only through
/// `words!`, which checks every field's offset at compile time.
///
/// # Safety
/// The implementor must be `#[repr(C)]` with no padding, and every field an
/// f64, such a record, or bytes a whole number of words long, so every word
/// reads as an f64.
pub unsafe trait AsWords: Sized {}

/// `val`'s words, laid end to end as perf.json5 lays them.
pub fn as_words<T: AsWords>(val: &T) -> &[f64] {
    // SAFETY: `AsWords` guarantees plain f64s laid end to end.
    unsafe {
        core::slice::from_raw_parts(
            std::ptr::from_ref::<T>(val).cast::<f64>(),
            size_of::<T>() / 8,
        )
    }
}

pub fn as_words_mut<T: AsWords>(val: &mut T) -> &mut [f64] {
    // SAFETY: `AsWords` guarantees plain f64s laid end to end, and `val` is
    // borrowed whole.
    unsafe {
        core::slice::from_raw_parts_mut(
            std::ptr::from_mut::<T>(val).cast::<f64>(),
            size_of::<T>() / 8,
        )
    }
}

/// A record with every word 0.0. `Default` cannot be derived for an array
/// longer than 32, and this serves such records instead.
pub fn zeroed<T: AsWords>() -> T {
    // SAFETY: `AsWords` guarantees plain f64s laid end to end, and all bits 0 is 0.0.
    unsafe { core::mem::zeroed() }
}

pub fn put<T: AsWords>(out: &mut [f64], at: usize, val: &T) {
    let words = as_words(val);
    out[at..at + words.len()].copy_from_slice(words);
}

macro_rules! words {
    ($t:ty, $words:expr, $($($field:ident).+ => $at:expr),+ $(,)?) => {
        const _: () = assert!(size_of::<$t>() == $words * 8, concat!(stringify!($t), " is not the size perf.json5 lays out"));
        $(const _: () = assert!(offset_of!($t, $($field).+) == $at * 8, concat!(stringify!($t), " puts ", stringify!($($field).+), " off the word perf.json5 gives it"));)+
        // SAFETY: the two assertions above are the claim.
        unsafe impl AsWords for $t {}
    };
}
pub(crate) use words;
