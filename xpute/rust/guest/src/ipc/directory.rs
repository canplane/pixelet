// xpute-guest/ipc/directory.rs

//! The directory a guest's first turn writes: where the host finds the rings
//! and streams, and the guest's quantum policy. Keys are looked up, never
//! counted to, so a host passes over an entry it does not know (as ELF's
//! auxiliary vector does).

#[path = "directory.spec.rs"]
pub mod spec;

use xpute_kit::math::scalar::round;

use crate::sched::quantum::QuantumPolicy;

/// A key (`spec::key`, or one the program numbers with `key::PROGRAM` set)
/// and its value.
pub type Entry = (u32, u32);

pub const fn directory_words(entries: usize) -> usize {
    spec::HEAD as usize + 2 * entries
}

/// A key may appear once, so no two readers can pick different values.
pub fn write_directory(words: &mut [u32], entries: &[Entry]) {
    xpute_kit::ensure!(directory_words(entries.len()) <= words.len(), ENOSPC, entries.len());
    for (i, (key, _)) in entries.iter().enumerate() {
        xpute_kit::ensure!(entries[..i].iter().all(|(k, _)| k != key), EINVAL, *key);
    }
    words[..spec::HEAD as usize].copy_from_slice(&[spec::MAGIC, spec::VERSION, entries.len() as u32, 0]);
    for (i, (key, value)) in entries.iter().enumerate() {
        let at = spec::HEAD as usize + 2 * i;
        words[at] = *key;
        words[at + 1] = *value;
    }
}

/// Each number in `spec::QUANTUM_SCALE`.
pub fn quantum_entries(q: &QuantumPolicy) -> [Entry; 3] {
    let scaled = |v: f64| round(v * f64::from(spec::QUANTUM_SCALE)) as u32;
    [
        (spec::key::MARGIN_SHARE, scaled(q.margin_share)),
        (spec::key::BATCH_FRAMES, scaled(q.batch_frames)),
        (spec::key::SETTLE_MS, scaled(q.settle_ms)),
    ]
}
