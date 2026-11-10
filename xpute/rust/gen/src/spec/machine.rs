// xpute-gen/spec/machine.rs

//! This generates a program's machine (spec/machine.schema.json) as `MACHINE`,
//! for both ends, into the files its `gen` names. The machine declares what
//! xpute gives the core: the memory, the stack, the rings, and the file and GPU
//! devices. `MACHINE` also carries where the front's parts lie, which the
//! runtime lays out and the core holds its header to. Nothing a protocol
//! carries is part of the machine.

use std::path::Path;

use serde::Deserialize;

use super::{beside, file, generated, read_as, Output};

/// Every range of the front begins on a page. This must equal `PAGE_BYTES` of
/// xpute-core's mem/section.rs.
const PAGE_BYTES: u32 = 4096;

/// These are the lengths in words of a ring's header and of a frame. They must
/// equal `HEADER_WORDS` of spec/ipc/ring.json and `WORDS` of
/// spec/ipc/frame.json.
const RING_HEADER_WORDS: u32 = 8;
const FRAME_WORDS: u32 = 4;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE_BYTES) * PAGE_BYTES
}

/// This answers the bytes a ring's header and frames take, as `ring_bytes` of
/// xpute-core's ipc/ring.rs computes them.
const fn ring_bytes(cap: u32) -> u32 {
    (RING_HEADER_WORDS + cap * FRAME_WORDS) * 4
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ring {
    #[serde(rename = "$doc")]
    _doc: Option<Vec<String>>,
    slot_bytes: u32,
    sq_cap: u32,
    cq_cap: u32,
    turn_posts: u32,
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
struct Machine {
    #[serde(rename = "$doc")]
    _doc: Vec<String>,
    gen: Gen,
    mem: Mem,
    stack: Stack,
    rings: Ring,
    fs: Option<Fs>,
    stream: Option<Stream>,
    gpu: Option<Gpu>,
}

/// A machine asks for the file device by having an `fs` entry, which carries
/// nothing else.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fs {
    #[serde(rename = "$doc", default)]
    _doc: Vec<String>,
}

/// The front's parts lie where xpute-core machine.rs places them: the header's
/// page past the stack, then the rings, their slots, and the stream past
/// them, which is 0 when the machine has none. The generated Rust asserts at
/// compile time that both computations agree.
impl Machine {
    fn head_at(&self) -> u32 {
        page_up(self.stack.module_bytes)
    }

    fn sq_at(&self) -> u32 {
        self.head_at() + PAGE_BYTES
    }

    fn cq_at(&self) -> u32 {
        self.sq_at() + page_up(ring_bytes(self.rings.sq_cap))
    }

    fn sq_slots_at(&self) -> u32 {
        self.cq_at() + page_up(ring_bytes(self.rings.cq_cap))
    }

    fn cq_slots_at(&self) -> u32 {
        self.sq_slots_at() + self.rings.sq_cap * self.rings.slot_bytes
    }

    fn stream_at(&self) -> u32 {
        match self.stream {
            Some(_) => self.cq_slots_at() + self.rings.cq_cap * self.rings.slot_bytes,
            None => 0,
        }
    }

    /// This answers each place under the name both ends give it, with its
    /// offset.
    fn places(&self) -> [(&'static str, u32); 6] {
        [
            ("head_at", self.head_at()),
            ("sq_at", self.sq_at()),
            ("cq_at", self.cq_at()),
            ("sq_slots_at", self.sq_slots_at()),
            ("cq_slots_at", self.cq_slots_at()),
            ("stream_at", self.stream_at()),
        ]
    }
}

/// Generates `json`'s machine as Rust and TypeScript into the files its `gen`
/// names. `json` and the paths of the outputs are relative to `root`.
pub fn outputs(root: &Path, json: &str) -> Result<Vec<Output>, String> {
    let m: Machine = read_as(&root.join(json).to_string_lossy())?;
    if m.mem.bytes.is_empty() {
        return Err(format!("{json}: mem.bytes names no size"));
    }
    let (ts, rs) = (beside(json, &m.gen.ts)?, beside(json, &m.gen.rs)?);
    Ok(vec![
        Output::new(&rs, machine_rs(&m, json, &rs)),
        Output::new(&ts, machine_ts(&m, json, &ts)),
    ])
}

fn machine_rs(m: &Machine, json: &str, shown: &str) -> String {
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
        "//! The machine its JSON declares (xpute-core machine.rs), which a build".into(),
        "//! script includes too as it links the module.".into(),
        String::new(),
        if m.stream.is_some() {
            "use xpute_core::machine::{Machine, Stream};".into()
        } else {
            "use xpute_core::machine::Machine;".into()
        },
        String::new(),
        "pub const MACHINE: Machine = Machine {".into(),
        format!("    mem_bytes: &[{}],", sizes.join(", ")),
        format!("    region_bytes: {},", m.mem.region_bytes),
        format!("    min_heap_bytes: {},", m.mem.min_heap_bytes),
        format!("    stack_bytes: {},", m.stack.module_bytes),
        format!("    thread_stack_bytes: {},", m.stack.thread_bytes),
        format!("    slot_bytes: {},", m.rings.slot_bytes),
        format!("    sq_cap: {},", m.rings.sq_cap),
        format!("    cq_cap: {},", m.rings.cq_cap),
        format!("    turn_posts: {},", m.rings.turn_posts),
        format!("    fs: {},", m.fs.is_some()),
        format!("    stream: {stream},"),
        "};".into(),
        String::new(),
        "// The runtime's MACHINE places the front's parts at these same numbers.".into(),
    ];
    for (name, at) in m.places() {
        lines.push(format!("const _: () = assert!(MACHINE.{name}() == {at});"));
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

fn machine_ts(m: &Machine, json: &str, shown: &str) -> String {
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
        "/** The machine its JSON declares: the sizes a runtime may give, what a".into(),
        " * reservation is counted in, the rings' sizes, where the front's header,".into(),
        " * the rings, their slots and the stream lie (0 without one), which".into(),
        " * the runtime lays before the first turn, whether it asks for the file".into(),
        " * device, and the GPU it asks for. */".into(),
        "export const MACHINE = {".into(),
        format!("  mem_bytes: [{}],", sizes.join(", ")),
        format!("  region_bytes: {},", m.mem.region_bytes),
        format!("  sq_cap: {},", m.rings.sq_cap),
        format!("  cq_cap: {},", m.rings.cq_cap),
        format!("  slot_bytes: {},", m.rings.slot_bytes),
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
