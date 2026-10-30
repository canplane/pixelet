// xpute-guest/ipc/directory.rs

//! The directory a guest's first turn writes: where the host finds the rings
//! and streams. Keys are looked up, never
//! counted to, so a host passes over an entry it does not know (as ELF's
//! auxiliary vector does).

#[path = "directory.spec.rs"]
pub mod spec;

use crate::mem::Memory;

/// A key (`spec::key`, or one the program numbers with `key::PROGRAM` set)
/// and its value.
pub type Entry = (u32, u32);

pub const fn directory_words(entries: usize) -> usize {
    spec::HEAD as usize + 2 * entries
}

/// Into the `words` at `at` in `mem`. A key may appear once, so no two
/// readers can pick different values.
///
/// # Safety
/// The range is valid and nothing else holds it while the directory is written.
pub unsafe fn write_directory(mem: Memory, at: usize, words: usize, entries: &[Entry]) {
    xpute_kit::ensure!(
        directory_words(entries.len()) <= words,
        ENOSPC,
        entries.len()
    );
    for (i, (key, _)) in entries.iter().enumerate() {
        xpute_kit::ensure!(entries[..i].iter().all(|(k, _)| k != key), EINVAL, *key);
    }
    // SAFETY: the caller's range.
    let words = unsafe { core::slice::from_raw_parts_mut(mem.at::<u32>(at), words) };
    words[..spec::HEAD as usize].copy_from_slice(&[
        spec::MAGIC,
        spec::VERSION,
        entries.len() as u32,
        0,
    ]);
    for (i, (key, val)) in entries.iter().enumerate() {
        let at = spec::HEAD as usize + 2 * i;
        words[at] = *key;
        words[at + 1] = *val;
    }
}
