// pxl-render/gpu/renderer.test.rs

use super::*;
use crate::frame::shader::record::SHADERS;

/// The only test here that installs the calls' state, so nothing races for it.
#[test]
fn the_renderer_makes_every_group_as_its_layout_lays_it_out() {
    let device = api::test_device();
    api::install_apart(device);
    let g = gpu().or_bug(Errno::ENODEV);
    let mut pipelines = Pipelines::new();
    // A module a turn at least.
    let made = (0..SHADERS).any(|_| pipelines.make_modules(g));
    assert!(
        made,
        "every module is made in as many turns as there are shaders"
    );
    // Every bind group it makes is checked against its layout's group as it is made
    // (api.rs `Layout::holds`).
    Renderer::new(g, device, pipelines);
}
