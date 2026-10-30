// xpute-tool/spec/data.rs

//! Data specs: JSON of named values written as TypeScript `as const`, as
//! Rust constants in a module `spec`, and as C++ constants. A number is f64
//! unless a `$type` says otherwise; a u64 is a hex string. `{ "$val", "$doc" }`
//! carries a reason.

use super::{
    align_star, casing_note, enum_name, file, generated, js_num, json_str, line_comments, read,
    rs_doc, rs_ident, ts_doc, Node, Output,
};

enum Enums {
    None,
    Named(&'static [&'static str]),
}

struct DataSpec {
    json: &'static str,
    ts: &'static str,
    rs: &'static str,
    enums: Enums,
    cpp: (&'static str, &'static str),
}

const DATA_SPECS: [DataSpec; 7] = [
    DataSpec {
        json: "spec/conformance/guest.json",
        ts: "ts/host/src/conformance.spec.ts",
        rs: "rust/conformance/src/guest.spec.rs",
        enums: Enums::None,
        cpp: ("cpp/conformance/guest.spec.hpp", "xpute::conformance"),
    },
    DataSpec {
        json: "spec/ipc/toc.json",
        ts: "ts/host/src/ipc/toc.spec.ts",
        rs: "rust/guest/src/ipc/toc.spec.rs",
        enums: Enums::None,
        cpp: ("cpp/xpute/guest/ipc/toc.spec.hpp", "xpute::toc"),
    },
    DataSpec {
        json: "spec/ipc/frame.json",
        ts: "ts/host/src/ipc/frame.spec.ts",
        rs: "rust/guest/src/ipc/frame.spec.rs",
        enums: Enums::None,
        cpp: ("cpp/xpute/guest/ipc/frame.spec.hpp", "xpute::frame"),
    },
    DataSpec {
        json: "spec/ipc/ring.json",
        ts: "ts/host/src/ipc/ring.spec.ts",
        rs: "rust/guest/src/ipc/ring.spec.rs",
        enums: Enums::None,
        cpp: ("cpp/xpute/guest/ipc/ring.spec.hpp", "xpute::ring"),
    },
    DataSpec {
        json: "spec/ipc/sys.json",
        ts: "ts/host/src/ipc/sys.spec.ts",
        rs: "rust/guest/src/ipc/sys.spec.rs",
        enums: Enums::None,
        cpp: ("cpp/xpute/guest/ipc/sys.spec.hpp", "xpute::sys"),
    },
    DataSpec {
        json: "spec/gpu/api.json",
        ts: "ts/gpu/src/api.spec.ts",
        rs: "rust/gpu/src/api.spec.rs",
        enums: Enums::Named(&["BINDING_KIND", "RESOURCE_KIND"]),
        cpp: ("cpp/xpute/gpu/api.spec.hpp", "xpute::gpu"),
    },
    DataSpec {
        json: "spec/gpu/enum.json",
        ts: "ts/gpu/src/enum.spec.ts",
        rs: "rust/gpu/src/enum.spec.rs",
        enums: Enums::Named(&[
            "TEXTURE_FORMAT",
            "VERTEX_FORMAT",
            "PRIMITIVE_TOPOLOGY",
            "INDEX_FORMAT",
            "FRONT_FACE",
            "CULL_MODE",
            "COMPARE_FUNCTION",
            "STENCIL_OPERATION",
            "BLEND_OPERATION",
            "BLEND_FACTOR",
            "VERTEX_STEP_MODE",
            "ADDRESS_MODE",
            "FILTER_MODE",
            "MIPMAP_FILTER_MODE",
            "TEXTURE_DIMENSION",
            "TEXTURE_VIEW_DIMENSION",
            "TEXTURE_ASPECT",
            "BUFFER_BINDING_TYPE",
            "SAMPLER_BINDING_TYPE",
            "TEXTURE_SAMPLE_TYPE",
            "STORAGE_TEXTURE_ACCESS",
            "LOAD_OP",
            "STORE_OP",
            "QUERY_TYPE",
            "CANVAS_ALPHA_MODE",
            "FEATURE_NAME",
            "LIMIT_NAME",
        ]),
        cpp: ("cpp/xpute/gpu/enum.spec.hpp", "xpute::gpu"),
    },
];

