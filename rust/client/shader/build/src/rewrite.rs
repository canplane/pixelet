// pxl-shader-build/rewrite.rs

//! naga's WGSL rewritten into shapes Adreno's driver makes pipelines of
//! (doc/experiment/20261103-shaders-on-two-phones): it refuses, with
//! `VK_ERROR_UNKNOWN`, modules where an array- or struct-typed local is set
//! whole in a branch and read about loops, and where a small array is carried
//! so. Each local naga makes of a SPIR-V phi of such a type (`phi_*`, set and
//! read whole alone) is split into a local a scalar or vector leaf
//! (`split_phis`); then every small array of a 32-bit scalar the bindings do
//! not lay out is a vector (`small_arrays_as_vectors`), which WGSL indexes
//! alike. Both read naga's own layout of its output: a struct's members a
//! line each, a local's declaration, store and load a line each.

use std::collections::{HashMap, HashSet};

/// Each struct's members, by name, as naga writes them: `name: type,` a line.
fn structs(text: &str) -> HashMap<String, Vec<(String, String)>> {
    let mut out = HashMap::new();
    let mut lines = text.lines();
    while let Some(l) = lines.next() {
        let Some(name) = l.strip_prefix("struct ").and_then(|r| r.strip_suffix(" {")) else {
            continue;
        };
        let mut members = Vec::new();
        for m in lines.by_ref() {
            if m == "}" {
                break;
            }
            if let Some((n, t)) = m.trim().strip_suffix(',').and_then(|m| m.split_once(": ")) {
                members.push((n.to_string(), t.to_string()));
            }
        }
        out.insert(name.to_string(), members);
    }
    out
}

/// `array<T, N>`'s `T` and `N`, a fixed size's alone.
fn array_of(t: &str) -> Option<(&str, u32)> {
    let inner = t.strip_prefix("array<")?.strip_suffix('>')?;
    let (base, n) = inner.rsplit_once(", ")?;
    Some((base, n.parse().ok()?))
}

/// The scalar or vector leaves of a value of type `t`, each as the access
/// that reaches it from the whole (`.member`, `[1]`) and its type.
fn leaves(
    structs: &HashMap<String, Vec<(String, String)>>,
    t: &str,
    path: &str,
    out: &mut Vec<(String, String)>,
) {
    if let Some((base, n)) = array_of(t) {
        for i in 0..n {
            leaves(structs, base, &format!("{path}[{i}]"), out);
        }
    } else if let Some(members) = structs.get(t) {
        for (n, mt) in members {
            leaves(structs, mt, &format!("{path}.{n}"), out);
        }
    } else {
        out.push((path.to_string(), t.to_string()));
    }
}

/// The whole of type `t` composed of `names`, its leaves in order.
fn compose(
    structs: &HashMap<String, Vec<(String, String)>>,
    t: &str,
    names: &mut impl Iterator<Item = String>,
) -> String {
    if let Some((base, n)) = array_of(t) {
        let parts: Vec<_> = (0..n).map(|_| compose(structs, base, names)).collect();
        format!("{t}({})", parts.join(", "))
    } else if let Some(members) = structs.get(t) {
        let parts: Vec<_> = members
            .iter()
            .map(|(_, mt)| compose(structs, mt, names))
            .collect();
        format!("{t}({})", parts.join(", "))
    } else {
        names.next().expect("a name a leaf")
    }
}

/// A leaf's local: the phi's name, then the access as words (`_f_member`,
/// `_i1`), which no two accesses share.
fn leaf_name(phi: &str, access: &str) -> String {
    let mut out = phi.to_string();
    let mut word = String::new();
    let flush = |out: &mut String, word: &mut String, sep: &str| {
        if !word.is_empty() {
            out.push_str(sep);
            out.push_str(word);
            word.clear();
        }
    };
    let mut sep = "";
    for c in access.chars() {
        match c {
            '.' => {
                flush(&mut out, &mut word, sep);
                sep = "_f_";
            }
            '[' => {
                flush(&mut out, &mut word, sep);
                sep = "_i";
            }
            ']' => {}
            c => word.push(c),
        }
    }
    flush(&mut out, &mut word, sep);
    out
}

/// The leading spaces of `l`.
fn indent(l: &str) -> &str {
    &l[..l.len() - l.trim_start().len()]
}

