// pxl-tool/spec.rs

//! Writes the TypeScript and Rust generated from `spec/` JSON: its data
//! specs (data.rs) and its command table (cmd.rs) through xpute's generator,
//! and what else it generates beside them. It links none of the crates it
//! writes into, so it builds when their generated code is broken. Its
//! generators write through xpute-gen's writer and follow the code and naming
//! skills.

pub mod asset;
pub(crate) mod bake;
pub mod catalog;
pub mod cmd;
pub mod data;
pub mod ingest;
pub mod route;
pub mod setting;

/// What xpute's generator writes and reads with, shared: the generated file,
/// the JSON's tree, and the text helpers.
pub use xpute_gen::spec::{
    align_star, file, mark, js_num, json_str, line_comments, pascal, plural, rs_doc, rs_field,
    rs_const, rs_member, ts_doc, Node, Output,
};

/// `#rrggbb`, a color as the specs write one, as one number, `0xRRGGBB`, for
/// the tool's own work on it (the texture pack's ground); the generated code
/// writes the string, which pxl-spec's `rgb` reads.
pub fn rgb(hex: &str) -> Result<u32, String> {
    hex.strip_prefix('#')
        .filter(|d| d.len() == 6 && d.chars().all(|c| c.is_ascii_hexdigit()))
        .and_then(|d| u32::from_str_radix(d, 16).ok())
        .ok_or(format!("color JSON: \"{hex}\" is not #rrggbb"))
}

use serde::Deserialize;
use xpute_gen::spec::generate_at;

use crate::asset::symbol;
use crate::brand;
use crate::root;

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    out.extend(cmd::outputs()?);
    out.extend(setting::outputs()?);
    out.extend(asset::color_outputs()?);
    out.extend(asset::decor_outputs()?);
    out.extend(bake::publication_outputs()?);
    out.extend(bake::place_theme_outputs()?);
    out.extend(data::outputs()?);
    out.extend(ingest::outputs()?);
    out.extend(catalog::outputs()?);
    out.extend(route::outputs()?);
    Ok(out)
}

/// What another generator writes, which `spec` leaves be: xpute's tree, by
/// xpute-tool, and the files bin/asset.rs and bin/brand.rs write from what
/// they read outside spec/.
const OTHERS: [&str; 4] = ["xpute/", symbol::RS, symbol::TS, brand::TS];

/// Writes every generated file that differs from what its source writes and deletes every one
/// that nothing writes any more; where `write` is false, it fails wherever it finds either
/// instead.
pub fn generate(write: bool) -> Result<(), String> {
    generate_at(&root(), outputs()?, &OTHERS, write)
}

pub fn read(path: &str) -> Result<Node, String> {
    read_as(path)
}

pub(crate) fn read_as<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T, String> {
    let text = std::fs::read_to_string(root().join(path)).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}
