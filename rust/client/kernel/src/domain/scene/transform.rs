// pxl-kernel/domain/scene/transform.rs

//! Where an instance stands in its frame, which its holder decides: an
//! character its figure or its preview stands, the decor the ghost shows.

use pxl_spec::space::coord::v3::V3;

/// Where, in units, its turn about Z, and its scale over its model as
/// registered, which stands the height it was registered at
/// (domain/scene/model/registry.rs `Model::scale`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub at: V3,
    pub rot: f32,
    pub scale: f32,
}
