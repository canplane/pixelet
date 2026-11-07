// pxl-render/examples/assembled.rs

//! Every shader by hand as the page takes it, and the world's asked one, written into the
//! directory named, a file each; and every pipeline the renderer makes over them, render and
//! compute, its entry points as the renderer names them (pipelines.json): for Tint
//! (ts/tool/shader/shader.test.ts) and the device's own compiler (ts/tool/shader/probe.html).

use pxl_render::frame::shader::record::{
    self, compute, COMPUTES, FRAGMENT_ENTRY, HEAD, NO_DEPTH, PARTS, PER_INSTANCE, PIPELINES, STRIP,
    VERTEX_ENTRY,
};
use pxl_render::gpu::api::vertex_format;
use xpute_gpu::enums::VERTEX_FORMAT;

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
    for id in 0..PIPELINES {
        let words = record::pipeline(id);
        let mut at = HEAD as usize;
        let mut next = || {
            at += 1;
            words[at - 1]
        };
        let buffers: Vec<String> = (0..next())
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
                        let format = VERTEX_FORMAT[vertex_format(next()) as usize];
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
            VERTEX_ENTRY[words[HEAD as usize - 1] as usize],
            buffers.join(","),
            words[5] != NO_DEPTH
        ));
    }
    for id in 0..COMPUTES {
        let (shader, entry) = compute(id);
        pipelines.push(format!(
            r#"{{"module":"{}","compute":"{entry}"}}"#,
            names[shader as usize]
        ));
    }
    // Pipelines alike in all the checks make of them are made once.
    let mut made: Vec<String> = Vec::new();
    for p in pipelines {
        if !made.contains(&p) {
            made.push(p);
        }
    }
    write("pipelines.json", &format!("[\n{}\n]\n", made.join(",\n")));
}
