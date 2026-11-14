// pxl-tool/spec/data.rs

//! pixelet's data specs, written by xpute's generator (xpute-gen
//! spec/data.rs): JSON of named values as Rust constants, and as TypeScript
//! `as const` where a spec lists a TypeScript file. The Rust carries no
//! names: a list of them is its count, and an enum where the spec asks for
//! one, and a table's keys are not listed; the TypeScript keeps them for what
//! a person reads. A `file` is a reference to an asset: the tool numbers each
//! and hands the generator plain numbers (`with_numbers`).

use xpute_gen::spec::data::{table_outputs, DataSpec};

use super::bake::PUBLICATION;
use super::catalog::{run_of, FILE};
use super::ingest::files;
use super::{read, Node, Output};
use crate::root;

const DATA_SPECS: [DataSpec; 18] = [
    DataSpec {
        json: "spec/client/core/sys/manifest.json",
        ts: Some("ts/client/src/abi/manifest.spec.ts"),
        rs: Some("rust/client/core/src/spec/manifest.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/spec/bake/object.json",
        ts: Some("ts/client/src/ui/feature/world/object.spec.ts"),
        rs: Some("rust/spec/src/bake/object.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/spec/asset/marker.json",
        ts: Some("ts/client/src/ui/shared/asset/marker.spec.ts"),
        rs: Some("rust/spec/src/asset/marker.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/spec/asset/id.json",
        ts: Some("ts/client/src/ui/shared/asset/id.spec.ts"),
        rs: Some("rust/spec/src/asset/id.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/spec/division.json",
        ts: Some("ts/client/src/ui/feature/division/division.spec.ts"),
        rs: Some("rust/spec/src/division.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/spec/search.json",
        ts: Some("ts/client/src/ui/feature/search/search.spec.ts"),
        rs: Some("rust/spec/src/search.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/spec/asset/character.json",
        ts: Some("ts/client/src/ui/shared/asset/character.spec.ts"),
        rs: Some("rust/spec/src/asset/character.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/spec/asset/cube.json",
        ts: Some("ts/client/src/abi/cube.spec.ts"),
        rs: Some("rust/spec/src/asset/cube.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/spec/asset/sky.json",
        rs: Some("rust/spec/src/asset/sky.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/client/core/domain/scene/gizmo/gizmo.json",
        ts: Some("ts/client/src/abi/gizmo.spec.ts"),
        rs: Some("rust/client/core/src/spec/gizmo.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/client/render/api.json",
        ts: Some("ts/client/src/abi/gpu.spec.ts"),
        rs: Some("rust/client/render/src/spec/api.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/client/render/frame/shader/debug_look.json",
        ts: Some("ts/client/src/abi/debug_look.spec.ts"),
        rs: Some("rust/client/render/src/frame/shader/debug_look.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/client/core/sys/io/file.json",
        ts: Some("ts/client/src/abi/file.spec.ts"),
        rs: Some("rust/client/core/src/spec/file.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/client/core/sys/io/demand.json",
        ts: Some("ts/client/src/abi/demand.spec.ts"),
        rs: Some("rust/client/core/src/spec/demand.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/client/core/sys/sched/phase.json",
        ts: Some("ts/client/src/abi/phase.spec.ts"),
        rs: Some("rust/client/core/src/spec/phase.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/client/core/perf/step.json",
        ts: Some("ts/client/src/abi/step.spec.ts"),
        rs: Some("rust/client/core/src/spec/step.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/client/render/gpu/span.json",
        ts: Some("ts/client/src/abi/span.spec.ts"),
        rs: Some("rust/client/render/src/spec/span.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/client/core/perf/perf.json",
        ts: Some("ts/client/src/abi/perf.spec.ts"),
        rs: Some("rust/client/core/src/spec/perf.spec.rs"),
        ..DataSpec::DEFAULT
    },
];

/// Each spec's files, as the generator writes them from plain data. A file
/// reference is, in Rust, the slot the catalog numbers it by: the spec's
/// references in document order from its run's first (catalog.rs); in
/// TypeScript, the file's number after ASSET, its place in the file list
/// from 1.
pub fn outputs() -> Result<Vec<Output>, String> {
    let files: Vec<String> = files()?.into_iter().map(|f| f.path).collect();
    // The specs whose enums a spec may name by `$uses`.
    let peers: Vec<DataSpec> = DATA_SPECS.into_iter().chain([PUBLICATION]).collect();
    let mut out = Vec::new();
    for d in &DATA_SPECS {
        let spec = read(d.json)?;
        let mut refs = 0;
        let ts = with_numbers(&spec, None, &mut |path| {
            refs += 1;
            let k = files.iter().position(|f| f == path);
            k.map(|k| k as u16 + 1).ok_or(format!("{}: {path} is no file the ingest lists", d.json))
        })?;
        if refs == 0 {
            out.extend(table_outputs(&root(), d, spec, &peers)?);
            continue;
        }
        let (first, cnt) = run_of(d.json)?;
        let mut next = first;
        let rs = with_numbers(&spec, None, &mut |_| {
            next += 1;
            Ok(next - 1)
        })?;
        if next != first + cnt {
            return Err(format!("{}: its references are not the {cnt} of its run", d.json));
        }
        out.extend(table_outputs(&root(), &DataSpec { ts: None, ..*d }, rs, &peers)?);
        out.extend(table_outputs(&root(), &DataSpec { rs: None, ..*d }, ts, &peers)?);
    }
    Ok(out)
}

/// `n` with each `file` leaf under it replaced by its number, which `number`
/// gives in document order, as catalog.rs `leaves` walks them, and its
/// `$type` by `u16`.
fn with_numbers(
    n: &Node,
    ty: Option<&str>,
    number: &mut impl FnMut(&str) -> Result<u16, String>,
) -> Result<Node, String> {
    let ty = n.str_at("$type").or(ty);
    Ok(match n {
        Node::Str(s) if ty == Some(FILE) => Node::Num(f64::from(number(s)?)),
        Node::Arr(a) => Node::Arr(a.iter().map(|e| with_numbers(e, ty, number)).collect::<Result<_, _>>()?),
        Node::Obj(entries) => Node::Obj(
            entries
                .iter()
                .map(|(k, v)| match k.as_str() {
                    "$type" if v.str() == Some(FILE) => Ok((k.clone(), Node::Str("u16".into()))),
                    "$val" => Ok((k.clone(), with_numbers(v, ty, number)?)),
                    k if k.starts_with('$') => Ok((k.to_string(), v.clone())),
                    _ => Ok((k.clone(), with_numbers(v, ty, number)?)),
                })
                .collect::<Result<_, String>>()?,
        ),
        n => n.clone(),
    })
}
