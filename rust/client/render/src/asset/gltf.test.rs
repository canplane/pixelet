// pxl-render/asset/gltf.test.rs

use std::fs;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use super::*;

fn parse(bytes: &[u8]) -> Result<Gltf, MarshalError> {
    let mut parsing = pin!(parse_gltf(bytes, &[], || core::future::ready(())));
    match parsing
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(parsed) => parsed,
        Poll::Pending => unreachable!("a pause that is ready never parks"),
    }
}

fn refused(bytes: &[u8]) -> Errno {
    parse(bytes).expect_err("refused").errnum
}

fn glb(json: &str, bin: &[u8]) -> Vec<u8> {
    let mut text = json.as_bytes().to_vec();
    text.resize(text.len().next_multiple_of(4), b' ');
    let mut data = bin.to_vec();
    data.resize(data.len().next_multiple_of(4), 0);
    let total = 12 + 8 + text.len() + 8 + data.len();
    let mut out = Vec::new();
    for word in [GLB_MAGIC, 2, total as u32, text.len() as u32, CHUNK_JSON] {
        out.extend(word.to_le_bytes());
    }
    out.extend(text);
    for word in [data.len() as u32, CHUNK_BIN] {
        out.extend(word.to_le_bytes());
    }
    out.extend(data);
    out
}

/// Three positions, then three u16 indices from byte 36.
fn triangle_bin(index: [u16; 3]) -> Vec<u8> {
    let mut bin = Vec::new();
    for v in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        bin.extend(v.to_le_bytes());
    }
    for i in index {
        bin.extend(i.to_le_bytes());
    }
    bin
}

const TRIANGLE: &str = r#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
"nodes":[{"mesh":0}],
"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}],
"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},
{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}],
"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":6}],
"buffers":[{"byteLength":44}]}"#;

/// The triangle with each `from` written as its `to`.
fn edited(edits: &[(&str, &str)]) -> Vec<u8> {
    let mut json = TRIANGLE.to_string();
    for (from, to) in edits {
        assert!(json.contains(from), "{from}");
        json = json.replacen(from, to, 1);
    }
    glb(&json, &triangle_bin([0, 1, 2]))
}

fn shipped(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            shipped(&path, out);
        } else if path.extension().is_some_and(|x| x == "glb") {
            out.push(path);
        }
    }
}

fn shipped_models() -> Vec<PathBuf> {
    let mut out = Vec::new();
    shipped(&Path::new(env!("PXL_REPO")).join("asset/content"), &mut out);
    out.sort();
    assert!(!out.is_empty());
    out
}

