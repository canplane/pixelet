// xpute-tool/spec/data.rs

//! xpute's own data specs, and where each one is written.

use xpute_gen::spec::data::{outputs_at, DataSpec, EnumScope, Layout};
use xpute_gen::spec::Output;

use crate::root;

const DATA_SPECS: [DataSpec; 3] = [
    DataSpec {
        json: "spec/io/stream.json",
        ts: Some("ts/runtime/src/io/stream.spec.ts"),
        rs: Some("rust/core/src/io/stream.spec.rs"),
        cpp: Some(("cpp/xpute/core/io/stream.spec.hpp", "xpute::stream")),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/gpu/api.json",
        uses: &["spec/gpu/enum.json"],
        ts: Some("ts/gpu/src/api.spec.ts"),
        rs: Some("rust/gpu/src/api.spec.rs"),
        enum_scope: EnumScope::Named(&["BINDING_KIND", "RESOURCE_KIND"]),
        cpp: Some(("cpp/xpute/gpu/api.spec.hpp", "xpute::gpu")),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/gpu/enum.json",
        ts: Some("ts/gpu/src/enum.spec.ts"),
        rs: Some("rust/gpu/src/enum.spec.rs"),
        enum_scope: EnumScope::Named(&[
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
        cpp: Some(("cpp/xpute/gpu/enum.spec.hpp", "xpute::gpu")),
        ..DataSpec::DEFAULT
    },
];

const LAYOUT: Layout = Layout {
    ts: &[
        ("ts/runtime/src/", "@xpute/runtime/"),
        ("ts/gpu/src/", "@xpute/gpu/"),
    ],
    rs: &[
        ("rust/core/src/", "xpute-core"),
        ("rust/gpu/src/", "xpute-gpu"),
    ],
    rs_names: true,
    slots: &[],
};

pub fn outputs() -> Result<Vec<Output>, String> {
    outputs_at(&root(), &DATA_SPECS, &LAYOUT)
}
