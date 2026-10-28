// pxl-render/param.test.rs

use super::*;

#[test]
fn every_module_of_settings_is_written() {
    // A module left out of `ALL` would go unwritten into the shaders, and
    // only a shader that reads one of its settings would say so.
    let modules = include_str!("param.rs").matches("params! {").count();
    assert_eq!(ALL.len(), modules, "a module's settings missing from `ALL`");
    let mut paths: Vec<&str> = ALL.iter().map(|m| m[0].0).collect();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(paths.len(), ALL.len(), "a module's settings in `ALL` twice");
}
