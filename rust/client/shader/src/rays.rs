// pxl-shader/rays.rs

//! The eye's rays' image (`Rays`), the screen's size on the GPU, and the
//! passes alone that bind it, the ones that draw a pixel's ray or read the
//! rays drawn: the cast, its walks again and the stops' faces put at their
//! pixels (cast.rs), and the light's receivers (light_mark.rs). `Rays` is
//! this module's own, so an entry anywhere else that names it does not
//! compile; the beam, the cells, the samples, decor and the shadow map read
//! the eye's angles alone.

use spirv_std::Image;

/// The eye's rays, a drawn pixel each, a layer a word of the ray's
/// (cast.rs `word`; pxl-spec-core `ray`): the cast writes them.
type Rays = Image!(2D, format = r32ui, sampled = false, arrayed = true);

#[path = "cast.rs"]
pub(super) mod cast;
#[path = "light_mark.rs"]
pub(super) mod light_mark;