#[test]
fn every_shipped_model_parses() {
    for path in shipped_models() {
        let gltf =
            parse(&fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(!gltf.meshes.is_empty(), "{}", path.display());
    }
}

#[test]
fn the_triangle_parses_as_written() {
    let gltf = parse(&edited(&[])).unwrap();
    assert_eq!(gltf.roots, [0]);
    assert_eq!(gltf.meshes[0][0].position.len(), 9);
    assert_eq!(gltf.meshes[0][0].index, [0, 1, 2]);
}

#[test]
fn an_index_past_the_vertices_is_refused() {
    let bytes = glb(TRIANGLE, &triangle_bin([0, 1, 3]));
    assert_eq!(refused(&bytes), Errno::EBADMSG);
}

#[test]
fn a_file_that_names_what_is_not_there_is_refused() {
    let cases: &[(&str, &[(&str, &str)])] = &[
        (
            "an accessor past its view",
            &[(
                r#""componentType":5126,"count":3"#,
                r#""componentType":5126,"count":4"#,
            )],
        ),
        (
            "a view past its buffer",
            &[(
                r#""byteOffset":36,"byteLength":6"#,
                r#""byteOffset":36,"byteLength":600"#,
            )],
        ),
        (
            "a view whose end overflows",
            &[(
                r#""byteOffset":36,"byteLength":6"#,
                r#""byteOffset":36,"byteLength":18446744073709551615"#,
            )],
        ),
        (
            "a stride under its element",
            &[(r#""byteLength":36}"#, r#""byteLength":36,"byteStride":4}"#)],
        ),
        ("an unknown component type", &[("5126", "5127")]),
        (
            "a position not of three",
            &[(r#""type":"VEC3""#, r#""type":"VEC2""#)],
        ),
        (
            "a buffer that is not there",
            &[(
                r#"{"buffer":0,"byteOffset":0"#,
                r#"{"buffer":1,"byteOffset":0"#,
            )],
        ),
        (
            "an accessor that is not there",
            &[(r#""indices":1"#, r#""indices":2"#)],
        ),
        ("a negative index", &[(r#""indices":1"#, r#""indices":-1"#)]),
        (
            "a fractional index",
            &[(r#"{"mesh":0}"#, r#"{"mesh":0.5}"#)],
        ),
        (
            "a mesh that is not there",
            &[(r#"{"mesh":0}"#, r#"{"mesh":1}"#)],
        ),
        (
            "a skin that is not there",
            &[(r#"{"mesh":0}"#, r#"{"mesh":0,"skin":0}"#)],
        ),
        (
            "a child that is not there",
            &[(r#"{"mesh":0}"#, r#"{"mesh":0,"children":[1]}"#)],
        ),
        (
            "a node its own child",
            &[(r#"{"mesh":0}"#, r#"{"mesh":0,"children":[0]}"#)],
        ),
        (
            "a cycle",
            &[(
                r#"{"mesh":0}"#,
                r#"{"mesh":0},{"children":[2]},{"children":[1]}"#,
            )],
        ),
        (
            "a node with two parents",
            &[(
                r#"{"mesh":0}"#,
                r#"{"mesh":0,"children":[2]},{"children":[2]},{}"#,
            )],
        ),
        (
            "a root with a parent",
            &[
                (r#""nodes":[0]"#, r#""nodes":[0,1]"#),
                (r#"{"mesh":0}"#, r#"{"mesh":0,"children":[1]},{}"#),
            ],
        ),
        (
            "a root listed twice",
            &[(r#""nodes":[0]"#, r#""nodes":[0,0]"#)],
        ),
        (
            "a scene that is not there",
            &[(r#""scene":0"#, r#""scene":1"#)],
        ),
        (
            "a normal of another count",
            &[
                (r#""POSITION":0}"#, r#""POSITION":0,"NORMAL":2}"#),
                (
                    r#""type":"SCALAR"}]"#,
                    r#""type":"SCALAR"},{"bufferView":0,"componentType":5126,"count":2,"type":"VEC3"}]"#,
                ),
            ],
        ),
    ];
    for (what, edits) in cases {
        assert_eq!(
            parse(&edited(edits)).map(|_| ()).map_err(|e| e.errnum),
            Err(Errno::EBADMSG),
            "{what}"
        );
    }
}

#[test]
fn what_is_not_read_is_refused_rather_than_misread() {
    let sparse = edited(&[(
        r#""count":3,"type":"VEC3"}"#,
        r#""count":3,"type":"VEC3","sparse":{"count":1}}"#,
    )]);
    assert_eq!(refused(&sparse), Errno::ENOTSUP);
    let required = edited(&[(
        r#"{"asset":"#,
        r#"{"extensionsRequired":["KHR_draco_mesh_compression"],"asset":"#,
    )]);
    assert_eq!(refused(&required), Errno::ENOTSUP);
}

#[test]
fn a_chunk_past_the_file_is_refused() {
    let mut bytes = edited(&[]);
    bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(refused(&bytes), Errno::EBADMSG);
}

#[test]
fn a_file_that_decodes_past_its_size_is_refused() {
    // Zeros of no view, a count the file does not hold.
    let zeros = edited(&[(
        r#"{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}"#,
        r#"{"componentType":5126,"count":1e9,"type":"VEC3"}"#,
    )]);
    assert_eq!(refused(&zeros), Errno::EFBIG);
    // One large accessor read again for each of a few primitives.
    let vertices = 10_000;
    let primitive = r#"{"attributes":{"POSITION":0},"indices":1}"#;
    let mut json = TRIANGLE.to_string();
    for (from, to) in [
        (primitive, vec![primitive; 64].join(",")),
        (
            r#""count":3,"type":"VEC3""#,
            format!(r#""count":{vertices},"type":"VEC3""#),
        ),
        (
            r#""byteOffset":0,"byteLength":36"#,
            format!(r#""byteOffset":0,"byteLength":{}"#, vertices * 12),
        ),
        (
            r#""byteOffset":36,"byteLength":6"#,
            format!(r#""byteOffset":{},"byteLength":6"#, vertices * 12),
        ),
    ] {
        assert!(json.contains(from), "{from}");
        json = json.replacen(from, &to, 1);
    }
    let mut bin = vec![0; vertices * 12];
    bin.extend([0, 0, 1, 0, 2, 0]);
    assert_eq!(refused(&glb(&json, &bin)), Errno::EFBIG);
}

#[test]
fn a_mesh_drawn_at_many_nodes_is_counted_at_each() {
    let nodes = vec![r#"{"mesh":0}"#; MAX_SCENE_PRIMITIVES as usize + 1].join(",");
    let roots: Vec<String> = (0..=MAX_SCENE_PRIMITIVES).map(|i| i.to_string()).collect();
    let instanced = edited(&[
        (
            r#""nodes":[0]"#,
            &format!(r#""nodes":[{}]"#, roots.join(",")),
        ),
        (r#"{"mesh":0}"#, &nodes),
    ]);
    assert_eq!(refused(&instanced), Errno::EFBIG);
}

#[test]
fn a_large_accessor_read_for_many_primitives_is_looked_up_once() {
    // A megabyte of extras on the accessor every primitive reads: looked up
    // again for each, it would be scanned some ten thousand times.
    let extras = format!(r#","extras":"{}"}}"#, "x".repeat(1 << 20));
    let primitive = r#"{"attributes":{"POSITION":0},"indices":1}"#;
    let many = vec![primitive; 1500].join(",");
    let gltf = parse(&edited(&[
        (
            r#""count":3,"type":"VEC3"}"#,
            &format!(r#""count":3,"type":"VEC3"{extras}"#),
        ),
        (primitive, &many),
    ]))
    .unwrap();
    assert_eq!(gltf.meshes[0].len(), 1500);
}

#[test]
fn a_deep_chain_of_nodes_is_walked_without_recursion() {
    let depth = 100_000;
    let mut nodes: Vec<String> = (1..depth)
        .map(|c| format!(r#"{{"children":[{c}]}}"#))
        .collect();
    nodes.push(r#"{"mesh":0}"#.to_string());
    let gltf = parse(&edited(&[(r#"{"mesh":0}"#, &nodes.join(","))])).unwrap();
    assert_eq!(scene_nodes(&gltf).len(), depth);
    assert_eq!(world_matrices(&gltf).len(), depth);
}

#[test]
fn any_number_in_a_shipped_model_pushed_up_is_parsed_or_refused() {
    let smallest = shipped_models()
        .into_iter()
        .min_by_key(|path| fs::metadata(path).unwrap().len())
        .unwrap();
    let bytes = fs::read(smallest).unwrap();
    let json_end = 20 + u32_le(&bytes, 12) as usize;
    for at in 20..json_end {
        if bytes[at].is_ascii_digit() && bytes[at] != b'9' {
            let mut pushed = bytes.clone();
            pushed[at] = b'9';
            let _ = parse(&pushed);
        }
    }
}
