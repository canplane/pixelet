// xpute-gen/spec/manifest.rs

//! This generates a program's manifest (spec/manifest.schema.json) as `MANIFEST`,
//! for both ends, into the files its `gen` names. The manifest declares what
//! xpute gives the core: the memory, the stack, the room of a frame's packet,
//! and the file and GPU devices. `MANIFEST` also carries where the front's parts lie, which the
//! runtime lays out and the core holds its header to. Nothing a protocol
//! carries is part of the manifest.

use std::path::Path;

use serde::Deserialize;

use super::{beside, file, generated, read_as, Output};

/// Every range of the front begins on a page. This must equal `PAGE_BYTES` of
/// xpute-core's mem/section.rs.
const PAGE_BYTES: u32 = 4096;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE_BYTES) * PAGE_BYTES
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mem {
    #[serde(rename = "$doc")]
    _doc: Option<Vec<String>>,
    bytes: Vec<u32>,
    region_bytes: u32,
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

/// The most bytes a frame's packet takes, either way.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pkt {
    #[serde(rename = "$doc")]
    _doc: Option<Vec<String>>,
    bytes: u32,
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
    pkt: Pkt,
    fs: Option<Fs>,
    stream: Option<Stream>,
    gpu: Option<Gpu>,
}

/// A manifest asks for the file device by having an `fs` entry, which carries
/// nothing else.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fs {
    #[serde(rename = "$doc", default)]
    _doc: Vec<String>,
}

/// The front's parts lie where xpute-core manifest.rs places them: the header's
/// page past the stack, and the stream on the page past it, which is 0 when the
/// manifest has none. The generated Rust asserts at
/// compile time that both computations agree.
impl Manifest {
    fn head_at(&self) -> u32 {
        page_up(self.stack.module_bytes)
    }

    fn stream_at(&self) -> u32 {
        match self.stream {
            Some(_) => self.head_at() + PAGE_BYTES,
            None => 0,
        }
    }

    /// This answers each place under the name both ends give it, with its
    /// offset.
    fn places(&self) -> [(&'static str, u32); 2] {
        [("head_at", self.head_at()), ("stream_at", self.stream_at())]
    }
}

/// Generates `json`'s manifest as Rust and TypeScript into the files its `gen`
/// names. `json` and the paths of the outputs are relative to `root`.
pub fn outputs(root: &Path, json: &str) -> Result<Vec<Output>, String> {
    let m: Manifest = read_as(&root.join(json).to_string_lossy())?;
    if m.mem.bytes.is_empty() {
        return Err(format!("{json}: mem.bytes names no size"));
    }
    let (ts, rs) = (beside(json, &m.gen.ts)?, beside(json, &m.gen.rs)?);
    Ok(vec![
        Output::new(&rs, manifest_rs(&m, json, &rs)),
        Output::new(&ts, manifest_ts(&m, json, &ts)),
    ])
}

fn manifest_rs(m: &Manifest, json: &str, shown: &str) -> String {
    let stream = match &m.stream {
        Some(s) => format!("Some(Stream {{ bytes: {} }})", s.bytes),
        None => "None".into(),
    };
    let sizes: Vec<String> = m.mem.bytes.iter().map(u32::to_string).collect();
    let mut lines = vec![
        format!("// {shown}"),
        "//".into(),
        generated(json),
        String::new(),
        "//! The manifest its JSON declares (xpute-core manifest.rs), which a build".into(),
        "//! script includes too as it links the module.".into(),
        String::new(),
        if m.stream.is_some() {
            "use xpute_core::manifest::{Manifest, Stream};".into()
        } else {
            "use xpute_core::manifest::Manifest;".into()
        },
        String::new(),
        "pub const MANIFEST: Manifest = Manifest {".into(),
        format!("    mem_bytes: &[{}],", sizes.join(", ")),
        format!("    region_bytes: {},", m.mem.region_bytes),
        format!("    min_heap_bytes: {},", m.mem.min_heap_bytes),
        format!("    stack_bytes: {},", m.stack.module_bytes),
        format!("    thread_stack_bytes: {},", m.stack.thread_bytes),
        format!("    pkt_bytes: {},", m.pkt.bytes),
        format!("    fs: {},", m.fs.is_some()),
        format!("    stream: {stream},"),
        "};".into(),
        String::new(),
        "// The runtime's MANIFEST places the front's parts at these same numbers.".into(),
    ];
    for (name, at) in m.places() {
        lines.push(format!("const _: () = assert!(MANIFEST.{name}() == {at});"));
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
    let sizes: Vec<String> = m.mem.bytes.iter().map(u32::to_string).collect();
    let mut lines = vec![
        format!("// {shown}"),
        "//".into(),
        generated(json),
        String::new(),
        "/** The manifest its JSON declares: the sizes a runtime may give, what a".into(),
        " * reservation is counted in, the most a frame's packet takes, where the".into(),
        " * front's header and the stream lie (0 without one), which".into(),
        " * the runtime lays before the first turn, whether it asks for the file".into(),
        " * device, and the GPU it asks for. */".into(),
        "export const MANIFEST = {".into(),
        format!("  mem_bytes: [{}],", sizes.join(", ")),
        format!("  region_bytes: {},", m.mem.region_bytes),
        format!("  pkt_bytes: {},", m.pkt.bytes),
    ];
    for (name, at) in m.places() {
        lines.push(format!("  {name}: {at},"));
    }
    lines.extend([
        format!("  fs: {},", m.fs.is_some()),
        format!("  gpu: {gpu},"),
        "} as const;".into(),
    ]);
    file(&lines)
}
