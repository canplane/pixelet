// pxl-tool/spec.rs

//! Writes the TypeScript and Rust generated from `spec/` JSON, through
//! xpute-tool's generators: its data specs and its command table (data.rs,
//! cmd.rs), and what only pixelet generates beside them. It links none of the crates it
//! writes into, so it builds when their generated code is broken.

pub mod asset;
pub mod bake;
pub mod catalog;
pub mod cmd;
pub mod data;
pub mod ingested;
pub mod route;
pub mod setting;

use serde::Deserialize;

pub use xpute_tool::spec::{
    align_star, casing_note, enum_name, file, js_num, json_str, laid_out, line_comments, packed,
    pascal, pretty, rgb_literal, rs_doc, rs_ident, snake, ts_doc, Node, Output,
};

use crate::root;

/// xpute-tool reads and writes under this repository's root.
pub fn repo() {
    xpute_tool::set_root(root());
}

pub fn read(path: &str) -> Result<Node, String> {
    repo();
    xpute_tool::spec::read(path)
}

pub fn read_as<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T, String> {
    repo();
    xpute_tool::spec::read_as(path)
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    out.extend(cmd::outputs()?);
    out.extend(setting::outputs()?);
    out.extend(asset::color_outputs()?);
    out.extend(asset::cube_outputs()?);
    out.extend(asset::decor_outputs()?);
    out.extend(bake::published_outputs()?);
    out.extend(bake::object_outputs()?);
    out.extend(bake::place_theme_outputs()?);
    out.extend(data::outputs()?);
    out.extend(ingested::outputs()?);
    out.extend(catalog::outputs()?);
    out.extend(route::outputs()?);
    Ok(out)
}

/// Writes every generated file that differs from what its JSON writes, or,
/// not `write`, fails where one does.
pub fn generate(write: bool) -> Result<(), String> {
    repo();
    xpute_tool::spec::apply(outputs()?, write)
}
