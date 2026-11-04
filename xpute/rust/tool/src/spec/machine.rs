// xpute-tool/spec/machine.rs

//! A program's machine (spec/machine.schema.json): the sockets xpute gives
//! its guest — the memory, the stack, the rings and the GPU socket — written
//! as `MACHINE` for both ends where its `gen` says: the front's places among
//! it, which the host lays and the guest holds its header to. Nothing a
//! protocol carries is in it.

use std::path::Path;

use serde::Deserialize;

use super::{beside, file, generated, read_as, Output};

/// The page every range of the front begins on (xpute-guest mem/section.rs
/// `PAGE_BYTES`).
const PAGE_BYTES: u32 = 4096;

/// A ring's header and a frame, in words (spec/ipc/ring.json `HEADER_WORDS`,
/// spec/ipc/frame.json `WORDS`).
const RING_HEADER_WORDS: u32 = 8;
const FRAME_WORDS: u32 = 4;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE_BYTES) * PAGE_BYTES
}

/// A ring's words (xpute-guest ipc/ring.rs `ring_bytes`).
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
struct Rings {
    #[serde(rename = "$doc")]
    _doc: Option<Vec<String>>,
    slot_bytes: u32,
    sq_cap: u32,
    cq_cap: u32,
    turn_posts: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Gpu {
    #[serde(rename = "$doc")]
    _doc: Option<Vec<String>>,
    bytes: u32,
    power: String,
    features: Features,
}

/// WebGPU's feature names: those the device must have, and those it takes
/// where the adapter has them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Features {
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
    rings: Rings,
    gpu: Option<Gpu>,
}

/// The front's places, as xpute-guest machine.rs computes them: the
/// header's page past the stack, the rings, their slots, and the GPU stream
/// past them, 0 without the socket.
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

    fn gpu_at(&self) -> u32 {
        match self.gpu {
            Some(_) => self.cq_slots_at() + self.rings.cq_cap * self.rings.slot_bytes,
            None => 0,
        }
    }

    /// Each place by its name on both ends, and its number.
    fn places(&self) -> [(&'static str, u32); 6] {
        [
            ("head_at", self.head_at()),
            ("sq_at", self.sq_at()),
            ("cq_at", self.cq_at()),
            ("sq_slots_at", self.sq_slots_at()),
            ("cq_slots_at", self.cq_slots_at()),
            ("gpu_at", self.gpu_at()),
        ]
    }
}

/// `json`'s machine, from `root`, as Rust and TypeScript where its `gen`
/// says, each path from `root` as `json` is.
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
    let gpu = match &m.gpu {
        Some(g) => format!(
            "Some(Gpu {{ bytes: {}, power: \"{}\", required: &[{}], optional: &[{}] }})",
            g.bytes,
            g.power,
            quoted(&g.features.required),
            quoted(&g.features.optional)
        ),
        None => "None".into(),
    };
    let sizes: Vec<String> = m.mem.bytes.iter().map(u32::to_string).collect();
    let mut lines = vec![
        format!("// {shown}"),
        "//".into(),
        generated(json),
        String::new(),
        "//! The machine its JSON declares (xpute-guest machine.rs), which a build".into(),
        "//! script includes too as it links the module.".into(),
        String::new(),
        if m.gpu.is_some() {
            "use xpute_guest::machine::{Gpu, Machine};".into()
        } else {
            "use xpute_guest::machine::Machine;".into()
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
        format!("    gpu: {gpu},"),
        "};".into(),
        String::new(),
        "// The host's MACHINE places the front's parts at these same numbers.".into(),
    ];
    for (name, at) in m.places() {
        lines.push(format!("const _: () = assert!(MACHINE.{name}() == {at});"));
    }
    file(&lines)
}

fn machine_ts(m: &Machine, json: &str, shown: &str) -> String {
    let gpu = match &m.gpu {
        Some(g) => format!(
            "{{ bytes: {}, power: \"{}\", features: {{ required: [{}], optional: [{}] }} }}",
            g.bytes,
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
        "/** The machine its JSON declares: the sizes a host may give, what a".into(),
        " * reservation is counted in, the rings' sizes, where the front's header,".into(),
        " * the rings, their slots and the GPU stream lie (0 without the socket),".into(),
        " * which the host lays before the first turn, and the GPU socket where it".into(),
        " * is taken. */".into(),
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
    lines.extend([format!("  gpu: {gpu},"), "} as const;".into()]);
    file(&lines)
}
