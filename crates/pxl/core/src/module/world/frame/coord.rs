// pxl_core/module/world/frame/coord.rs

use pxl_shared::spec::space::indexer::L4_BASE;
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use pxl_shared::spec::space::pvf3::projection;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;
use pxl_shared::spec::space::v3::V3;

pub fn get_local_world_origin(map_center: Option<&PVF3>) -> IVec {
    match map_center {
        None => 0,
        Some(c) => L4_BASE(c.i),
    }
}

pub fn local_world_to_canonical<'a>(out: &'a mut PVF3, local: &V3, map_origin: IVec) -> &'a mut PVF3 {
    projection::project(out, local[0], local[1], local[2]);
    out.i = ivec::add(out.i, map_origin);
    out
}
