// pxl-render/examples/assembled.rs

//! Writes into the named directory every shader as the app receives it, one file each, and
//! the world's asked module. It also writes pipelines.json: every pipeline the renderer makes
//! over those shaders, render and compute, with its entry points named as the renderer names
//! them. Tint compiles them in ts/tool/shader/shader.test.ts, and a device's own browser
//! compiles them in ts/tool/shader/probe.html.

use pxl_render::frame::shader::record::{
    self, COMPUTE_CNT, FRAGMENT_ENTRY, HDR, NO_DEPTH, PARTS, PER_INSTANCE, PIPELINE_CNT, STRIP,
    VERTEX_ENTRY, compute,
};
use pxl_render::gpu::api::vertex_format;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("the directory to write into");
    std::fs::create_dir_all(&dir).expect("the directory");
    let write = |name: &str, text: &str| {
        std::fs::write(format!("{dir}/{name}"), text).expect("a shader");
    };
    let names: Vec<String> = PARTS
        .iter()
        .enumerate()
        .map(|(kind, parts)| format!("{kind}-{}", parts.last().expect("a shader's parts")))
        .collect();
    for (kind, name) in names.iter().enumerate() {
        write(&format!("{name}.wgsl"), record::src(kind as u32));
    }
    write("world_asked.wgsl", record::WORLD_ASKED);
    let mut pipelines: Vec<String> = Vec::new();
    for id in 0..PIPELINE_CNT {
        let words = record::pipeline(id);
        let mut at = HDR as usize;
        let mut next = || {
            at += 1;
            words[at - 1]
        };
        let bufs: Vec<String> = (0..next())
            .map(|_| {
                let stride = next();
                let step = if next() == PER_INSTANCE {
                    "instance"
                } else {
                    "vertex"
                };
                let attributes: Vec<String> = (0..next())
                    .map(|_| {
                        let (location, off) = (next(), next());
                        let format = vertex_format(next()).name();
                        format!(
                            r#"{{"shaderLocation":{location},"offset":{off},"format":"{format}"}}"#
                        )
                    })
                    .collect();
                format!(
                    r#"{{"arrayStride":{stride},"stepMode":"{step}","attributes":[{}]}}"#,
                    attributes.join(",")
                )
            })
            .collect();
        let fragment = if words[1] != 0 {
            format!(r#","fragment":"{FRAGMENT_ENTRY}""#)
        } else {
            String::new()
        };
        let topology = if words[2] == STRIP {
            "triangle-strip"
        } else {
            "triangle-list"
        };
        pipelines.push(format!(
            r#"{{"module":"{}","vertex":"{}"{fragment},"buffers":[{}],"topology":"{topology}","depth":{}}}"#,
            names[words[0] as usize],
            VERTEX_ENTRY[words[HDR as usize - 1] as usize],
            bufs.join(","),
            words[5] != NO_DEPTH
        ));
    }
    for id in 0..COMPUTE_CNT {
        let (shader, entry, _) = compute(id);
        pipelines.push(format!(
            r#"{{"module":"{}","compute":"{entry}"}}"#,
            names[shader as usize]
        ));
    }
    // Two pipelines that no check can tell apart are written once.
    let mut made: Vec<String> = Vec::new();
    for p in pipelines {
        if !made.contains(&p) {
            made.push(p);
        }
    }
    write("pipelines.json", &format!("[\n{}\n]\n", made.join(",\n")));
}
