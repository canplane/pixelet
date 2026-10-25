// pxl-tool/spec/data.rs

//! Data specs: JSON of named values written as Rust constants and, where the
//! shell reads them, TypeScript `as const`. A number is f64 unless a `$type`
//! says otherwise; a u64 is a hex string. `{ "$value", "$doc" }` carries a reason.
//! pixelet's Rust carries no names: a list of them is its count, and an enum
//! where the spec asks for one, and a table's keys are not listed; the
//! TypeScript keeps them for what a person reads. xpute's Rust keeps its own.

use super::{align_star, casing_note, enum_name, file, js_num, json_str, laid_out, line_comments, read, rgb_literal, rs_doc, rs_ident, ts_doc, Node, Output};

pub enum Enums {
    None,
    All,
    Named(&'static [&'static str]),
}

struct DataSpec {
    json: &'static str,
    /// The specs whose names its expressions may read, each name owned by
    /// one file in scope.
    uses: &'static [&'static str],
    ts: Option<&'static str>,
    rs: Option<&'static str>,
    module: Option<&'static str>,
    enums: Enums,
    /// Absent where the TypeScript reads all of it.
    ts_names: Option<&'static [&'static str]>,
    cpp: Option<(&'static str, &'static str)>,
}

const DATA_SPECS: [DataSpec; 25] = [
    DataSpec {
        json: "spec/spec/space/earth.json",
        uses: &[],
        ts: Some("apps/client/src/config/earth.spec.ts"),
        rs: Some("crates/spec/src/space/coord/earth.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/spec/space/geoid.json",
        uses: &[],
        ts: None,
        rs: Some("crates/spec/src/space/coord/geoid.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/spec/block/ext.json",
        uses: &[],
        ts: Some("apps/client/src/catalog/block/ext.spec.ts"),
        rs: Some("crates/spec/src/block/ext.spec.rs"),
        module: Some("ext"),
        enums: Enums::All,
        ts_names: Some(&["MARKER_TEXT_CHARS"]),
        cpp: None,
    },
    DataSpec {
        json: "spec/spec/asset/marker.json",
        uses: &[],
        ts: None,
        rs: Some("crates/spec/src/block/marker.spec.rs"),
        module: Some("marker"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/spec/asset/id.json",
        uses: &[],
        ts: None,
        rs: Some("crates/spec/src/block/id.spec.rs"),
        module: Some("id"),
        enums: Enums::All,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/spec/division.json",
        uses: &[],
        ts: Some("apps/client/src/shell/world/division/division.spec.ts"),
        rs: Some("crates/spec/src/division.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "xpute/spec/conformance/guest.json",
        uses: &[],
        ts: Some("xpute/ts/conformance/src/guest.spec.ts"),
        rs: Some("xpute/rust/conformance/src/guest.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: Some(("xpute/cpp/conformance/guest.spec.hpp", "xpute::conformance")),
    },
    DataSpec {
        json: "xpute/spec/ipc/directory.json",
        uses: &[],
        ts: Some("xpute/ts/host/src/ipc/directory.spec.ts"),
        rs: Some("xpute/rust/guest/src/ipc/directory.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: Some(("xpute/cpp/guest/ipc/directory.spec.hpp", "xpute::directory")),
    },
    DataSpec {
        json: "xpute/spec/ipc/frame.json",
        uses: &[],
        ts: Some("xpute/ts/host/src/ipc/frame.spec.ts"),
        rs: Some("xpute/rust/guest/src/ipc/frame.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: Some(("xpute/cpp/guest/ipc/frame.spec.hpp", "xpute::frame")),
    },
    DataSpec {
        json: "xpute/spec/ipc/ring.json",
        uses: &[],
        ts: Some("xpute/ts/host/src/ipc/ring.spec.ts"),
        rs: Some("xpute/rust/guest/src/ipc/ring.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: Some(("xpute/cpp/guest/ipc/ring.spec.hpp", "xpute::ring")),
    },
    DataSpec {
        json: "xpute/spec/ipc/sys.json",
        uses: &[],
        ts: Some("xpute/ts/host/src/ipc/sys.spec.ts"),
        rs: Some("xpute/rust/guest/src/ipc/sys.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: Some(("xpute/cpp/guest/ipc/sys.spec.hpp", "xpute::sys")),
    },
    DataSpec {
        json: "xpute/spec/gpu/api.json",
        uses: &[],
        ts: Some("xpute/ts/gpu/src/api.spec.ts"),
        rs: Some("xpute/rust/gpu/src/api.spec.rs"),
        module: Some("spec"),
        enums: Enums::Named(&["BINDING_KIND", "RESOURCE_KIND"]),
        ts_names: None,
        cpp: Some(("xpute/cpp/gpu/api.spec.hpp", "xpute::gpu")),
    },
    DataSpec {
        json: "xpute/spec/gpu/enum.json",
        uses: &[],
        ts: Some("xpute/ts/gpu/src/enum.spec.ts"),
        rs: Some("xpute/rust/gpu/src/enum.spec.rs"),
        module: Some("spec"),
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
        ts_names: None,
        cpp: Some(("xpute/cpp/gpu/enum.spec.hpp", "xpute::gpu")),
    },
    DataSpec {
        json: "xpute/spec/sched/tick.json",
        uses: &[],
        ts: Some("xpute/ts/host/src/sched/tick.spec.ts"),
        rs: Some("xpute/rust/guest/src/sched/tick.spec.rs"),
        module: Some("spec"),
        enums: Enums::Named(&["PHASES"]),
        ts_names: None,
        cpp: Some(("xpute/cpp/guest/sched/tick.spec.hpp", "xpute")),
    },
    DataSpec {
        json: "spec/client/kernel/door/directory.json",
        uses: &[],
        ts: Some("apps/client/src/abi/directory.spec.ts"),
        rs: Some("crates/client/kernel/src/door/directory.spec.rs"),
        module: Some("directory"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/spec/bake/place_theme.json",
        uses: &[],
        ts: Some("apps/client/src/shell/search/place_theme.spec.ts"),
        rs: None,
        module: None,
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/client/kernel/domain/query/model/variant.json",
        uses: &[],
        ts: Some("apps/client/src/shell/character/variant.spec.ts"),
        rs: Some("crates/client/kernel/src/domain/query/model/variant.spec.rs"),
        module: Some("variant_spec"),
        enums: Enums::All,
        ts_names: Some(&["groups", "PERSON_GENDER_PREFIXES"]),
        cpp: None,
    },
    DataSpec {
        json: "spec/client/kernel/domain/scene/indicator/gizmo.json",
        uses: &[],
        ts: Some("apps/client/src/shell/debug/gizmo.spec.ts"),
        rs: Some("crates/client/kernel/src/domain/scene/indicator/gizmo.spec.rs"),
        module: Some("gizmo_spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/client/render/gpu/api.json",
        uses: &[],
        ts: Some("apps/client/src/abi/render/gpu/api.spec.ts"),
        rs: Some("crates/client/render/src/gpu/api.spec.rs"),
        module: Some("api_spec"),
        enums: Enums::None,
        ts_names: Some(&["surface_role", "stream"]),
        cpp: None,
    },
    DataSpec {
        json: "spec/client/kernel/sys/io/host.json",
        uses: &[],
        ts: Some("apps/client/src/abi/host.spec.ts"),
        rs: Some("crates/client/kernel/src/sys/io/host.spec.rs"),
        module: Some("host_spec"),
        enums: Enums::None,
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/client/kernel/sys/io/demand.json",
        uses: &[],
        ts: Some("apps/client/src/abi/io/demand.spec.ts"),
        rs: Some("crates/client/kernel/src/sys/io/demand.spec.rs"),
        module: Some("demand_spec"),
        enums: Enums::Named(&["STREAM_NAMES", "OUTCOME_NAMES", "TASK_PHASE_NAMES"]),
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/client/kernel/turn/step.json",
        uses: &[],
        ts: Some("apps/client/src/abi/turn/step.spec.ts"),
        rs: Some("crates/client/kernel/src/turn/step.spec.rs"),
        module: Some("spec"),
        enums: Enums::Named(&["STEP_NAMES", "EFFECT_NAMES"]),
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/client/render/gpu/span.json",
        uses: &[],
        ts: Some("apps/client/src/abi/render/gpu/span.spec.ts"),
        rs: Some("crates/client/render/src/gpu/span.spec.rs"),
        module: Some("span_spec"),
        enums: Enums::Named(&["GPU_SPAN_NAMES", "FRAME_KINDS"]),
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/client/kernel/profiler/profiler.json",
        uses: &[
            "xpute/spec/sched/tick.json",
            "spec/client/kernel/sys/io/demand.json",
            "spec/client/kernel/turn/step.json",
            "spec/client/render/gpu/span.json",
            "xpute/spec/gpu/api.json",
        ],
        ts: Some("apps/client/src/shell/debug/profiler/profiler.spec.ts"),
        rs: Some("crates/client/kernel/src/profiler/profiler.spec.rs"),
        module: Some("spec"),
        enums: Enums::Named(&["WAKES"]),
        ts_names: None,
        cpp: None,
    },
    DataSpec {
        json: "spec/client/kernel/sys/mem/mem.json",
        uses: &[],
        ts: Some("apps/client/src/abi/mem/mem.spec.ts"),
        rs: Some("crates/client/kernel/src/sys/mem/mem.spec.rs"),
        module: Some("spec"),
        enums: Enums::None,
        ts_names: Some(&["DIRECTORY_AT", "DESKTOP_BYTES", "PHONE_BYTES", "REGION_BYTES", "SUBMISSION_CAPACITY", "COMPLETION_CAPACITY"]),
        cpp: None,
    },
];

fn type_of<'a>(n: &'a Node, or: &'a str) -> &'a str {
    n.str_at("$type").unwrap_or(or)
}

fn is_ts_ident(k: &str) -> bool {
    let mut c = k.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic() || f == '_' || f == '$') && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '$')
}

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

struct Scope {
    module: String,
    top: Vec<String>,
    enums: bool,
    enum_names: Vec<String>,
    /// Names are carried as strings, as xpute's own tables are; pixelet's
    /// Rust gets a list's count in their place.
    names: bool,
}

fn is_primitive(ty: &str) -> bool {
    matches!(ty, "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" | "rgb" | "str")
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
            "rgb" => ("u32".to_string(), rgb_literal(s)?),
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
            // An object whose entries carry no `$number` is a structure, not a set.
            let variants: Vec<(String, String)> = keys.iter().map(|(k, v)| (k.to_string(), js_num(v.get("$number").and_then(Node::num).unwrap_or_default()))).collect();
            rs_enum(name, &variants, indent, &mut lines);
            lines.push(String::new());
        }
        let mut body: Vec<String> = Vec::new();
        let inner = format!("{indent}    ");
        for (k, v) in &keys {
            rs_items(k, v, t, &inner, &mut body, scope)?;
        }
        if !scope.enums && scope.names {
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

/// A member's name, as an enum takes it once made an identifier: a letter, a
/// digit or `_`, then those or `-` (WebGPU's "2d-array").
fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|f| f.is_ascii_alphanumeric() || f == '_') && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-')
}

/// A member's name as a C++ enumerator: `-` an `_`, a leading digit behind
/// an `_`, a keyword followed by one.
fn cpp_ident(k: &str) -> String {
    const KEYWORDS: [&str; 12] = ["float", "double", "int", "char", "bool", "default", "delete", "new", "and", "or", "not", "xor"];
    let id: String = k.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if KEYWORDS.contains(&id.as_str()) {
        format!("{id}_")
    } else if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{id}")
    } else {
        id
    }
}

fn rs_leaf(name: &str, v: &Node, ty: &str, indent: &str, out: &mut Vec<String>, scope: &Scope) -> Result<(), String> {
    if let Node::Arr(a) = v {
        let strings = matches!(a.first(), Some(Node::Str(_)));
        if strings && ty != "u64" && enum_path(ty, scope).is_none() && ty != "rgb" {
            out.push(if scope.names {
                format!(
                    "{indent}pub const {}: &[&str] = &[{}];",
                    rs_ident(name),
                    a.iter().map(|e| e.str().map(json_str).unwrap_or_default()).collect::<Vec<_>>().join(", ")
                )
            } else {
                // How many there are, as the spec's widths read a list's name.
                format!("{indent}pub const {}: usize = {};", rs_ident(name), a.len())
            });
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

/// The names `json` may read past its own, from the specs it uses: each
/// laid out over its own uses, a name owned by one file, no use circular.
fn used(json: &str, path: &mut Vec<&'static str>) -> Result<Vec<(String, Node)>, String> {
    let d = DATA_SPECS.iter().find(|d| d.json == json).ok_or(format!("{json}: uses a spec that is not listed"))?;
    if path.contains(&d.json) {
        return Err(format!("{}: uses itself through {}", d.json, path.join(" → ")));
    }
    path.push(d.json);
    let mut names: Vec<(String, Node)> = Vec::new();
    for &u in d.uses {
        let below = used(u, path)?;
        let Node::Obj(entries) = laid_out(read(u)?, &below)? else {
            return Err(format!("{u}: not a table"));
        };
        for (k, v) in entries.into_iter().filter(|(k, _)| !k.starts_with('$')) {
            if names.iter().any(|(n, _)| *n == k) {
                return Err(format!("{}: `{k}` is named in two of the specs it uses", d.json));
            }
            names.push((k, v));
        }
    }
    path.pop();
    Ok(names)
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut outputs = Vec::new();
    for d in &DATA_SPECS {
        let uses = used(d.json, &mut Vec::new())?;
        let spec = laid_out(read(d.json)?, &uses)?;
        if let Some(k) = spec.named().map(|(k, _)| k).find(|k| uses.iter().any(|(u, _)| u == k)) {
            return Err(format!("{}: `{k}` is named here and in a spec it uses", d.json));
        }
        let base = d.json.rsplit('/').next().unwrap_or(d.json);
        let names: Vec<&str> = spec.named().map(|(k, _)| k).collect();
        let head_doc = spec.lines_at("$doc");
        if let Some(ts_path) = d.ts {
            let logical = [
                ("apps/client/src/", "@pxl/client/"),
                ("xpute/ts/host/src/", "@xpute/host/"),
                ("xpute/ts/gpu/src/", "@xpute/gpu/"),
                ("xpute/ts/conformance/src/", "@xpute/conformance/"),
            ]
            .iter()
            .find_map(|(dir, name)| ts_path.strip_prefix(dir).map(|p| format!("{name}{p}")))
            .unwrap_or(ts_path.to_string());
            let mut ts = vec![format!("// {logical}"), "//".into(), super::generated(d.json, d.json)];
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
                ts.extend([String::new(), format!("import type {{ {} }} from \"@xpute/kit/abi/word.ts\";", words.join(", "))]);
            }
            ts.extend(body);
            outputs.push(Output::new(ts_path, file(&ts)));
        }
        if let Some(rs_path) = d.rs {
            let (crate_name, under_src) = [
                ("crates/client/kernel/src/", "pxl-kernel"),
                ("crates/client/render/src/", "pxl-render"),
                ("crates/spec/src/", "pxl-spec"),
                ("xpute/rust/guest/src/", "xpute-guest"),
                ("xpute/rust/gpu/src/", "xpute-gpu"),
                ("xpute/rust/conformance/src/", "xpute-conformance"),
            ]
            .iter()
            .find_map(|(dir, name)| rs_path.strip_prefix(dir).map(|rest| (*name, rest)))
            .ok_or(format!("{rs_path}: not under pxl-kernel, pxl-render, pxl-spec, xpute-guest, xpute-gpu or xpute-conformance"))?;
            let mut rs = vec![format!("// {crate_name}/{under_src}"), "//".into(), super::generated(d.json, base)];
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
                names: crate_name.starts_with("xpute-"),
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
            let mut cpp = vec![format!("// {}", super::in_xpute(cpp_path)), "//".into(), super::generated(d.json, d.json)];
            if let Some(doc) = &head_doc {
                cpp.push("//".into());
                cpp.extend(line_comments(doc, ""));
            }
            let lists = names.iter().any(|k| matches!(spec.get(k), Some(Node::Arr(_))));
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
                _ => &[],
            };
            for name in &names {
                cpp_items(name, spec.get(name).ok_or("a name it listed")?, "f64", enums.contains(&&**name), &mut cpp)?;
            }
            cpp.extend([String::new(), format!("}} // namespace {namespace}")]);
            outputs.push(Output::new(cpp_path, file(&cpp)));
        }
    }
    Ok(outputs)
}

/// A list of names is a `string_view` array, and with `numbered` also an
/// enum class of them numbered by place; any other non-number is refused.
fn cpp_items(name: &str, n: &Node, ty: &str, numbered: bool, out: &mut Vec<String>) -> Result<(), String> {
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
        let names = list.iter().map(|v| v.str().ok_or(format!("{name}: a C++ list takes names only"))).collect::<Result<Vec<_>, _>>()?;
        if numbered {
            out.push(format!("enum class {} : std::uint8_t {{", enum_name(name)));
            out.extend(names.iter().enumerate().map(|(k, v)| format!("    {} = {k},", cpp_ident(v))));
            out.extend(["};".to_string(), String::new()]);
        }
        let quoted: Vec<String> = names.iter().map(|v| json_str(v)).collect();
        out.push(format!("inline constexpr std::array<std::string_view, {}> {name}{{{}}};", names.len(), quoted.join(", ")));
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
