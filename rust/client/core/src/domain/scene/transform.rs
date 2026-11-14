// pxl-core/domain/scene/transform.rs

//! The placement of a model instance. The part that holds the instance
//! decides it: a character's walk places the character, and the ghost places
//! the decor it previews.

use pxl_spec::space::coord::v3::V3;

/// `at` is the position in world units and `rot` the turn about Z in radians.
/// `scale` multiplies the scale the model was registered with
/// (domain/scene/model/registry.rs `Model::scale`), so a scale of 1 draws the
/// model at the height it was registered at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transform {
    pub at: V3,
    pub rot: f32,
    pub scale: f32,
}
