// pxl-render/examples/assembled.rs

//! Every shader by hand as the page takes it, and the world's asked one, written into the
//! directory named, a file each: for Tint (ts/tool/shader/shader.test.ts).

use pxl_render::frame::shader::record::{self, PARTS};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("the directory to write into");
    std::fs::create_dir_all(&dir).expect("the directory");
    let write = |name: String, text: &str| {
        std::fs::write(format!("{dir}/{name}.wgsl"), text).expect("a shader");
    };
    for (kind, parts) in PARTS.iter().enumerate() {
        write(
            format!("{kind}-{}", parts.last().expect("a shader's parts")),
            record::src(kind as u32),
        );
    }
    write("world_asked".to_string(), record::WORLD_ASKED);
}
