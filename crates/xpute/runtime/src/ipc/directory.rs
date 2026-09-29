// xpute-runtime/ipc/directory.rs

//! The directory a guest's first turn writes: where its host finds the rings,
//! the streams and whatever else it reads of the memory, and the policy the
//! guest declares for its turns. Where anything lies is the guest's choice,
//! so this is the one place the host learns it; the host knows only where
//! the directory itself is.
//!
//! The format is directory.spec.rs's, from spec/xpute/ipc/directory.json: a
//! head, then entries of a key and a value. A key is looked up, never
//! counted to, so an entry added later is passed over by a host that does not
//! know it — the ELF auxiliary vector's rule, for the same reason.

#[path = "directory.spec.rs"]
pub mod spec;

use xpute_core::math::scalar::round;

use crate::sched::quantum::QuantumPolicy;

/// An entry: a key (`spec::key`, or one with `key::PROGRAM` set that the
/// program numbers) and its value.
pub type Entry = (u32, u32);

/// Words a directory of `entries` entries takes.
pub const fn directory_words(entries: usize) -> usize {
    spec::HEAD as usize + 2 * entries
}

/// Writes the directory into `words`: the head, then every entry in order. A
/// key appears once, so no two readers can differ on which of two values it
/// names.
pub fn write_directory(words: &mut [u32], entries: &[Entry]) {
    xpute_core::ensure!(directory_words(entries.len()) <= words.len(), ENOSPC, entries.len());
    for (i, (key, _)) in entries.iter().enumerate() {
        xpute_core::ensure!(entries[..i].iter().all(|(k, _)| k != key), EINVAL, *key);
    }
    words[..spec::HEAD as usize].copy_from_slice(&[spec::MAGIC, spec::VERSION, entries.len() as u32, 0]);
    for (i, (key, value)) in entries.iter().enumerate() {
        let at = spec::HEAD as usize + 2 * i;
        words[at] = *key;
        words[at + 1] = *value;
    }
}

/// The quantum policy's entries, each number in `spec::QUANTUM_SCALE`.
pub fn quantum_entries(q: &QuantumPolicy) -> [Entry; 3] {
    let scaled = |v: f64| round(v * f64::from(spec::QUANTUM_SCALE)) as u32;
    [
        (spec::key::MARGIN_SHARE, scaled(q.margin_share)),
        (spec::key::BATCH_FRAMES, scaled(q.batch_frames)),
        (spec::key::SETTLE_MS, scaled(q.settle_ms)),
    ]
}
