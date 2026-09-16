// pxl_core/module/world/raycast.rs

//! The world's raycast, the kernel's (crates/pxl/core/src/world/raycast.rs):
//! `ray_hit_world` asks it over the chunks the scheduler holds and the
//! composites it keeps, and reads the hit back as doubles.

use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use pxl_shared::spec::space::v3::V3;
use xpute_core::kit::mat4::Vec3;

use crate::kernel::kernel;
use crate::render::mesh::voxel::MaterialClass;
use crate::world::build::World;
use crate::world::raycast::{hit_decor, DecorHit};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldHit {
    pub material_class: MaterialClass,
    pub face_normal: V3,
    pub hit_point: V3,
    pub t: f64,
    pub box_min: V3,
    pub box_max: V3,
}

/// A ray as an origin and a direction: what camera/ray.rs builds from the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayLike {
    pub origin: Vec3,
    pub direction: Vec3,
}

/// The map origin as a ray query carries it, in units.
pub fn ray_origin(map_origin: IVec) -> [f64; 2] {
    [ivec::to_real(ivec::X(map_origin)), ivec::to_real(ivec::Y(map_origin))]
}

/// The nearest box the ray strikes within `max_dist`, or None: the kernel's
/// walk of every chunk the ray's XY projection crosses. The ray is in the
/// local world frame — relative to `map_origin` on X and Y — and so is what
/// comes back.
pub fn ray_hit_world(ray: &RayLike, map_origin: IVec, max_dist: f64) -> Option<WorldHit> {
    let k = kernel();
    let mut world = World {
        volumes: k.volumes.as_mut()?,
        materials: &k.terrain,
        store: &k.objects,
        painter: &mut k.painter,
    };
    let (o, d) = (ray.origin, ray.direction);
    let h = k.raycast.hit(&mut world, &mut k.sched, ray_origin(map_origin), [o.x, o.y, o.z], [d.x, d.y, d.z], max_dist)?;
    Some(WorldHit {
        material_class: h.material_class as MaterialClass,
        face_normal: h.normal,
        hit_point: h.point,
        t: h.t,
        box_min: h.box_min,
        box_max: h.box_max,
    })
}

/// The placed decor entry the ray strikes within `max_dist`, or None:
/// the kernel's hit test over every placed instance the on-screen chunks show.
pub fn ray_hit_decor(ray: &RayLike, map_origin: IVec, max_dist: f64) -> Option<DecorHit> {
    let k = kernel();
    let world = World {
        volumes: k.volumes.as_mut()?,
        materials: &k.terrain,
        store: &k.objects,
        painter: &mut k.painter,
    };
    let (o, d) = (ray.origin, ray.direction);
    hit_decor(&world, &k.sched, ray_origin(map_origin), [o.x, o.y, o.z], [d.x, d.y, d.z], max_dist)
}
