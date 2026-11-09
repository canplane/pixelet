// pxl-tool/spec/catalog.rs

//! The catalog: every `file` leaf of the asset specs, a slot each, numbered in
//! document order, spec by spec in `SPECS`' order. The Rust names each slot;
//! the catalog fills them with the file each names, by its number below the
//! asset root, and its extent (pxl-spec asset/catalog.rs reads the layout).
//! The runtime serves it, as hex in TypeScript.

use xpute_tool::spec::data::Run;

use super::ingest::{file_number, files};
use super::{file, read, Node, Output};

/// A data spec's `$type` for a file reference: in Rust its slot, in
/// TypeScript its number below the asset root.
pub(crate) const FILE: &str = "file";

pub(crate) const CUBE_JSON: &str = "spec/spec/asset/cube.json";
pub(crate) const DECOR_JSON: &str = "spec/spec/asset/decor.json";
const CHARACTER_JSON: &str = "spec/spec/asset/character.json";
const SKY_JSON: &str = "spec/spec/asset/sky.json";

/// The specs whose `file` leaves are slots, in the order they are numbered.
const SPECS: [&str; 4] = [CUBE_JSON, DECOR_JSON, CHARACTER_JSON, SKY_JSON];

/// Every slot's path, each spec's run of them in `SPECS`' order.
fn paths() -> Result<Vec<(&'static str, Vec<String>)>, String> {
    SPECS
        .into_iter()
        .map(|json| {
            let mut paths = Vec::new();
            leaves(&read(json)?, None, &mut paths);
            Ok((json, paths))
        })
        .collect()
}

/// The `file` leaves under `n`, in document order, as data.rs renders them.
fn leaves(n: &Node, ty: Option<&str>, out: &mut Vec<String>) {
    let ty = n.str_at("$type").or(ty);
    match n {
        _ if n.is_wrapper() => leaves(n.val(), ty, out),
        Node::Str(s) if ty == Some(FILE) => out.push(s.clone()),
        Node::Arr(a) => a.iter().for_each(|e| leaves(e, ty, out)),
        Node::Obj(_) => n.named().for_each(|(_, v)| leaves(v, ty, out)),
        _ => {}
    }
}

/// Each spec's run: its first slot, and how many it makes.
pub(crate) fn runs() -> Result<Vec<Run<'static>>, String> {
    let mut first = 0;
    paths()?
        .into_iter()
        .map(|(json, paths)| {
            let n = u16::try_from(paths.len()).map_err(|e| e.to_string())?;
            first += n;
            Ok((json, first - n, n))
        })
        .collect()
}

/// The run of the spec at `json`: its first slot and how many it makes.
pub(crate) fn run_of(json: &str) -> Result<(u16, u16), String> {
    runs()?
        .into_iter()
        .find(|(j, ..)| *j == json)
        .map(|(_, first, cnt)| (first, cnt))
        .ok_or(format!("{json}: no run of the catalog's"))
}

/// Every slot's path, the slot its place.
pub(crate) fn slots() -> Result<Vec<String>, String> {
    Ok(paths()?.into_iter().flat_map(|(_, p)| p).collect())
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let slots = slots()?;
    let files = files()?;
    let mut bytes = (slots.len() as u32).to_le_bytes().to_vec();
    let mut extents = Vec::with_capacity(slots.len() * 24);
    for path in &slots {
        let number = file_number(path)?;
        bytes.extend(number.to_le_bytes());
        for e in files[number as usize - 1].extent.unwrap_or([0.0; 3]) {
            extents.extend(e.to_le_bytes());
        }
    }
    bytes.extend(extents);
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let ts = [
        "// @pxl/client/service/sys/fs/catalog.spec.ts",
        "//",
        "// GENERATED from the asset specs and ingest.json",
        "//",
        "// The catalog the runtime plugs in, served below the asset root at `CATALOG`",
        "// (ts/client/src/service/sys/fs/asset.ts): the file each of the core's slots names and its",
        "// extent, laid out as pxl-spec asset/catalog.rs reads it, in hex.",
        "",
    ]
    .map(String::from)
    .into_iter()
    .chain([format!("export const CATALOG_HEX = \"{hex}\";")])
    .collect::<Vec<_>>();
    let rs = [
        "// pxl-spec/asset/catalog.spec.rs",
        "//",
        "// GENERATED from the asset specs",
        "",
        "use super::Slot;",
        "",
        "/// Every file reference the asset specs make, a slot each.",
    ]
    .map(String::from)
    .into_iter()
    .chain([
        format!("pub const SLOTS: usize = {};", slots.len()),
        String::new(),
        "/// The cube kind's texture pack (cube.json `pack`).".to_string(),
        format!("pub const pack: Slot = Slot({});", run_of(CUBE_JSON)?.0),
    ])
    .collect::<Vec<_>>();
    Ok(vec![
        Output::new("ts/client/src/service/sys/fs/catalog.spec.ts", file(&ts)),
        Output::new("rust/spec/src/asset/catalog.spec.rs", file(&rs)),
    ])
}
