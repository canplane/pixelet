// xpute-core/golden.rs

#![cfg(any(test, feature = "golden"))]

//! Reads golden records: lines of `dotted.path<TAB>value`.
//!
//! A record of a protocol (XTP packets, TLV bytes, which texts are JSON) binds
//! every language and is never re-recorded to make a test pass: a difference
//! means one side moved. A record of a value that stays inside one language
//! may be re-recorded on a deliberate change.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub struct Golden {
    map: HashMap<&'static str, &'static str>,
}

impl Golden {
    pub fn load(src: &'static str) -> Golden {
        Golden {
            map: src.lines().filter_map(|l| l.split_once('\t')).collect(),
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    pub fn s(&self, key: &str) -> &'static str {
        self.map.get(key).unwrap_or_else(|| crate::bug!(ENOENT))
    }

    pub fn u64(&self, key: &str) -> u64 {
        self.s(key).parse().unwrap_or_else(|_| crate::bug!(EINVAL))
    }

    pub fn i64(&self, key: &str) -> i64 {
        self.s(key).parse().unwrap_or_else(|_| crate::bug!(EINVAL))
    }

    pub fn u32(&self, key: &str) -> u32 {
        self.s(key).parse().unwrap_or_else(|_| crate::bug!(EINVAL))
    }

    pub fn i32(&self, key: &str) -> i32 {
        self.s(key).parse().unwrap_or_else(|_| crate::bug!(EINVAL))
    }

    /// A double written through f64_bits.
    pub fn f64(&self, key: &str) -> f64 {
        let text = self.s(key);
        f64::from_bits(u64::from_str_radix(text.trim_start_matches("0x"), 16).unwrap_or_else(|_| crate::bug!(EINVAL)))
    }

    pub fn bool(&self, key: &str) -> bool {
        match self.s(key) {
            "true" => true,
            "false" => false,
            _ => crate::bug!(EINVAL),
        }
    }

    /// Bytes written as hex.
    pub fn bytes(&self, key: &str) -> Vec<u8> {
        let text = self.s(key);
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap_or_else(|_| crate::bug!(EINVAL)))
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
        crate::ensure!(k > 0, ENOENT);
    }

    pub fn len(&self, base: &str) -> usize {
        let mut n = 0;
        self.each(base, |_| n += 1);
        n
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// None where the call panicked: "throw" in a record.
pub fn caught<T>(f: impl FnOnce() -> T) -> Option<T> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let out = catch_unwind(AssertUnwindSafe(f)).ok();
    std::panic::set_hook(hook);
    out
}