/// A TypeScript file's directory, and the name it is imported by.
const TS_LAYOUT: [(&str, &str); 2] = [
    ("ts/host/src/", "@xpute/host/"),
    ("ts/gpu/src/", "@xpute/gpu/"),
];

/// A Rust file's `src/`, and its crate's name.
const RS_LAYOUT: [(&str, &str); 3] = [
    ("rust/guest/src/", "xpute-guest"),
    ("rust/gpu/src/", "xpute-gpu"),
    ("rust/conformance/src/", "xpute-conformance"),
];

fn type_of<'a>(n: &'a Node, or: &'a str) -> &'a str {
    n.str_at("$type").unwrap_or(or)
}

fn is_ts_ident(k: &str) -> bool {
    let mut c = k.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphabetic() || f == '_' || f == '$')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '$')
}

fn ts_literal(n: &Node, ty: &str, indent: &str) -> Result<String, String> {
    if n.is_wrapper() {
        return ts_literal(n.value(), type_of(n, ty), indent);
    }
    Ok(match n {
        Node::Num(v) => format!("{} as number", js_num(*v)),
        Node::Bool(b) => b.to_string(),
        Node::Str(s) if ty == "u64" => format!("{s}n"),
        Node::Str(s) => json_str(s),
        Node::Arr(a) => format!(
            "[{}]",
            a.iter()
                .map(|e| ts_literal(e, ty, indent))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        ),
        Node::Null => "null".to_string(),
        Node::Obj(_) => {
            let inner = format!("{indent}  ");
            let t = type_of(n, ty);
            let mut lines = vec!["{".to_string()];
            for (k, v) in n.named() {
                if let Some(doc) = v.is_obj().then(|| v.lines_at("$doc")).flatten() {
                    lines.extend(ts_doc(&doc, &inner).into_iter().enumerate().map(|(i, l)| {
                        if i == 0 {
                            l
                        } else {
                            align_star(l, &inner)
                        }
                    }));
                }
                let key = if is_ts_ident(k) {
                    k.to_string()
                } else {
                    json_str(k)
                };
                lines.push(format!("{inner}{key}: {},", ts_literal(v, t, &inner)?));
            }
            lines.push(format!("{indent}}}"));
            lines.join("\n")
        }
    })
}

struct Scope {
    module: String,
    top: Vec<String>,
    enum_names: Vec<String>,
}

fn is_primitive(ty: &str) -> bool {
    matches!(
        ty,
        "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" | "str"
    )
}

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

fn rs_scalar(n: &Node, ty: &str, scope: &Scope) -> Result<(String, String), String> {
    Ok(match n {
        Node::Bool(b) => ("bool".to_string(), b.to_string()),
        Node::Str(s) => match ty {
            "u64" => ("u64".to_string(), s.clone()),
            _ => match enum_path(ty, scope) {
                None => ("&str".to_string(), json_str(s)),
                Some(path) => (path.clone(), format!("{path}::{}", rs_ident(s))),
            },
        },
        Node::Num(v) if ty == "f64" || ty == "f32" => (
            ty.to_string(),
            if v.fract() == 0.0 {
                format!("{}.0", js_num(*v))
            } else {
                js_num(*v)
            },
        ),
        Node::Num(v) => (ty.to_string(), js_num(*v)),
        _ => return Err(format!("a {ty} leaf that is not a scalar: {n:?}")),
    })
}

