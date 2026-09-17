// xpute_core/sys/vector.rs
// (no pair: the writer is script/pair.ts, which flattens what a generator
// under script/vector/ returns)

#![cfg(any(test, feature = "vector"))]

//! The reader of the vectors: lines of `path<TAB>value`, the path the keys
//! and indices down to the value joined by dots. A split per line and no
//! parser, so reading is not itself a thing to verify. Under `cargo test`
//! only — the crate's own tests, and another crate's through the `vector`
//! feature on its dev-dependency.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub struct Vector {
    map: HashMap<&'static str, &'static str>,
}

impl Vector {
    /// The vector an `include_str!` read.
    pub fn load(src: &'static str) -> Vector {
        Vector {
            map: src.lines().filter_map(|l| l.split_once('\t')).collect(),
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    pub fn s(&self, key: &str) -> &'static str {
        self.map.get(key).unwrap_or_else(|| panic!("vector: no {key}"))
    }

    pub fn u64(&self, key: &str) -> u64 {
        self.s(key).parse().unwrap_or_else(|_| panic!("vector: {key} is not a u64"))
    }

    pub fn i64(&self, key: &str) -> i64 {
        self.s(key).parse().unwrap_or_else(|_| panic!("vector: {key} is not an i64"))
    }

    pub fn u32(&self, key: &str) -> u32 {
        self.s(key).parse().unwrap_or_else(|_| panic!("vector: {key} is not a u32"))
    }

    pub fn i32(&self, key: &str) -> i32 {
        self.s(key).parse().unwrap_or_else(|_| panic!("vector: {key} is not an i32"))
    }

    /// A double written through f64_bits.
    pub fn f64(&self, key: &str) -> f64 {
        let text = self.s(key);
        f64::from_bits(u64::from_str_radix(text.trim_start_matches("0x"), 16).unwrap_or_else(|_| panic!("vector: {key} is not a double's bits")))
    }

    pub fn bool(&self, key: &str) -> bool {
        match self.s(key) {
            "true" => true,
            "false" => false,
            other => panic!("vector: {key} is {other}, not a boolean"),
        }
    }

    /// Bytes written as hex.
    pub fn bytes(&self, key: &str) -> Vec<u8> {
        let text = self.s(key);
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap_or_else(|_| panic!("vector: {key} is not hex")))
            .collect()
    }

    /// Calls `f` with `base.0`, `base.1`, … while an entry has that prefix.
    pub fn each(&self, base: &str, mut f: impl FnMut(&str)) {
        let mut k = 0;
        loop {
            let prefix = if base.is_empty() { format!("{k}") } else { format!("{base}.{k}") };
            let dotted = format!("{prefix}.");
            if !self.map.keys().any(|key| *key == prefix || key.starts_with(&dotted)) {
                break;
            }
            f(&prefix);
            k += 1;
        }
        assert!(k > 0, "vector: {base} is empty");
    }

    /// How many entries `base` has.
    pub fn len(&self, base: &str) -> usize {
        let mut n = 0;
        self.each(base, |_| n += 1);
        n
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// A result, or none where the TypeScript threw — "throw" in a vector.
pub fn caught<T>(f: impl FnOnce() -> T) -> Option<T> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let out = catch_unwind(AssertUnwindSafe(f)).ok();
    std::panic::set_hook(hook);
    out
}
