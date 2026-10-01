// pxl-tool/spec/data.rs

//! Data specs. A data spec is a JSON file of named values, written as a Rust
//! module of nested `pub mod`s of constants, and — where the shell still
//! reads the same values — as a TypeScript module of `as const` objects. Most
//! of these are the kernel's alone, so they name no `ts` at all.
//!
//! A leaf is a number, a string, a boolean or an array of one kind of them; a
//! number is an f64 unless a `$type` above it says otherwise ("u8".."u64",
//! "i32", "f32"), and a u64 is written as a hex string.
//!
//! A closed set of names — a list of them, an object of entries, or names an
//! integer `$type` numbers — is also an enum, as cube_type.json's channels
//! are. A string whose `$type` names an enum (a path, or a name this spec
//! declares) is that enum's member; `"rgb"` is its packed color and `"hash"`
//! its FNV-1a. Any other string is a `&str`. A value that carries its reason
//! is `{ "$value": …, "$doc": [lines] }`; an object's own reason is its
//! `$doc` key. The file's `$doc` heads the module.

use super::{align_star, casing_note, enum_name, file, js_num, json_str, line_comments, read, rgb_literal, rs_doc, rs_ident, ts_doc, Node, Output};

/// Whether a data spec's closed sets are written as Rust enums: all of them,
/// the top-level lists named, or none.
pub enum Enums {
    None,
    All,
    Named(&'static [&'static str]),
}

struct DataSpec {
    json: &'static str,
    ts: Option<&'static str>,
    rs: Option<&'static str>,
    /// The Rust module the router declares for the file, where it is not the
    /// file's stem.
    module: Option<&'static str>,
    enums: Enums,
    /// The top-level names the TypeScript reads; the rest of the table is the
    /// Rust's alone and is not written into the shell. Absent where the
    /// TypeScript reads all of it.
    ts_names: Option<&'static [&'static str]>,
    /// The C++ header written from it, and the namespace its names are
    /// declared in: `key.SUBMISSION` of directory.json is
    /// `xpute::directory::key::SUBMISSION`.
    cpp: Option<(&'static str, &'static str)>,
}

