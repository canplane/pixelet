// xpute-golden/lib.rs

//! Reads golden records, files of lines `dotted.path<TAB>value`.
//!
//! A record holds every language that reads it to the same answer. It is
//! never recorded again to make a test pass: a difference means that one
//! implementation has drifted. A record changed on purpose changes for
//! every language at once.

use std::collections::HashMap;

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
        self.map.get(key).unwrap_or_else(|| xpute_kit::bug!(ENOENT))
    }

    pub fn u64(&self, key: &str) -> u64 {
        self.s(key)
            .parse()
            .unwrap_or_else(|_| xpute_kit::bug!(EINVAL))
    }

    pub fn i64(&self, key: &str) -> i64 {
        self.s(key)
            .parse()
            .unwrap_or_else(|_| xpute_kit::bug!(EINVAL))
    }

    pub fn u32(&self, key: &str) -> u32 {
        self.s(key)
            .parse()
            .unwrap_or_else(|_| xpute_kit::bug!(EINVAL))
    }

    pub fn i32(&self, key: &str) -> i32 {
        self.s(key)
            .parse()
            .unwrap_or_else(|_| xpute_kit::bug!(EINVAL))
    }

    /// A double recorded as the hex of its bits, so it reads back exactly,
    /// -0 and a NaN's payload included.
    pub fn f64(&self, key: &str) -> f64 {
        let text = self.s(key);
        f64::from_bits(
            u64::from_str_radix(text.trim_start_matches("0x"), 16)
                .unwrap_or_else(|_| xpute_kit::bug!(EINVAL)),
        )
    }

    pub fn bool(&self, key: &str) -> bool {
        match self.s(key) {
            "true" => true,
            "false" => false,
            _ => xpute_kit::bug!(EINVAL),
        }
    }

    /// This reads bytes recorded as hex, two digits a byte.
    pub fn bytes(&self, key: &str) -> Vec<u8> {
        let text = self.s(key);
        (0..text.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&text[i..i + 2], 16).unwrap_or_else(|_| xpute_kit::bug!(EINVAL))
            })
            .collect()
    }

    /// Calls `f` with `base.0`, `base.1` and so on, for as long as some
    /// record's key starts with that prefix. A list with no item is a bug.
    pub fn each(&self, base: &str, mut f: impl FnMut(&str)) {
        let mut k = 0;
        loop {
            let prefix = if base.is_empty() {
                format!("{k}")
            } else {
                format!("{base}.{k}")
            };
            let dotted = format!("{prefix}.");
            if !self
                .map
                .keys()
                .any(|key| *key == prefix || key.starts_with(&dotted))
            {
                break;
            }
            f(&prefix);
            k += 1;
        }
        xpute_kit::ensure!(k > 0, ENOENT);
    }

    pub fn len(&self, base: &str) -> usize {
        let mut n = 0;
        self.each(base, |_| n += 1);
        n
    }
}