/// Each array- or struct-typed `phi_*` local, of 2 to 64 leaves, a local
/// a leaf: a store of the whole a `let` then a store a leaf; a load a
/// composition of its leaves.
pub fn split_phis(text: &str) -> String {
    let structs = structs(text);
    let mut split: HashMap<String, (String, Vec<(String, String)>)> = HashMap::new();
    let mut out = Vec::new();
    let mut held = 0u32;
    for l in text.lines() {
        let t = l.trim();
        if let Some((name, ty)) = t
            .strip_prefix("var phi_")
            .and_then(|r| r.strip_suffix(';'))
            .and_then(|r| r.split_once(": "))
        {
            let name = format!("phi_{name}");
            if array_of(ty).is_some() || structs.contains_key(ty) {
                let mut ls = Vec::new();
                leaves(&structs, ty, "", &mut ls);
                if (2..=64).contains(&ls.len()) {
                    for (a, lt) in &ls {
                        out.push(format!("{}var {}: {lt};", indent(l), leaf_name(&name, a)));
                    }
                    split.insert(name, (ty.to_string(), ls));
                    continue;
                }
            }
        }
        if let Some((lhs, rhs)) = t.strip_suffix(';').and_then(|r| r.split_once(" = ")) {
            if let Some((ty, ls)) = split.get(lhs) {
                let _ = ty;
                held += 1;
                let whole = format!("split_{held}");
                out.push(format!("{}let {whole} = {rhs};", indent(l)));
                for (a, _) in ls {
                    out.push(format!("{}{} = {whole}{a};", indent(l), leaf_name(lhs, a)));
                }
                continue;
            }
            if let Some(n) = lhs.strip_prefix("let ") {
                if let Some((ty, ls)) = split.get(rhs) {
                    let mut names = ls.iter().map(|(a, _)| leaf_name(rhs, a));
                    out.push(format!(
                        "{}let {n} = {};",
                        indent(l),
                        compose(&structs, ty, &mut names)
                    ));
                    continue;
                }
            }
        }
        out.push(l.to_string());
    }
    let text = out.join("\n") + if text.ends_with('\n') { "\n" } else { "" };
    for name in split.keys() {
        let used = text
            .match_indices(name.as_str())
            .any(|(i, _)| !text[i + name.len()..].starts_with('_'));
        assert!(!used, "{name} is read or set otherwise than whole");
    }
    text
}

/// Every `array<u32|i32|f32, 2..4>` a vector of its size, but in the
/// structs a uniform or storage binding reaches, whose layout the host
/// writes.
pub fn small_arrays_as_vectors(text: &str) -> String {
    let structs = structs(text);
    let mut bound = HashSet::new();
    let mut open: Vec<String> = text
        .lines()
        .filter(|l| l.starts_with("var<uniform>") || l.starts_with("var<storage"))
        .filter_map(|l| {
            l.split_once(": ")
                .map(|(_, t)| t.trim_end_matches(';').to_string())
        })
        .collect();
    while let Some(t) = open.pop() {
        for word in t.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if let Some(members) = structs.get(word) {
                if bound.insert(word.to_string()) {
                    open.extend(members.iter().map(|(_, mt)| mt.clone()));
                }
            }
        }
    }
    let mut out = String::with_capacity(text.len());
    let mut in_bound = false;
    for l in text.split_inclusive('\n') {
        if let Some(name) = l
            .strip_prefix("struct ")
            .and_then(|r| r.trim_end().strip_suffix(" {"))
        {
            in_bound = bound.contains(name);
        }
        if in_bound {
            out.push_str(l);
        } else {
            out.push_str(&vectors(l));
        }
        if l.trim_end() == "}" {
            in_bound = false;
        }
    }
    out
}

/// `l` with each `array<S, N>`, `S` a 32-bit scalar and `N` 2 to 4, as `vecN<S>`.
fn vectors(l: &str) -> String {
    let mut out = String::with_capacity(l.len());
    let mut rest = l;
    while let Some(i) = rest.find("array<") {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let swapped = ["u32", "i32", "f32"].iter().find_map(|s| {
            (2..=4).find_map(|n| {
                let from = format!("array<{s}, {n}>");
                tail.starts_with(&from)
                    .then(|| (from.len(), format!("vec{n}<{s}>")))
            })
        });
        match swapped {
            Some((len, v)) => {
                out.push_str(&v);
                rest = &tail[len..];
            }
            None => {
                out.push_str("array<");
                rest = &tail["array<".len()..];
            }
        }
    }
    out.push_str(rest);
    out
}
