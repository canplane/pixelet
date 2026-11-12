// xpute-gen/spec/manifest.rs

//! This generates a program's manifest (spec/manifest.schema.json) for both
//! ends, into the files its `gen` names: what the core asks of xpute, its
//! memory, its fixed ranges and the heap it counts on, its stacks, its stream
//! and its GPU. Nothing a protocol
//! carries is part of the manifest.

use std::path::Path;

use serde::Deserialize;

use super::{beside, file, generated, read_as, Output};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mem {
    #[serde(rename = "$doc")]
    _doc: Option<Vec<String>>,
    bytes: u32,
    fixed_bytes: u32,
    min_heap_bytes: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stack {
    #[serde(rename = "$doc")]
    _doc: Option<Vec<String>>,
    module_bytes: u32,
    thread_bytes: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stream {
    bytes: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Gpu {
    #[serde(rename = "$doc")]
    _doc: Option<Vec<String>>,
    power: String,
    features: FeatureSet,
}

/// These are WebGPU's feature names: those the device must have, and those it
/// takes where the adapter has them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureSet {
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    optional: Vec<String>,
}

fn quoted(names: &[String]) -> String {
    names
        .iter()
        .map(|n| format!("\"{n}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Gen {
    ts: String,
    rs: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    #[serde(rename = "$doc")]
    _doc: Vec<String>,
    gen: Gen,
    mem: Mem,
    stack: Stack,
    stream: Option<Stream>,
    gpu: Option<Gpu>,
}

/// Generates `json`'s manifest as Rust and TypeScript into the files its `gen`
/// names. `json` and the paths of the outputs are relative to `root`.
pub fn outputs(root: &Path, json: &str) -> Result<Vec<Output>, String> {
    let m: Manifest = read_as(&root.join(json).to_string_lossy())?;
    let (ts, rs) = (beside(json, &m.gen.ts)?, beside(json, &m.gen.rs)?);
    Ok(vec![
        Output::new(&rs, manifest_rs(&m, json, &rs)),
        Output::new(&ts, manifest_ts(&m, json, &ts)),
    ])
}

fn manifest_rs(m: &Manifest, json: &str, shown: &str) -> String {
    let mut lines = vec![
        format!("// {shown}"),
        "//".into(),
        generated(json),
        String::new(),
        "//! The manifest its JSON declares, which a build script includes too as it".into(),
        "//! links the module.".into(),
        String::new(),
        "/// The core's memory, whole before its first entry and never grown.".into(),
        format!("pub const MEM_BYTES: u32 = {};", m.mem.bytes),
        "/// The program's fixed ranges, past the module's data.".into(),
        format!("pub const FIXED_BYTES: u32 = {};", m.mem.fixed_bytes),
        "/// The heap the core counts on, past the fixed ranges.".into(),
        format!("pub const MIN_HEAP_BYTES: u32 = {};", m.mem.min_heap_bytes),
        "/// A module's stack, first in its memory.".into(),
        format!("pub const STACK_BYTES: u32 = {};", m.stack.module_bytes),
        "/// A native core's thread's stack.".into(),
        format!("pub const THREAD_STACK_BYTES: u32 = {};", m.stack.thread_bytes),
    ];
    if let Some(s) = &m.stream {
        lines.extend([
            "/// The stream's room.".into(),
            format!("pub const STREAM_BYTES: u32 = {};", s.bytes),
        ]);
    }
    if let Some(g) = &m.gpu {
        lines.extend([
            String::new(),
            "// What the core asks of its GPU: the power preference, and the features".into(),
            "// the device must have and those it takes where the adapter has them.".into(),
            format!(
                "pub const GPU_ASK: (&str, &[&str], &[&str]) = (\"{}\", &[{}], &[{}]);",
                g.power,
                quoted(&g.features.required),
                quoted(&g.features.optional)
            ),
        ]);
    }
    file(&lines)
}

fn manifest_ts(m: &Manifest, json: &str, shown: &str) -> String {
    let gpu = match &m.gpu {
        Some(g) => format!(
            "{{ power: \"{}\", features: {{ required: [{}], optional: [{}] }} }}",
            g.power,
            quoted(&g.features.required),
            quoted(&g.features.optional)
        ),
        None => "null".into(),
    };
    let lines = vec![
        format!("// {shown}"),
        "//".into(),
        generated(json),
        String::new(),
        "/** The manifest its JSON declares: the core's memory, whole before its".into(),
        " * first entry and never grown, and the GPU the core asks for. */".into(),
        "export const MANIFEST = {".into(),
        format!("  mem_bytes: {},", m.mem.bytes),
        format!("  gpu: {gpu},"),
        "} as const;".into(),
    ];
    file(&lines)
}