fn rs_enum(name: &str, variants: &[(String, String)], indent: &str, out: &mut Vec<String>) {
    let e = enum_name(name);
    out.extend([
        format!("{indent}#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]"),
        format!("{indent}#[repr(u8)]"),
        format!("{indent}pub enum {e} {{"),
    ]);
    out.extend(
        variants
            .iter()
            .map(|(v, n)| format!("{indent}    {} = {n},", rs_ident(v))),
    );
    out.extend([format!("{indent}}}"), String::new()]);
    out.extend([
        format!("{indent}impl {e} {{"),
        format!("{indent}    /// Every member, in the JSON's own order."),
        format!("{indent}    pub const ALL: [{e}; {}] = [", variants.len()),
    ]);
    out.extend(
        variants
            .iter()
            .map(|(v, _)| format!("{indent}        {e}::{},", rs_ident(v))),
    );
    out.extend([
        format!("{indent}    ];"),
        String::new(),
        format!("{indent}    /// The member a number names, or none for a number the set does not"),
        format!("{indent}    /// give out."),
        format!("{indent}    pub fn of(val: u8) -> Option<{e}> {{"),
        format!("{indent}        {e}::ALL.into_iter().find(|member| *member as u8 == val)"),
        format!("{indent}    }}"),
        format!("{indent}}}"),
    ]);
}