const DATA_SPECS: [DataSpec; 10] = [
    DataSpec {
        json: "spec/xpute/conformance/guest.json",
        ts: Some("packages/xpute/conformance/src/guest.spec.ts"),
        rs: Some("crates/xpute/conformance/src/guest.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: Some(("cpp/xpute/conformance/guest.spec.hpp", "xpute::conformance")),
    },
    DataSpec {
        json: "spec/xpute/ipc/directory.json",
        ts: Some("packages/xpute/runtime/src/ipc/directory.spec.ts"),
        rs: Some("crates/xpute/runtime/src/ipc/directory.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: Some(("cpp/xpute/runtime/ipc/directory.spec.hpp", "xpute::directory")),
    },
    DataSpec {
        json: "spec/xpute/ipc/sys.json",
        ts: Some("packages/xpute/runtime/src/ipc/sys.spec.ts"),
        rs: Some("crates/xpute/runtime/src/ipc/sys.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: Some(("cpp/xpute/runtime/ipc/sys.spec.hpp", "xpute::sys")),
    },
    DataSpec {
        json: "spec/pxl/web/kernel/abi/directory.json",
        ts: Some("apps/web/src/kernel/abi/directory.spec.ts"),
        rs: Some("crates/pxl/web/kernel/src/abi/directory.spec.rs"),
        module: Some("directory"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/pxl/spec/bake/poi_badge.json",
        ts: Some("apps/web/src/feature/search/poi_badge.spec.ts"),
        rs: None,
        module: None,
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/pxl/web/kernel/layer/character/variant.json",
        ts: Some("apps/web/src/feature/character/variant.spec.ts"),
        rs: Some("crates/pxl/web/kernel/src/layer/character/variant.spec.rs"),
        module: Some("variant_spec"),
        enums: Enums::All,
        ts_names: Some(&["groups", "DEFAULT_PERSON_VARIANT_KEY", "DEFAULT_PET_VARIANT_KEY", "PERSON_GENDER_PREFIXES"]),
        cpp: None,
    },
    DataSpec {
        json: "spec/pxl/web/render/gpu/api.json",
        ts: Some("apps/web/src/kernel/render/gpu/api.spec.ts"),
        rs: Some("crates/pxl/web/render/src/gpu/api.spec.rs"),
        module: Some("api_spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/pxl/web/kernel/abi/host.json",
        ts: Some("apps/web/src/kernel/abi/host.spec.ts"),
        rs: Some("crates/pxl/web/kernel/src/abi/host.spec.rs"),
        module: Some("host"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/pxl/web/kernel/sched/profiler/profiler.json",
        ts: Some("apps/web/src/feature/debug/profiler/profiler.spec.ts"),
        rs: Some("crates/pxl/web/kernel/src/sched/profiler/profiler.spec.rs"),
        module: Some("spec"),
        enums: Enums::Named(&["CATEGORY_NAMES", "OUTCOME_NAMES", "TASK_PHASE_NAMES", "STEP_NAMES", "EFFECT_NAMES"]),
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/pxl/web/kernel/mem/mem.json",
        ts: Some("apps/web/src/kernel/mem/mem.spec.ts"),
        rs: Some("crates/pxl/web/kernel/src/mem/mem.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: Some(&["DIRECTORY_AT", "DESKTOP_BYTES", "PHONE_BYTES", "REGION_BYTES", "SUBMISSION_CAPACITY", "COMPLETION_CAPACITY"]),
        cpp: None,
    },
];

/// A value's `$type`, where it names one.
fn type_of<'a>(n: &'a Node, or: &'a str) -> &'a str {
    n.str_at("$type").unwrap_or(or)
}

fn is_ts_ident(k: &str) -> bool {
    let mut c = k.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic() || f == '_' || f == '$') && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '$')
}

/// A value as TypeScript writes it, under the `$type` it is given.
pub fn ts_literal(n: &Node, ty: &str, indent: &str) -> Result<String, String> {
    if n.is_wrapper() {
        return ts_literal(n.value(), type_of(n, ty), indent);
    }
    Ok(match n {
        Node::Num(v) => format!("{} as number", js_num(*v)),
        Node::Bool(b) => b.to_string(),
        Node::Str(s) if ty == "u64" => format!("{s}n"),
        Node::Str(s) if ty == "rgb" => rgb_literal(s)?,
        Node::Str(s) => json_str(s),
        Node::Arr(a) => format!("[{}]", a.iter().map(|e| ts_literal(e, ty, indent)).collect::<Result<Vec<_>, _>>()?.join(", ")),
        Node::Null => "null".to_string(),
        Node::Obj(_) => {
            let inner = format!("{indent}  ");
            let t = type_of(n, ty);
            let mut lines = vec!["{".to_string()];
            for (k, v) in n.named() {
                if let Some(doc) = v.is_obj().then(|| v.lines_at("$doc")).flatten() {
                    lines.extend(ts_doc(&doc, &inner).into_iter().enumerate().map(|(i, l)| if i == 0 { l } else { align_star(l, &inner) }));
                }
                let key = if is_ts_ident(k) { k.to_string() } else { json_str(k) };
                lines.push(format!("{inner}{key}: {},", ts_literal(v, t, &inner)?));
            }
            lines.push(format!("{indent}}}"));
            lines.join("\n")
        }
    })
}

/// Where the Rust of a data spec reads a name the JSON types a string with:
/// the module the spec is written as, and the top-level names it declares.
struct Scope {
    module: String,
    top: Vec<String>,
    enums: bool,
    enum_names: Vec<String>,
}

/// FNV-1a over UTF-8, 64 bits: xpute_core::kit::hash::fnv1a64_bytes.
fn fnv1a64(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("0x{h:016x}")
}

fn is_primitive(ty: &str) -> bool {
    matches!(ty, "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" | "rgb" | "hash" | "str")
}

/// Whether a `$type` names an enum: a path, or a name this spec declares.
fn enum_path(ty: &str, scope: &Scope) -> Option<String> {
    if is_primitive(ty) {
        return None;
    }
    let first = ty.split("::").next().unwrap_or(ty);
    let declared = scope.top.iter().any(|t| t == first);
    if ty.contains("::") && !declared {
        return Some(ty.to_string());
    }
    declared.then(|| format!("{}::{ty}", scope.module))
}

/// A leaf as the Rust holds it: a string is the enum member or the hash its
/// `$type` names, and otherwise the string.
fn rs_scalar(n: &Node, ty: &str, scope: &Scope) -> Result<(String, String), String> {
    Ok(match n {
        Node::Bool(b) => ("bool".to_string(), b.to_string()),
        Node::Str(s) => match ty {
            "u64" => ("u64".to_string(), s.clone()),
            "rgb" => ("u32".to_string(), rgb_literal(s)?),
            "hash" => ("u64".to_string(), fnv1a64(s)),
            _ => match enum_path(ty, scope) {
                None => ("&str".to_string(), json_str(s)),
                Some(path) => (path.clone(), format!("{path}::{}", rs_ident(s))),
            },
        },
        Node::Num(v) if ty == "f64" || ty == "f32" => (ty.to_string(), if v.fract() == 0.0 { format!("{}.0", js_num(*v)) } else { js_num(*v) }),
        Node::Num(v) => (ty.to_string(), js_num(*v)),
        _ => return Err(format!("a {ty} leaf that is not a scalar: {n:?}")),
    })
}

/// A closed set of names as the Rust names it, the way cube_type.json's
/// channels are: an enum whose members are the JSON's names, numbered as the
/// JSON numbers them — its own numbers where it gives them, its order where
/// it lists them.
fn rs_enum(name: &str, variants: &[(String, String)], indent: &str, out: &mut Vec<String>) {
    let e = enum_name(name);
    out.extend([
        format!("{indent}#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]"),
        format!("{indent}#[repr(u8)]"),
        format!("{indent}pub enum {e} {{"),
    ]);
    out.extend(variants.iter().map(|(v, n)| format!("{indent}    {} = {n},", rs_ident(v))));
    out.extend([format!("{indent}}}"), String::new()]);
    out.extend([
        format!("{indent}impl {e} {{"),
        format!("{indent}    /// Every member, in the JSON's own order."),
        format!("{indent}    pub const ALL: [{e}; {}] = [", variants.len()),
    ]);
    out.extend(variants.iter().map(|(v, _)| format!("{indent}        {e}::{},", rs_ident(v))));
    out.extend([
        format!("{indent}    ];"),
        String::new(),
        format!("{indent}    /// The member a number names, or none for a number the set does not"),
        format!("{indent}    /// give out."),
        format!("{indent}    pub fn of(value: u8) -> Option<{e}> {{"),
        format!("{indent}        {e}::ALL.into_iter().find(|member| *member as u8 == value)"),
        format!("{indent}    }}"),
        format!("{indent}}}"),
    ]);
}

fn rs_items(name: &str, n: &Node, ty: &str, indent: &str, out: &mut Vec<String>, scope: &Scope) -> Result<(), String> {
    let doc = if n.is_obj() { n.lines_at("$doc") } else { None };
    let mut lines: Vec<String> = Vec::new();
    if n.is_wrapper() {
        rs_leaf(name, n.value(), type_of(n, ty), indent, &mut lines, scope)?;
    } else if !n.is_obj() {
        rs_leaf(name, n, ty, indent, &mut lines, scope)?;
    } else {
        let t = type_of(n, ty);
        let keys: Vec<(&str, &Node)> = n.named().collect();
        // A catalog: names numbered as decor.json numbers its types — its
        // numbers stay constants, as cube_type.spec.rs keeps both, and are one
        // enum beside them where no two names share one — or a set of entries
        // each an object of its own, numbered in order.
        let numbers: Vec<Option<f64>> = keys.iter().map(|(_, v)| v.num()).collect();
        let numbered = scope.enums && !keys.is_empty() && matches!(t, "u8" | "u16" | "u32") && numbers.iter().all(|v| v.is_some_and(|v| v < 256.0));
        let distinct = numbers.iter().enumerate().all(|(i, a)| numbers[..i].iter().all(|b| b != a));
        if numbered && distinct {
            rs_enum(
                name,
                &keys.iter().map(|(k, v)| (k.to_string(), js_num(v.num().unwrap_or_default()))).collect::<Vec<_>>(),
                indent,
                &mut lines,
            );
            lines.push(String::new());
        } else if scope.enums && !keys.is_empty() && keys.iter().all(|(_, v)| v.is_obj() && v.get("$number").and_then(Node::num).is_some()) {
            // A set of entries, each naming its own number as decor.json
            // numbers its kinds; an object whose entries carry none is a
            // structure, not a set.
            let variants: Vec<(String, String)> = keys.iter().map(|(k, v)| (k.to_string(), js_num(v.get("$number").and_then(Node::num).unwrap_or_default()))).collect();
            rs_enum(name, &variants, indent, &mut lines);
            lines.push(String::new());
        }
        let mut body: Vec<String> = Vec::new();
        let inner = format!("{indent}    ");
        for (k, v) in &keys {
            rs_items(k, v, t, &inner, &mut body, scope)?;
        }
        // Each object's keys are also listed as its `KEYS`, where its closed
        // sets are not enums.
        if !scope.enums {
            body.push(format!(
                "{indent}    pub const KEYS: &[&str] = &[{}];",
                keys.iter().map(|(k, _)| json_str(k)).collect::<Vec<_>>().join(", ")
            ));
        }
        if !body.is_empty() {
            lines.push(format!("{indent}pub mod {} {{", rs_ident(name)));
            lines.extend(body);
            lines.push(format!("{indent}}}"));
        }
    }
    if lines.is_empty() {
        return Ok(());
    }
    if let Some(doc) = doc {
        out.extend(rs_doc(&doc, indent, "///"));
    }
    out.extend(lines);
    Ok(())
}

fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic() || f == '_') && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-')
}

fn rs_leaf(name: &str, v: &Node, ty: &str, indent: &str, out: &mut Vec<String>, scope: &Scope) -> Result<(), String> {
    if let Node::Arr(a) = v {
        let strings = matches!(a.first(), Some(Node::Str(_)));
        if strings && ty != "u64" && enum_path(ty, scope).is_none() && ty != "rgb" && ty != "hash" {
            out.push(format!(
                "{indent}pub const {}: &[&str] = &[{}];",
                rs_ident(name),
                a.iter().map(|e| e.str().map(json_str).unwrap_or_default()).collect::<Vec<_>>().join(", ")
            ));
            // A list of names is a closed set, and an enum beside it.
            if (scope.enums || scope.enum_names.iter().any(|n| n == name)) && a.iter().all(|e| e.str().is_some_and(is_name)) {
                let variants: Vec<(String, String)> = a.iter().enumerate().map(|(i, e)| (e.str().unwrap_or_default().to_string(), i.to_string())).collect();
                rs_enum(name, &variants, indent, out);
            }
            return Ok(());
        }
        let items = a.iter().map(|e| rs_scalar(e, ty, scope)).collect::<Result<Vec<_>, _>>()?;
        let item_ty = items.first().map(|(t, _)| t.clone()).unwrap_or_else(|| if ty == "str" { "&str".to_string() } else { ty.to_string() });
        out.push(format!(
            "{indent}pub const {}: [{item_ty}; {}] = [{}];",
            rs_ident(name),
            a.len(),
            items.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>().join(", ")
        ));
        return Ok(());
    }
    let (t, lit) = rs_scalar(v, ty, scope)?;
    out.push(format!("{indent}pub const {}: {t} = {lit};", rs_ident(name)));
    Ok(())
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut outputs = Vec::new();
    for d in &DATA_SPECS {
        let spec = read(d.json)?;
        let base = d.json.rsplit('/').next().unwrap_or(d.json);
        let names: Vec<&str> = spec.named().map(|(k, _)| k).collect();
        let head_doc = spec.lines_at("$doc");
        if let Some(ts_path) = d.ts {
            let logical = [
                ("apps/web/src/", "@pxl/web/"),
                ("packages/xpute/runtime/src/", "@xpute/runtime/"),
                ("packages/xpute/conformance/src/", "@xpute/conformance/"),
            ]
            .iter()
            .find_map(|(dir, name)| ts_path.strip_prefix(dir).map(|p| format!("{name}{p}")))
            .unwrap_or(ts_path.to_string());
            let mut ts = vec![format!("// {logical}"), "//".into(), format!("// GENERATED by pxl-tool/bin/spec.rs from {} — do not edit.", d.json)];
            if let Some(doc) = &head_doc {
                ts.push("//".into());
                ts.extend(line_comments(doc, ""));
            }
            let mut body: Vec<String> = Vec::new();
            let mut words: Vec<String> = Vec::new();
            if let Some(listed) = d.ts_names {
                let unknown: Vec<&str> = listed.iter().copied().filter(|n| !names.contains(n)).collect();
                if !unknown.is_empty() {
                    return Err(format!("{}: tsNames lists {}, which the table does not have", d.json, unknown.join(", ")));
                }
            }
            for name in names.iter().copied().filter(|n| d.ts_names.is_none_or(|l| l.contains(n))) {
                let n = spec.get(name).ok_or("a name it listed")?;
                body.push(String::new());
                if let Some(doc) = n.is_obj().then(|| n.lines_at("$doc")).flatten() {
                    body.extend(ts_doc(&doc, "").into_iter().enumerate().map(|(i, l)| if i == 0 { l } else { align_star(l, "") }));
                }
                if n.is_wrapper() && !matches!(n.value(), Node::Arr(_) | Node::Obj(_) | Node::Null) {
                    let t = type_of(n, "f64");
                    let annot = (matches!(n.value(), Node::Num(_)) || t == "u64").then_some(t);
                    let annot = annot.filter(|a| *a != "u64");
                    if let Some(a) = annot {
                        if !words.iter().any(|w| w == a) {
                            words.push(a.to_string());
                        }
                    }
                    body.push(format!("export const {name}{} = {};", annot.map(|a| format!(": {a}")).unwrap_or_default(), ts_literal(n, t, "")?));
                } else {
                    body.push(format!("export const {name} = {} as const;", ts_literal(n, "f64", "")?));
                    // A set numbered by its entries' `$number`, as the Rust
                    // enum is: its members' numbers, the way decor.spec.ts
                    // lists decor_types.
                    let entries: Vec<(&str, &Node)> = if n.is_obj() { n.named().collect() } else { Vec::new() };
                    if matches!(d.enums, Enums::All) && !entries.is_empty() && entries.iter().all(|(_, v)| v.is_obj() && v.get("$number").and_then(Node::num).is_some()) {
                        body.extend([String::new(), format!("export const {} = {{", enum_name(name))]);
                        body.extend(entries.iter().map(|(k, v)| format!("  {k}: {},", js_num(v.get("$number").and_then(Node::num).unwrap_or_default()))));
                        body.push("} as const;".into());
                    }
                }
            }
            if !words.is_empty() {
                words.sort();
                ts.extend([String::new(), format!("import type {{ {} }} from \"@xpute/core/abi/word.ts\";", words.join(", "))]);
            }
            ts.extend(body);
            outputs.push(Output::new(ts_path, file(&ts)));
        }
        if let Some(rs_path) = d.rs {
            let (crate_name, under_src) = [
                ("crates/pxl/web/kernel/src/", "pxl-kernel"),
                ("crates/pxl/web/render/src/", "pxl-render"),
                ("crates/xpute/runtime/src/", "xpute-runtime"),
                ("crates/xpute/conformance/src/", "xpute-conformance"),
            ]
            .iter()
            .find_map(|(dir, name)| rs_path.strip_prefix(dir).map(|rest| (*name, rest)))
            .ok_or(format!("{rs_path}: not under pxl-kernel, pxl-render, xpute-runtime or xpute-conformance"))?;
            let mut rs = vec![
                format!("// {crate_name}/{under_src}"),
                "//".into(),
                format!("// GENERATED by pxl-tool/bin/spec.rs from {base} — do not edit."),
            ];
            if let Some(doc) = &head_doc {
                rs.push("//".into());
                rs.extend(line_comments(doc, ""));
            }
            rs.push(String::new());
            rs.extend(casing_note());
            rs.extend([
                "//".to_string(),
                "// `dead_code` because a table is generated whole: an entry earns its".into(),
                "// place in the JSON, not by having a caller in this crate.".into(),
                format!(
                    "#![allow(non_upper_case_globals, non_snake_case, {}dead_code)]",
                    if matches!(d.enums, Enums::None) { "" } else { "non_camel_case_types, " }
                ),
            ]);
            let dir = under_src.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
            let stem = under_src.rsplit('/').next().unwrap_or(under_src).trim_end_matches(".spec.rs");
            let mut module = String::from("crate");
            if !dir.is_empty() {
                module.push_str("::");
                module.push_str(&dir.replace('/', "::"));
            }
            module.push_str("::");
            module.push_str(d.module.unwrap_or(stem));
            let mut top: Vec<String> = names.iter().map(|k| enum_name(k)).collect();
            top.extend(names.iter().map(|k| k.to_string()));
            let scope = Scope {
                module,
                top,
                enums: matches!(d.enums, Enums::All),
                enum_names: match d.enums {
                    Enums::Named(list) => list.iter().map(|s| s.to_string()).collect(),
                    _ => Vec::new(),
                },
            };
            for name in &names {
                let mut items = Vec::new();
                rs_items(name, spec.get(name).ok_or("a name it listed")?, "f64", "", &mut items, &scope)?;
                if !items.is_empty() {
                    rs.push(String::new());
                    rs.extend(items);
                }
            }
            outputs.push(Output::new(rs_path, file(&rs)));
        }
        if let Some((cpp_path, namespace)) = d.cpp {
            let mut cpp = vec![format!("// {cpp_path}"), "//".into(), format!("// GENERATED by pxl-tool/bin/spec.rs from {} — do not edit.", d.json)];
            if let Some(doc) = &head_doc {
                cpp.push("//".into());
                cpp.extend(line_comments(doc, ""));
            }
            cpp.extend([
                String::new(),
                "#pragma once".into(),
                String::new(),
                "#include <cstdint>".into(),
                String::new(),
                format!("namespace {namespace} {{"),
            ]);
            for name in &names {
                cpp_items(name, spec.get(name).ok_or("a name it listed")?, "f64", &mut cpp)?;
            }
            cpp.extend([String::new(), format!("}} // namespace {namespace}")]);
            outputs.push(Output::new(cpp_path, file(&cpp)));
        }
    }
    Ok(outputs)
}

/// A name's C++ constants: a number each, an object's entries in a
/// namespace of its name. What is not a number — a string, an array — is
/// refused, since nothing a C++ header carries is one yet.
fn cpp_items(name: &str, n: &Node, ty: &str, out: &mut Vec<String>) -> Result<(), String> {
    let doc = if n.is_obj() { n.lines_at("$doc") } else { None };
    out.push(String::new());
    if let Some(doc) = doc {
        out.extend(line_comments(&doc, ""));
    }
    if n.is_obj() && !n.is_wrapper() {
        let t = type_of(n, ty);
        out.push(format!("namespace {name} {{"));
        for (k, v) in n.named() {
            cpp_items(k, v, t, out)?;
        }
        out.push(format!("}} // namespace {name}"));
        return Ok(());
    }
    let (value, t) = if n.is_wrapper() { (n.value(), type_of(n, ty)) } else { (n, ty) };
    let number = value.num().ok_or(format!("{name}: a C++ header takes numbers only"))?;
    let (ty, suffix) = match t {
        "u32" => ("std::uint32_t", "u"),
        "i32" => ("std::int32_t", ""),
        other => return Err(format!("{name}: a C++ header takes u32 and i32, not {other}")),
    };
    out.push(format!("inline constexpr {ty} {name} = {}{suffix};", js_num(number)));
    Ok(())
}
