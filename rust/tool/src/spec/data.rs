// pxl-tool/spec/data.rs

//! pixelet's data specs, written by xpute's generator (xpute-tool
//! spec/data.rs): JSON of named values as Rust constants, and as TypeScript
//! `as const` where a spec lists a TypeScript file. The Rust carries no
//! names: a list of them is its count, and an enum where the spec asks for
//! one, and a table's keys are not listed; the TypeScript keeps them for what
//! a person reads. A `file` is a slot of the catalog (catalog.rs).

use xpute_tool::spec::data::{outputs_at, DataSpec, Enums, Layout, Slots};

use super::catalog::{runs, FILE};
use super::ingest::files;
use super::Output;
use crate::root;

const DATA_SPECS: [DataSpec; 20] = [
    DataSpec {
        json: "spec/spec/space/earth.json",
        uses: &[],
        ts: None,
        ts_names: None,
        rs: Some("rust/spec/src/space/coord/earth.spec.rs"),
        module: "spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/spec/space/geoid.json",
        uses: &[],
        ts: None,
        ts_names: None,
        rs: Some("rust/spec/src/space/coord/geoid.spec.rs"),
        module: "spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/spec/object/ext.json",
        uses: &[],
        ts: None,
        ts_names: None,
        rs: Some("rust/spec/src/object/ext.spec.rs"),
        module: "ext",
        rs_also: &[],
        enums: Enums::All,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/spec/asset/marker.json",
        uses: &[],
        ts: Some("ts/client/src/shell/catalog/asset/marker.spec.ts"),
        ts_names: Some(&["PIN_LABEL", "MARKER_TEXT_CHARS"]),
        rs: Some("rust/spec/src/asset/marker.spec.rs"),
        module: "marker",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/spec/asset/id.json",
        uses: &[],
        ts: Some("ts/client/src/shell/catalog/asset/id.spec.ts"),
        ts_names: None,
        rs: Some("rust/spec/src/asset/id.spec.rs"),
        module: "id",
        rs_also: &[],
        enums: Enums::All,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/spec/division.json",
        uses: &[],
        ts: Some("ts/client/src/shell/division/division.spec.ts"),
        ts_names: None,
        rs: Some("rust/spec/src/division.spec.rs"),
        module: "spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/spec/search.json",
        uses: &[],
        ts: Some("ts/client/src/shell/search/search.spec.ts"),
        ts_names: None,
        rs: Some("rust/spec/src/search.spec.rs"),
        module: "spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/spec/asset/character.json",
        uses: &[],
        ts: Some("ts/client/src/shell/catalog/asset/character.spec.ts"),
        ts_names: Some(&["groups", "GENDER"]),
        rs: Some("rust/spec/src/asset/character.spec.rs"),
        module: "character",
        rs_also: &[],
        enums: Enums::All,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/spec/asset/sky.json",
        uses: &[],
        ts: None,
        ts_names: None,
        rs: Some("rust/spec/src/asset/sky.spec.rs"),
        module: "sky",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/kernel/domain/scene/gizmo/gizmo.json",
        uses: &[],
        ts: Some("ts/client/src/abi/gizmo.spec.ts"),
        ts_names: None,
        rs: Some("rust/client/kernel/src/domain/scene/gizmo/gizmo.spec.rs"),
        module: "gizmo_spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/render/gpu/api.json",
        uses: &[],
        ts: Some("ts/client/src/abi/gpu.spec.ts"),
        ts_names: Some(&["surface_role", "stream"]),
        rs: Some("rust/client/render/src/gpu/api.spec.rs"),
        module: "api_spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/kernel/sys/io/file.json",
        uses: &[],
        ts: Some("ts/client/src/abi/file.spec.ts"),
        ts_names: None,
        rs: Some("rust/client/kernel/src/sys/io/file.spec.rs"),
        module: "file_spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/kernel/sys/io/runtime.json",
        uses: &[],
        ts: Some("ts/client/src/abi/runtime.spec.ts"),
        ts_names: None,
        rs: Some("rust/client/kernel/src/sys/io/runtime.spec.rs"),
        module: "runtime_spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/kernel/sys/sched/pacing.json",
        uses: &[],
        ts: Some("ts/client/src/abi/pacing.spec.ts"),
        ts_names: None,
        rs: Some("rust/client/kernel/src/sys/sched/pacing.spec.rs"),
        module: "pacing_spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/kernel/sys/io/demand.json",
        uses: &[],
        ts: Some("ts/client/src/abi/demand.spec.ts"),
        ts_names: None,
        rs: Some("rust/client/kernel/src/sys/io/demand.spec.rs"),
        module: "demand_spec",
        rs_also: &[],
        enums: Enums::Named(&["STREAM_NAMES", "OUTCOME_NAMES", "TASK_PHASE_NAMES"]),
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/kernel/sys/sched/phase.json",
        uses: &[],
        ts: Some("ts/client/src/abi/phase.spec.ts"),
        ts_names: None,
        rs: Some("rust/client/kernel/src/sys/sched/phase.spec.rs"),
        module: "phase_spec",
        rs_also: &[],
        enums: Enums::Named(&["PHASES"]),
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/kernel/turn/step.json",
        uses: &[],
        ts: Some("ts/client/src/abi/step.spec.ts"),
        ts_names: None,
        rs: Some("rust/client/kernel/src/turn/step.spec.rs"),
        module: "spec",
        rs_also: &[],
        enums: Enums::Named(&["STEP_NAMES", "EFFECT_NAMES"]),
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/render/gpu/span.json",
        uses: &[],
        ts: Some("ts/client/src/abi/span.spec.ts"),
        ts_names: None,
        rs: Some("rust/client/render/src/gpu/span.spec.rs"),
        module: "span_spec",
        rs_also: &[],
        enums: Enums::Named(&["GPU_SPAN_NAMES", "FRAME_KINDS", "LIGHT_MOVED"]),
        cpp: None,
        mk: None,
    },
    DataSpec {
        json: "spec/client/kernel/perf/perf.json",
        uses: &[
            "spec/client/kernel/sys/sched/phase.json",
            "spec/client/kernel/sys/io/demand.json",
            "spec/client/kernel/turn/step.json",
            "spec/client/render/gpu/span.json",
        ],
        ts: Some("ts/client/src/abi/perf.spec.ts"),
        ts_names: None,
        rs: Some("rust/client/kernel/src/perf/perf.spec.rs"),
        module: "spec",
        rs_also: &[],
        enums: Enums::Named(&["WAKES", "HIT_KINDS"]),
        cpp: None,
        mk: None,
    },
    // Read, not written: the profiler uses its names; xpute generates it.
    DataSpec {
        json: "xpute/spec/gpu/api.json",
        uses: &[],
        ts: None,
        ts_names: None,
        rs: None,
        module: "spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    },
];

/// A TypeScript file's directory, and the name it is imported by.
const TS_LAYOUT: [(&str, &str); 1] = [("ts/client/src/", "@pxl/client/")];

/// A Rust file's `src/`, and its crate's name.
const RS_LAYOUT: [(&str, &str); 3] = [
    ("rust/client/kernel/src/", "pxl-kernel"),
    ("rust/client/render/src/", "pxl-render"),
    ("rust/spec/src/", "pxl-spec"),
];

/// What a file reference is written as in Rust: a type whose one field is
/// the reference's slot.
const SLOT: &str = "crate::asset::catalog::Slot";

pub fn outputs() -> Result<Vec<Output>, String> {
    let files: Vec<String> = files()?.into_iter().map(|f| f.path).collect();
    let runs = runs()?;
    let slots = [Slots {
        ty: FILE,
        rs: SLOT,
        names: &files,
        runs: &runs,
    }];
    let layout = Layout {
        ts: &TS_LAYOUT,
        rs: &RS_LAYOUT,
        rs_names: false,
        slots: &slots,
    };
    outputs_at(&root(), &DATA_SPECS, &layout)
}