fn rs_items(
    name: &str,
    n: &Node,
    ty: &str,
    indent: &str,
    out: &mut Vec<String>,
    scope: &Scope,
) -> Result<(), String> {
    let doc = if n.is_obj() { n.lines_at("$doc") } else { None };
    let mut lines: Vec<String> = Vec::new();
    if n.is_wrapper() {
        rs_leaf(name, n.value(), type_of(n, ty), indent, &mut lines, scope)?;
    } else if !n.is_obj() {
        rs_leaf(name, n, ty, indent, &mut lines, scope)?;
    } else {
        let t = type_of(n, ty);
        let keys: Vec<(&str, &Node)> = n.named().collect();
        let mut body: Vec<String> = Vec::new();
        let inner = format!("{indent}    ");
        for (k, v) in &keys {
            rs_items(k, v, t, &inner, &mut body, scope)?;
        }
        body.push(format!(
            "{indent}    pub const KEYS: &[&str] = &[{}];",
            keys.iter()
                .map(|(k, _)| json_str(k))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.push(format!("{indent}pub mod {} {{", rs_ident(name)));
        lines.extend(body);
        lines.push(format!("{indent}}}"));
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

/// A member's name, as an enum takes it once made an identifier: a letter, a
/// digit or `_`, then those or `-` (WebGPU's "2d-array").
fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphanumeric() || f == '_')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-')
}

/// A member's name as a C++ enumerator: `-` an `_`, a leading digit behind
/// an `_`, a keyword followed by one.
fn cpp_ident(k: &str) -> String {
    const KEYWORDS: [&str; 12] = [
        "float", "double", "int", "char", "bool", "default", "delete", "new", "and", "or", "not",
        "xor",
    ];
    let id: String = k
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if KEYWORDS.contains(&id.as_str()) {
        format!("{id}_")
    } else if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{id}")
    } else {
        id
    }
}

fn rs_leaf(
    name: &str,
    v: &Node,
    ty: &str,
    indent: &str,
    out: &mut Vec<String>,
    scope: &Scope,
) -> Result<(), String> {
    if let Node::Arr(a) = v {
        let strings = matches!(a.first(), Some(Node::Str(_)));
        if strings && ty != "u64" && enum_path(ty, scope).is_none() {
            out.push(format!(
                "{indent}pub const {}: &[&str] = &[{}];",
                rs_ident(name),
                a.iter()
                    .map(|e| e.str().map(json_str).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if scope.enum_names.iter().any(|n| n == name)
                && a.iter().all(|e| e.str().is_some_and(is_name))
            {
                let variants: Vec<(String, String)> = a
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (e.str().unwrap_or_default().to_string(), i.to_string()))
                    .collect();
                rs_enum(name, &variants, indent, out);
            }
            return Ok(());
        }
        let items = a
            .iter()
            .map(|e| rs_scalar(e, ty, scope))
            .collect::<Result<Vec<_>, _>>()?;
        let item_ty = items.first().map(|(t, _)| t.clone()).unwrap_or_else(|| {
            if ty == "str" {
                "&str".to_string()
            } else {
                ty.to_string()
            }
        });
        out.push(format!(
            "{indent}pub const {}: [{item_ty}; {}] = [{}];",
            rs_ident(name),
            a.len(),
            items
                .iter()
                .map(|(_, l)| l.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        return Ok(());
    }
    let (t, lit) = rs_scalar(v, ty, scope)?;
    out.push(format!(
        "{indent}pub const {}: {t} = {lit};",
        rs_ident(name)
    ));
    Ok(())
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut outputs = Vec::new();
    for d in &DATA_SPECS {
        let spec = read(d.json)?;
        let names: Vec<&str> = spec.named().map(|(k, _)| k).collect();
        let head_doc = spec.lines_at("$doc");
        let ts_path = d.ts;
        let logical = TS_LAYOUT
            .iter()
            .find_map(|(dir, name)| ts_path.strip_prefix(dir).map(|p| format!("{name}{p}")))
            .unwrap_or(ts_path.to_string());
        let mut ts = vec![format!("// {logical}"), "//".into(), generated(d.json)];
        if let Some(doc) = &head_doc {
            ts.push("//".into());
            ts.extend(line_comments(doc, ""));
        }
        let mut body: Vec<String> = Vec::new();
        let mut words: Vec<String> = Vec::new();
        for name in names.iter().copied() {
            let n = spec.get(name).ok_or("a name it listed")?;
            body.push(String::new());
            if let Some(doc) = n.is_obj().then(|| n.lines_at("$doc")).flatten() {
                body.extend(ts_doc(&doc, "").into_iter().enumerate().map(|(i, l)| {
                    if i == 0 {
                        l
                    } else {
                        align_star(l, "")
                    }
                }));
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
                body.push(format!(
                    "export const {name}{} = {};",
                    annot.map(|a| format!(": {a}")).unwrap_or_default(),
                    ts_literal(n, t, "")?
                ));
            } else {
                body.push(format!(
                    "export const {name} = {} as const;",
                    ts_literal(n, "f64", "")?
                ));
            }
        }
        if !words.is_empty() {
            words.sort();
            ts.extend([
                String::new(),
                format!(
                    "import type {{ {} }} from \"@xpute/kit/abi/word.ts\";",
                    words.join(", ")
                ),
            ]);
        }
        ts.extend(body);
        outputs.push(Output::new(ts_path, file(&ts)));
        let rs_path = d.rs;
        let (crate_name, under_src) = RS_LAYOUT
            .iter()
            .find_map(|(dir, name)| rs_path.strip_prefix(dir).map(|rest| (*name, rest)))
            .ok_or(format!("{rs_path}: under no crate the layout names"))?;
        let mut rs = vec![
            format!("// {crate_name}/{under_src}"),
            "//".into(),
            generated(d.json),
        ];
        if let Some(doc) = &head_doc {
            rs.push("//".into());
            rs.extend(line_comments(doc, ""));
        }
        rs.push(String::new());
        rs.extend(casing_note());
        rs.extend([
            "// A table is generated whole, whether or not this crate calls an entry."
                .to_string(),
            format!(
                "#![allow(non_upper_case_globals, non_snake_case, clippy::absolute_paths, {}dead_code, reason = \"generated from the spec\")]",
                if matches!(d.enums, Enums::None) {
                    ""
                } else {
                    "non_camel_case_types, "
                }
            ),
        ]);
        let dir = under_src.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
        let mut module = String::from("crate");
        if !dir.is_empty() {
            module.push_str("::");
            module.push_str(&dir.replace('/', "::"));
        }
        module.push_str("::spec");
        let mut top: Vec<String> = names.iter().map(|k| enum_name(k)).collect();
        top.extend(names.iter().map(|k| k.to_string()));
        let scope = Scope {
            module,
            top,
            enum_names: match d.enums {
                Enums::Named(list) => list.iter().map(|s| s.to_string()).collect(),
                Enums::None => Vec::new(),
            },
        };
        for name in &names {
            let mut items = Vec::new();
            rs_items(
                name,
                spec.get(name).ok_or("a name it listed")?,
                "f64",
                "",
                &mut items,
                &scope,
            )?;
            if !items.is_empty() {
                rs.push(String::new());
                rs.extend(items);
            }
        }
        outputs.push(Output::new(rs_path, file(&rs)));
        let (cpp_path, namespace) = d.cpp;
        let mut cpp = vec![format!("// {cpp_path}"), "//".into(), generated(d.json)];
        if let Some(doc) = &head_doc {
            cpp.push("//".into());
            cpp.extend(line_comments(doc, ""));
        }
        let lists = names
            .iter()
            .any(|k| matches!(spec.get(k), Some(Node::Arr(_))));
        cpp.extend([String::new(), "#pragma once".into(), String::new()]);
        if lists {
            cpp.push("#include <array>".into());
        }
        cpp.push("#include <cstdint>".into());
        if lists {
            cpp.push("#include <string_view>".into());
        }
        cpp.extend([String::new(), format!("namespace {namespace} {{")]);
        let enums: &[&str] = match d.enums {
            Enums::Named(list) => list,
            Enums::None => &[],
        };
        for name in &names {
            cpp_items(
                name,
                spec.get(name).ok_or("a name it listed")?,
                "f64",
                enums.contains(&&**name),
                &mut cpp,
            )?;
        }
        cpp.extend([String::new(), format!("}} // namespace {namespace}")]);
        outputs.push(Output::new(cpp_path, file(&cpp)));
    }
    Ok(outputs)
}

/// A list of names is a `string_view` array, and with `numbered` also an
/// enum class of them numbered by place; any other non-number is refused.
fn cpp_items(
    name: &str,
    n: &Node,
    ty: &str,
    numbered: bool,
    out: &mut Vec<String>,
) -> Result<(), String> {
    let doc = if n.is_obj() { n.lines_at("$doc") } else { None };
    out.push(String::new());
    if let Some(doc) = doc {
        out.extend(line_comments(&doc, ""));
    }
    if n.is_obj() && !n.is_wrapper() {
        let t = type_of(n, ty);
        out.push(format!("namespace {name} {{"));
        for (k, v) in n.named() {
            cpp_items(k, v, t, false, out)?;
        }
        out.push(format!("}} // namespace {name}"));
        return Ok(());
    }
    if let Node::Arr(list) = if n.is_wrapper() { n.value() } else { n } {
        let names = list
            .iter()
            .map(|v| {
                v.str()
                    .ok_or(format!("{name}: a C++ list takes names only"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if numbered {
            out.push(format!("enum class {} : std::uint8_t {{", enum_name(name)));
            out.extend(
                names
                    .iter()
                    .enumerate()
                    .map(|(k, v)| format!("    {} = {k},", cpp_ident(v))),
            );
            out.extend(["};".to_string(), String::new()]);
        }
        let quoted: Vec<String> = names.iter().map(|v| json_str(v)).collect();
        out.push(format!(
            "inline constexpr std::array<std::string_view, {}> {name}{{{}}};",
            names.len(),
            quoted.join(", ")
        ));
        return Ok(());
    }
    let (val, t) = if n.is_wrapper() {
        (n.value(), type_of(n, ty))
    } else {
        (n, ty)
    };
    let number = val
        .num()
        .ok_or(format!("{name}: a C++ header takes numbers only"))?;
    let (ty, suffix) = match t {
        "u32" => ("std::uint32_t", "u"),
        "i32" => ("std::int32_t", ""),
        other => {
            return Err(format!(
                "{name}: a C++ header takes u32 and i32, not {other}"
            ))
        }
    };
    out.push(format!(
        "inline constexpr {ty} {name} = {}{suffix};",
        js_num(number)
    ));
    Ok(())
}
