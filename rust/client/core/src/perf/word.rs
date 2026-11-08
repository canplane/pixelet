// pxl-core/perf/word.rs

//! Records laid out word for word as perf.json lays them, with every offset
//! checked at compile time.

use core::mem::size_of;

/// A record laid out as a perf.json section. Implemented only through
/// `words!`, which checks every field's offset at compile time.
///
/// # Safety
/// The implementor must be `#[repr(C)]` with no padding, and every field an
/// f64, such a record, or bytes a whole number of words long, so every word
/// reads as an f64.
pub unsafe trait Words: Sized {}

/// `val`'s words, laid end to end as perf.json lays them.
pub fn as_words<T: Words>(val: &T) -> &[f64] {
    // SAFETY: `Words` guarantees plain f64s laid end to end.
    unsafe {
        core::slice::from_raw_parts(
            std::ptr::from_ref::<T>(val).cast::<f64>(),
            size_of::<T>() / 8,
        )
    }
}

pub fn as_words_mut<T: Words>(val: &mut T) -> &mut [f64] {
    // SAFETY: `Words` guarantees plain f64s laid end to end, and `val` is
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
pub fn zeroed<T: Words>() -> T {
    // SAFETY: `Words` guarantees plain f64s laid end to end, and all bits 0 is 0.0.
    unsafe { core::mem::zeroed() }
}

pub fn put<T: Words>(out: &mut [f64], at: u32, val: &T) {
    let words = as_words(val);
    let at = at as usize;
    out[at..at + words.len()].copy_from_slice(words);
}

macro_rules! words {
    ($t:ty, $words:expr, $($($field:ident).+ => $at:expr),+ $(,)?) => {
        const _: () = assert!(size_of::<$t>() == $words as usize * 8, concat!(stringify!($t), " is not the size perf.json lays out"));
        $(const _: () = assert!(offset_of!($t, $($field).+) == $at as usize * 8, concat!(stringify!($t), " puts ", stringify!($($field).+), " off the word perf.json gives it"));)+
        // SAFETY: the two assertions above are the claim.
        unsafe impl Words for $t {}
    };
}
pub(crate) use words;
