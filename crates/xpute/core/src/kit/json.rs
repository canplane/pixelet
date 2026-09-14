// xpute_core/kit/json.rs

//! `JSON.stringify` and `JSON.parse` of a flat string map — the one JSON
//! the records carry (a POI's tags, a placement's), written by either side
//! and read by the other, so the text is the host's to the byte.
//!
//! What the host does that a naive port would not:
//! - an object's own key order puts array-index keys ("0", "12") first in
//!   ascending number, then the rest in insertion order, and both
//!   stringify and parse follow it;
//! - a key given twice keeps its first position and its last value;
//! - an `undefined` value is left out of the text;
//! - a string escapes `"`, `\`, the five short controls, and every other
//!   code unit below U+0020 as `\u00xx` in lowercase hex, and nothing else.
//!
//! The one thing it cannot be: a lone surrogate, which a JavaScript string
//! holds and a Rust one cannot; `\ud800` alone parses to U+FFFD.

use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

/// `Record<string, string | undefined>`, in the order it was built.
pub type StrMap = Vec<(String, Option<String>)>;

/// A canonical array index: "0", or digits with no leading zero, below
/// 2^32 − 1.
fn is_array_index(k: &str) -> bool {
    if k.is_empty() || k.len() > 10 || !k.bytes().all(|b| b.is_ascii_digit()) || (k.len() > 1 && k.starts_with('0')) {
        return false;
    }
    k.parse::<u64>().is_ok_and(|n| n < 0xffff_ffff)
}

/// The entries as the object holds them: a key once, at its first
/// position with its last value, index keys first.
fn ordered(map: &[(String, Option<String>)]) -> Vec<(&str, Option<&str>)> {
    let mut out: Vec<(&str, Option<&str>)> = Vec::with_capacity(map.len());
    for (k, v) in map {
        match out.iter_mut().find(|(have, _)| *have == k.as_str()) {
            Some(slot) => slot.1 = v.as_deref(),
            None => out.push((k.as_str(), v.as_deref())),
        }
    }
    let (mut index, rest): (Vec<_>, Vec<_>) = out.into_iter().partition(|(k, _)| is_array_index(k));
    index.sort_by_key(|(k, _)| k.parse::<u64>().unwrap());
    index.extend(rest);
    index
}

fn quote(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `JSON.stringify(map)`.
pub fn stringify_str_map(map: &[(String, Option<String>)]) -> String {
    let mut out = String::from("{");
    let mut first = true;
    for (k, v) in ordered(map) {
        let Some(v) = v else { continue };
        if !first {
            out.push(',');
        }
        first = false;
        quote(&mut out, k);
        out.push(':');
        quote(&mut out, v);
    }
    out.push('}');
    out
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn fail(&self, what: &str) -> MarshalError {
        MarshalError::new(Errno::EBADMSG, Some(&format!("JSON: {what} at {}", self.at)), None)
    }

    fn ws(&mut self) {
        while self.at < self.s.len() && matches!(self.s[self.at], b' ' | b'\t' | b'\n' | b'\r') {
            self.at += 1;
        }
    }

    fn eat(&mut self, b: u8) -> Result<(), MarshalError> {
        self.ws();
        if self.s.get(self.at) != Some(&b) {
            return Err(self.fail(&format!("expected '{}'", b as char)));
        }
        self.at += 1;
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, MarshalError> {
        let digits = self.s.get(self.at..self.at + 4).ok_or_else(|| self.fail("short \\u escape"))?;
        let text = core::str::from_utf8(digits).map_err(|_| self.fail("bad \\u escape"))?;
        if !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(self.fail("bad \\u escape"));
        }
        self.at += 4;
        Ok(u32::from_str_radix(text, 16).unwrap())
    }

    fn string(&mut self) -> Result<String, MarshalError> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let Some(&b) = self.s.get(self.at) else { return Err(self.fail("unterminated string")) };
            match b {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    let Some(&e) = self.s.get(self.at) else { return Err(self.fail("unterminated escape")) };
                    self.at += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let unit = if (0xd800..0xdc00).contains(&hi) && self.s.get(self.at..self.at + 2) == Some(b"\\u") {
                                let save = self.at;
                                self.at += 2;
                                let lo = self.hex4()?;
                                if (0xdc00..0xe000).contains(&lo) {
                                    0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
                                } else {
                                    self.at = save;
                                    hi
                                }
                            } else {
                                hi
                            };
                            out.push(char::from_u32(unit).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(self.fail("bad escape")),
                    }
                }
                b if b < 0x20 => return Err(self.fail("control character in string")),
                _ => {
                    // A whole UTF-8 sequence: the input is a &str, so it is valid.
                    let len = match b {
                        0x00..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    out.push_str(core::str::from_utf8(&self.s[self.at..self.at + len]).unwrap());
                    self.at += len;
                }
            }
        }
    }
}

/// `JSON.parse(text)` for an object whose values are strings. Anything
/// else — another value type, trailing text — is refused, where the host
/// would hand back whatever the text held.
pub fn parse_str_map(text: &str) -> Result<StrMap, MarshalError> {
    let mut p = Parser { s: text.as_bytes(), at: 0 };
    p.eat(b'{')?;
    let mut raw: StrMap = Vec::new();
    p.ws();
    if p.s.get(p.at) == Some(&b'}') {
        p.at += 1;
    } else {
        loop {
            let k = p.string()?;
            p.eat(b':')?;
            p.ws();
            let v = p.string()?;
            raw.push((k, Some(v)));
            p.ws();
            match p.s.get(p.at) {
                Some(b',') => p.at += 1,
                Some(b'}') => {
                    p.at += 1;
                    break;
                }
                _ => return Err(p.fail("expected ',' or '}'")),
            }
        }
    }
    p.ws();
    if p.at != p.s.len() {
        return Err(p.fail("trailing text"));
    }
    Ok(ordered(&raw).into_iter().map(|(k, v)| (k.to_string(), v.map(str::to_string))).collect())
}

#[cfg(test)]
#[path = "json.test.rs"]
mod test;
