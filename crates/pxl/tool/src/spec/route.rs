// pxl-tool/spec/route.rs

//! The address space. One JSON for every root and branch, written into the
//! three places that build a URL or read one — the client's shell, its
//! kernel, and the Worker — and checked against the two that can only carry
//! literals: wrangler's own `run_worker_first`, and the deploy guard's
//! sentinels. Five copies of one decision was what this replaced, and the
//! split that made them five was exactly the shape
//! issue/20260910-one-server-root.md was written about.

use regex::Regex;

use super::{json_str, line_comments, read, ts_doc, Node, Output};
use crate::root;

const JSON: &str = "spec/pxl/spec/route.json";

/// The quoted strings of the first `[…]` that follows `pattern` in a file,
/// sorted.
fn listed(path: &str, pattern: &str, what: &str) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(root().join(path)).map_err(|e| format!("{path}: {e}"))?;
    let re = Regex::new(pattern).map_err(|e| e.to_string())?;
    let found = re.captures(&text).ok_or(format!("{path}: no {what} to check against {JSON}"))?;
    let quoted = Regex::new(r#""([^"]+)""#).map_err(|e| e.to_string())?;
    let mut out: Vec<String> = quoted.captures_iter(&found[1]).map(|c| c[1].to_string()).collect();
    out.sort();
    Ok(out)
}

fn json_list(xs: &[String]) -> String {
    serde_json::to_string(xs).unwrap_or_default()
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let spec = read(JSON)?;
    let roots: Vec<(&str, &Node)> = spec.get("roots").map(|r| r.entries().iter().map(|(k, v)| (k.as_str(), v)).collect()).unwrap_or_default();
    let branches: Vec<(&str, &Node)> = spec.get("branches").map(|b| b.entries().iter().map(|(k, v)| (k.as_str(), v)).collect()).unwrap_or_default();

    let mut paths: Vec<(String, String)> = roots.iter().map(|(n, r)| (n.to_string(), r.str_at("path").unwrap_or_default().to_string())).collect();
    for (name, b) in &branches {
        let under = b.str_at("under").unwrap_or_default();
        let base = paths
            .iter()
            .find(|(n, _)| n == under)
            .map(|(_, p)| p.clone())
            .ok_or(format!("{JSON}: {name} is under {under}, which is not a root"))?;
        paths.push((name.to_string(), format!("{base}/{}", b.str_at("name").unwrap_or_default())));
    }
    let path = |name: &str| paths.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone()).unwrap_or_default();
    let doc = |name: &str| roots.iter().chain(branches.iter()).find(|(n, _)| *n == name).and_then(|(_, r)| r.lines_at("$doc"));

    // Every root is one entry in the edge's own list, which is what keeps a
    // relay reading a prefix rather than a route.
    let worker_first: Vec<String> = roots.iter().map(|(n, _)| format!("{}/*", path(n))).collect();

    // ---- what only carries literals, checked rather than written ----
    let declared = listed("apps/web/wrangler.jsonc", r#""run_worker_first"\s*:\s*\[([^\]]*)\]"#, "run_worker_first")?;
    let mut claimed = worker_first.clone();
    claimed.sort();
    if declared != claimed {
        return Err(format!(
            "apps/web/wrangler.jsonc's run_worker_first is {} where {JSON}'s roots are {}. A root the edge does not claim is answered by the static-asset fallback with index.html, which reads as the app loading rather than as a route that is not there.",
            json_list(&declared),
            json_list(&worker_first)
        ));
    }

    // A root the client only requests under never reaches an artifact as its
    // own literal — a branch is folded into one constant here, so the client
    // carries `/geo/search` and nothing spelling `/geo` alone — and a guard
    // that looked for one would fail every build. What it can look for is the
    // roots and branches the client requests whole, which is what `bundle`
    // marks, in the artifact `bundle` names. Both lists are checked, because a
    // path that moves between the two looks exactly like a path that has gone
    // missing (2026-09-23).
    for (list, where_) in [("SENTINELS", "shell"), ("KERNEL_ROUTE_SENTINELS", "kernel")] {
        let found = listed("apps/web/script/check-bundle.ts", &format!(r"const {list} = \[([^\]]*)\]"), list)?;
        let mut want: Vec<String> = roots.iter().chain(branches.iter()).filter(|(_, r)| r.str_at("bundle") == Some(where_)).map(|(n, _)| path(n)).collect();
        want.sort();
        if found != want {
            return Err(format!(
                "apps/web/script/check-bundle.ts's {list} is {} where {JSON} marks {} as reaching the {where_}. A sentinel the build never emits fails every deploy; one the build dropped passes a guard that is checking nothing.",
                json_list(&found),
                json_list(&want)
            ));
        }
    }

    let mut all: Vec<(String, String)> = Vec::new();
    all.extend(roots.iter().map(|(n, _)| (n.to_string(), path(n))));
    all.extend(branches.iter().map(|(n, _)| (n.to_string(), path(n))));
    // The edge and the dev proxy match on a prefix, which is the root or
    // branch with its separator — never a bare root, which would also match a
    // sibling whose name merely starts with it.
    all.extend(roots.iter().map(|(n, _)| (format!("{}_PREFIX", n.strip_suffix("_ROOT").unwrap_or(n)), format!("{}/", path(n)))));

    let spec_doc = spec.lines_at("$doc").unwrap_or_default();
    let head = |logical: &str| {
        let mut h = vec![format!("// {logical}"), "//".to_string(), format!("// GENERATED by pxl-tool/bin/spec.rs from {JSON} — do not edit.")];
        h.extend(line_comments(&spec_doc, ""));
        h.push(String::new());
        h
    };

    let mut rs = head("pxl-spec/route.spec.rs");
    for (name, value) in &all {
        rs.extend(doc(name).unwrap_or_default().iter().map(|l| if l.is_empty() { "///".to_string() } else { format!("/// {l}") }));
        rs.extend([format!("pub const {name}: &str = {};", json_str(value)), String::new()]);
    }
    rs.extend([
        "/// Every root, as the edge claims them.".to_string(),
        format!(
            "pub const WORKER_FIRST: [&str; {}] = [{}];",
            worker_first.len(),
            worker_first.iter().map(|w| json_str(w)).collect::<Vec<_>>().join(", ")
        ),
        String::new(),
    ]);

    let mut ts = head("@pxl/web/config/route.spec.ts");
    for (name, value) in &all {
        ts.extend(ts_doc(&doc(name).unwrap_or_default(), ""));
        ts.extend([format!("export const {name} = {};", json_str(value)), String::new()]);
    }

    let mut worker = head("@pxl/web/cloudflare-worker/route.spec.ts");
    worker.extend(
        [
            "// The Worker is bundled for the Workers runtime rather than from src/, so it",
            "// cannot import the shell's copy — but it can import this one, which sits",
            "// beside it and is written from the same JSON. It relays by prefix, so the",
            "// prefixes are all it is given.",
            "",
        ]
        .map(String::from),
    );
    worker.extend(all.iter().filter(|(n, _)| n.ends_with("_PREFIX")).map(|(n, v)| format!("export const {n} = {};", json_str(v))));
    worker.push(String::new());

    Ok(vec![
        Output::new("crates/pxl/spec/src/route.spec.rs", rs.join("\n")),
        Output::new("apps/web/src/config/route.spec.ts", ts.join("\n")),
        Output::new("apps/web/cloudflare-worker/route.spec.ts", worker.join("\n")),
    ])
}
