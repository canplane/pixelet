// pxl-tool/spec/route.rs

//! The address space from one JSON, written into the client's driver, the
//! Worker and the servers, and its part of wrangler's and Caddy's
//! configurations: the roots the edge claims, the site it answers on and the
//! origin it relays to.

use regex::Regex;

use super::{json_str, line_comments, read, ts_doc, Node, Output};
use crate::root;

const JSON: &str = "spec/spec/route.json";

pub fn outputs() -> Result<Vec<Output>, String> {
    let spec = read(JSON)?;
    let roots: Vec<(&str, &Node)> = spec
        .get("roots")
        .map(|r| r.entries().iter().map(|(k, v)| (k.as_str(), v)).collect())
        .unwrap_or_default();
    let branches: Vec<(&str, &Node)> = spec
        .get("branches")
        .map(|b| b.entries().iter().map(|(k, v)| (k.as_str(), v)).collect())
        .unwrap_or_default();

    let mut paths: Vec<(String, String)> = roots
        .iter()
        .map(|(n, r)| {
            (
                n.to_string(),
                r.str_at("path").unwrap_or_default().to_string(),
            )
        })
        .collect();
    for (name, b) in &branches {
        let under = b.str_at("under").unwrap_or_default();
        let base = paths
            .iter()
            .find(|(n, _)| n == under)
            .map(|(_, p)| p.clone())
            .ok_or(format!(
                "{JSON}: {name} is under {under}, which is not a root"
            ))?;
        paths.push((
            name.to_string(),
            format!("{base}/{}", b.str_at("name").unwrap_or_default()),
        ));
    }
    let path = |name: &str| {
        paths
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, p)| p.clone())
            .unwrap_or_default()
    };
    let doc = |name: &str| {
        roots
            .iter()
            .chain(branches.iter())
            .find(|(n, _)| *n == name)
            .and_then(|(_, r)| r.lines_at("$doc"))
    };

    let worker_first: Vec<String> = roots
        .iter()
        .map(|(n, _)| format!("{}/*", path(n)))
        .collect();

    // Every root, so none is answered by the static-asset fallback with
    // index.html, which reads as the app loading rather than as no route.
    let wrangler_path = "ts/server/edge/wrangler.jsonc";
    let wrangler = std::fs::read_to_string(root().join(wrangler_path))
        .map_err(|e| format!("{wrangler_path}: {e}"))?;
    let claim = Regex::new(r#""run_worker_first"\s*:\s*\[[^\]]*\]"#).map_err(|e| e.to_string())?;
    if !claim.is_match(&wrangler) {
        return Err(format!("{wrangler_path}: no run_worker_first to write"));
    }
    let list = serde_json::to_string(&worker_first)
        .unwrap_or_default()
        .replace(',', ", ");
    let wrangler = claim
        .replace(&wrangler, format!(r#""run_worker_first": {list}"#).as_str())
        .into_owned();

    // The site the edge answers on and the origin it relays to, each an
    // https URL of a host alone.
    let origin = |name: &str| -> Result<(String, String, Vec<String>), String> {
        let o = spec
            .get("origins")
            .and_then(|o| o.get(name))
            .ok_or(format!("{JSON}: no origins.{name}"))?;
        let url = o.str_at("url").unwrap_or_default();
        let host = url
            .strip_prefix("https://")
            .filter(|h| {
                !h.is_empty()
                    && h.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-'
                    })
            })
            .ok_or(format!(
                "{JSON}: origins.{name} {url:?} is no https URL of a host alone"
            ))?;
        Ok((
            url.to_string(),
            host.to_string(),
            o.lines_at("$doc").unwrap_or_default(),
        ))
    };
    let (site, site_host, site_doc) = origin("SITE")?;
    let (relay, relay_host, relay_doc) = origin("ORIGIN")?;
    let write = |text: &str, path: &str, pattern: &str, with: &str| -> Result<String, String> {
        let re = Regex::new(pattern).map_err(|e| e.to_string())?;
        if !re.is_match(text) {
            return Err(format!("{path}: nothing matches {pattern}"));
        }
        Ok(re.replace(text, with).into_owned())
    };
    let wrangler = write(
        &wrangler,
        wrangler_path,
        r#""pattern"\s*:\s*"[^"]*""#,
        &format!(r#""pattern": "{site_host}""#),
    )?;
    let wrangler = write(
        &wrangler,
        wrangler_path,
        r#""GEO_ORIGIN"\s*:\s*"[^"]*""#,
        &format!(r#""GEO_ORIGIN": "{relay}""#),
    )?;
    let wrangler = write(
        &wrangler,
        wrangler_path,
        r#""API_ORIGIN"\s*:\s*"[^"]*""#,
        &format!(r#""API_ORIGIN": "{relay}""#),
    )?;
    // The origin's one site block, its address the host.
    let caddy_path = "infra/prod/Caddyfile";
    let caddy = std::fs::read_to_string(root().join(caddy_path))
        .map_err(|e| format!("{caddy_path}: {e}"))?;
    let caddy = write(
        &caddy,
        caddy_path,
        r"(?m)^[a-z0-9.-]+ \{$",
        &format!("{relay_host} {{"),
    )?;

    let mut all: Vec<(String, String)> = Vec::new();
    all.extend(roots.iter().map(|(n, _)| (n.to_string(), path(n))));
    all.extend(branches.iter().map(|(n, _)| (n.to_string(), path(n))));
    // A prefix keeps its separator; a bare root would match a sibling starting with it.
    all.extend(roots.iter().map(|(n, _)| {
        (
            format!("{}_PREFIX", n.strip_suffix("_ROOT").unwrap_or(n)),
            format!("{}/", path(n)),
        )
    }));

    let spec_doc = spec.lines_at("$doc").unwrap_or_default();
    let head = |logical: &str| {
        let mut h = vec![
            format!("// {logical}"),
            "//".to_string(),
            format!("// GENERATED from {JSON}"),
        ];
        h.extend(line_comments(&spec_doc, ""));
        h.push(String::new());
        h
    };

    let mut rs = head("pxl-spec/route.spec.rs");
    for (name, val) in &all {
        rs.extend(doc(name).unwrap_or_default().iter().map(|l| {
            if l.is_empty() {
                "///".to_string()
            } else {
                format!("/// {l}")
            }
        }));
        rs.extend([
            format!("pub const {name}: &str = {};", json_str(val)),
            String::new(),
        ]);
    }
    rs.extend([
        "/// Every root, as the edge claims them.".to_string(),
        format!(
            "pub const WORKER_FIRST: [&str; {}] = [{}];",
            worker_first.len(),
            worker_first
                .iter()
                .map(|w| json_str(w))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        String::new(),
    ]);

    let mut ts = head("@pxl/client/driver/namespace/route.spec.ts");
    for (name, val) in &all {
        ts.extend(ts_doc(&doc(name).unwrap_or_default(), ""));
        ts.extend([
            format!("export const {name} = {};", json_str(val)),
            String::new(),
        ]);
    }

    for (name, url, doc) in [
        ("SITE_URL", &site, &site_doc),
        ("ORIGIN_URL", &relay, &relay_doc),
    ] {
        ts.extend(ts_doc(doc, ""));
        ts.extend([
            format!("export const {name} = {};", json_str(url)),
            String::new(),
        ]);
    }

    let mut worker = head("@pxl/edge/route.spec.ts");
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
    worker.extend(
        all.iter()
            .filter(|(n, _)| n.ends_with("_PREFIX"))
            .map(|(n, v)| format!("export const {n} = {};", json_str(v))),
    );
    worker.push(String::new());
    Ok(vec![
        Output::new("rust/spec/src/route.spec.rs", rs.join("\n")),
        Output::new(
            "ts/client/src/driver/namespace/route.spec.ts",
            ts.join("\n"),
        ),
        Output::new("ts/server/edge/route.spec.ts", worker.join("\n")),
        Output::new(wrangler_path, wrangler),
        Output::new(caddy_path, caddy),
    ])
}
