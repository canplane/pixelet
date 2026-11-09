// pxl-render/pass.rs

//! What is drawn: the screen's and the light's passes over the device (gpu.rs) and the rays
//! (ray.rs). The ambient occlusion, the air, the clouds, the sky and the stars, the shadow map
//! and the screen's finish and resolve, and the passes that draw what the core lists over the
//! world its rays cast: the markers, the decor and its impostors, the location indicator and
//! the characters. All of those but the markers mark the tiles their changed draws cover
//! (`mark`). The dev gizmos and the character surface are passes of their own here as well.

pub mod ao;
pub mod atmosphere;
pub mod character;
pub mod cloud;
pub mod debug;
pub mod decor;
pub mod finish;
pub mod impostor;
pub mod location;
pub mod mark;
pub mod marker;
pub mod model;
pub mod propagate;
pub mod resolve;
pub mod shadow;
pub mod skinned_mesh;
pub mod sky;
pub mod star;
