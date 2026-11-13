// pxl-core/link/export/surface.rs

//! The surfaces the UI lends the GPU (xpute gpu/wit/surface.wit), each an
//! object the core serves under the handle the link issued it: opened by the
//! UI, resized by it, and closed as the UI drops it.

use std::rc::Rc;

use super::refuse_unless;
use crate::domain::control::sensor;
use crate::domain::scene::resolution;
use crate::domain::view::frame::pose::view_resize;
use crate::sys::sched::invalidate;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface;
use xpute_guest::link;
use xpute_link::Handle;
use xpute_gpu_guest::abi::surface as wit;
use xpute_gpu_link::spec::SURFACE_URI;
use xpute_kit::status::errno::Errno;

/// The surface the link issued `.0`, closed as the UI drops it.
struct Surface(Handle);

/// Mounts the surfaces' opener: each open opens a surface under its handle,
/// which draws nothing until SCENE_DRAW (scene.rs `draw`); ENFILE where every
/// surface is taken.
pub(super) fn mount() {
    link::mount(xpute_link::uri(SURFACE_URI), |_, h| {
        refuse_unless(surface::surface_open(h), Errno::ENFILE)?;
        invalidate();
        Ok(wit::surface(Rc::new(Surface(h))))
    });
}

impl wit::Surface for Surface {
    fn resize(&self, width: u32, height: u32, dpr: f32) -> Result<(), Errno> {
        resize(self.0, width, height, f64::from(dpr))
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        close(self.0);
    }
}

/// Makes the surface `handle` draw the world. The sensors are watched while
/// the world's surface is open, and the view is sized at once when the
/// surface's box is already known. Returns false when the surface is not
/// open.
pub(super) fn draw_world(handle: u32) -> bool {
    let Some(s) = surface::surface(handle) else {
        return false;
    };
    s.role = surface_role::WORLD;
    sensor::sensors_open();
    if s.px_w != 0 {
        s.ratio = resolution::surface_ratio(s.role, [s.px_w, s.px_h], s.dpr);
        view_resize(f64::from(s.px_w) / s.dpr, f64::from(s.px_h) / s.dpr);
    }
    invalidate();
    true
}

fn resize(h: Handle, width: u32, height: u32, dpr: f64) -> Result<(), Errno> {
    refuse_unless(dpr > 0.0, Errno::EINVAL)?;
    let role = surface::surface(h).map_or(0, |s| s.role);
    let ratio = resolution::surface_ratio(role, [width, height], dpr);
    refuse_unless(surface::surface_resize(h, width, height, dpr, ratio), Errno::EBADF)?;
    // The world's view is its surface's layout box, in CSS pixels.
    if role == surface_role::WORLD {
        view_resize(f64::from(width) / dpr, f64::from(height) / dpr);
    }
    invalidate();
    Ok(())
}

fn close(h: Handle) {
    let Some(role) = surface::surface(h).map(|s| s.role) else { return };
    super::super::close_surface(h);
    // A surface can close after the one replacing it has opened, so the
    // sensors stop with the last world's surface.
    let world_open = surface::surfaces().any(|s| s.role == surface_role::WORLD);
    if role == surface_role::WORLD && !world_open {
        sensor::sensors_close();
    }
}
