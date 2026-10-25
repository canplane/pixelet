// xpute-gpu/host/surface.rs

//! The surfaces a guest names by number, and the window's surface a program
//! binds to one.

use xpute_guest::abi::handle::{handle_slot, SLOT_BITS};

use super::configure::ConfigureAt;
use super::GpuHost;

/// What the guest asked of a surface, which holds whether or not a window's
/// surface is bound to it.
pub(super) struct Surface {
    /// CONFIGURE's format, usage and whether alpha is premultiplied.
    pub(super) asked: Option<(wgpu::TextureFormat, wgpu::TextureUsages, bool)>,
    /// RESIZE's, at least a pixel each way.
    pub(super) size: [u32; 2],
    bound: Option<Bound>,
    /// The handle bound here, 0 for none: a slot outlives a handle, so an old
    /// one let go must not take the one bound since.
    pub(super) handle: u32,
    /// The newest handle bound here, kept after it is let go, so an older
    /// one's late bind is refused.
    seen: u32,
    /// This turn's texture, the same to every CURRENT of the turn.
    pub(super) frame: Option<Frame>,
    /// Drawn where the window gives none, kept until what is asked changes,
    /// so a hidden window costs no texture a frame.
    unshown: Option<wgpu::Texture>,
}

struct Bound {
    surface: wgpu::Surface<'static>,
    /// Not configured as asked, so no frame is taken from it until it is.
    stale: bool,
}

/// The window's texture, shown at the turn's end; or, where it gives none,
/// one never shown, as WebGPU's getCurrentTexture always gives a texture.
pub(super) enum Frame {
    Shown(wgpu::SurfaceTexture),
    Unshown(wgpu::Texture),
}

impl Default for Surface {
    fn default() -> Self {
        Surface {
            asked: None,
            size: [1, 1],
            bound: None,
            handle: 0,
            seen: 0,
            frame: None,
            unshown: None,
        }
    }
}

impl Frame {
    pub(super) fn texture(&self) -> &wgpu::Texture {
        match self {
            Frame::Shown(f) => &f.texture,
            Frame::Unshown(t) => t,
        }
    }
}

/// Whether `a`'s generation is behind `b`'s, generations wrapping in 16 bits.
fn older(a: u32, b: u32) -> bool {
    (((a >> SLOT_BITS) as u16).wrapping_sub((b >> SLOT_BITS) as u16) as i16) < 0
}

impl GpuHost {
    /// The window's surface `s` bound to the guest's surface `handle`,
    /// configured once the guest has asked: whether it was, which it is not
    /// where a newer handle has bound the slot since.
    pub fn bind(&mut self, handle: u32, s: wgpu::Surface<'static>) -> bool {
        let Some(slot) = self.claim(handle) else {
            return false;
        };
        self.surface(slot).bound = Some(Bound {
            surface: s,
            stale: true,
        });
        self.want_configured(slot);
        true
    }

    /// Lets `handle`'s surface go, if it is still the one bound under its
    /// slot, which is then drawn into frames never shown: whether it was.
    pub fn unbind(&mut self, handle: u32) -> bool {
        let Some(slot) = self.release(handle) else {
            return false;
        };
        let s = self.surface(slot);
        s.bound = None;
        s.frame = None;
        true
    }

    /// The slot, or none where it has seen a newer handle.
    pub(super) fn claim(&mut self, handle: u32) -> Option<u32> {
        let slot = handle_slot(handle);
        let s = self.surface(slot);
        if s.seen != 0 && older(handle, s.seen) {
            return None;
        }
        s.seen = handle;
        s.handle = handle;
        Some(slot)
    }

    /// `handle`'s slot, if `handle` is the one bound there, which then holds none.
    fn release(&mut self, handle: u32) -> Option<u32> {
        let slot = handle_slot(handle);
        let s = self
            .surfaces
            .get_mut(slot as usize)
            .filter(|s| handle != 0 && s.handle == handle)?;
        s.handle = 0;
        Some(slot)
    }

    /// Every bound surface not configured as asked, configured: run where
    /// `configure_at` says, while nothing else uses the host.
    pub fn configure_surfaces(&mut self) {
        let frame_latency = self.frame_latency;
        for (n, s) in self.surfaces.iter_mut().enumerate() {
            let (Some(b), Some((format, usage, premultiplied))) =
                (s.bound.as_mut().filter(|b| b.stale), s.asked)
            else {
                continue;
            };
            let caps = b.surface.get_capabilities(&self.adapter);
            assert!(
                caps.formats.contains(&format),
                "[gpu] surface {n} cannot show {format:?} on this adapter"
            );
            // Shows through where clear, as a premultiplied canvas does; Metal
            // offers only opaque and post-multiplied.
            let alpha_mode = [
                wgpu::CompositeAlphaMode::PreMultiplied,
                wgpu::CompositeAlphaMode::PostMultiplied,
            ]
            .into_iter()
            .find(|m| premultiplied && caps.alpha_modes.contains(m))
            .unwrap_or(wgpu::CompositeAlphaMode::Opaque);
            let config = wgpu::SurfaceConfiguration {
                usage,
                format,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width: s.size[0],
                height: s.size[1],
                present_mode: self.present_mode,
                desired_maximum_frame_latency: frame_latency,
                alpha_mode,
                view_formats: Vec::new(),
            };
            b.surface.configure(&self.device, &config);
            b.stale = false;
        }
    }

    /// `surface`'s texture for this turn: the window's where it gives one,
    /// else one never shown. An outdated surface is configured and asked
    /// again once; a suboptimal one is shown and configured before the next.
    pub(super) fn current(&mut self, surface: u32) -> Frame {
        let mut asked = false;
        loop {
            if let ConfigureAt::Here = self.configure_at {
                self.configure_surfaces();
            }
            let Some(b) = self.surface(surface).bound.as_mut().filter(|b| !b.stale) else {
                break;
            };
            match b.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f) => return Frame::Shown(f),
                wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                    b.stale = true;
                    self.configure_at.ask();
                    return Frame::Shown(f);
                }
                wgpu::CurrentSurfaceTexture::Outdated if !asked => {
                    asked = true;
                    self.want_configured(surface);
                }
                _ => break,
            }
        }
        let device = self.device.clone();
        let s = self.surface(surface);
        let Some((format, usage, _)) = s.asked else {
            panic!("[gpu] surface {surface} drawn before it is configured")
        };
        let usage = usage | wgpu::TextureUsages::RENDER_ATTACHMENT;
        let size = wgpu::Extent3d {
            width: s.size[0],
            height: s.size[1],
            depth_or_array_layers: 1,
        };
        let kept = s
            .unshown
            .take()
            .filter(|t| t.format() == format && t.usage() == usage && t.size() == size);
        let texture = kept.unwrap_or_else(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        });
        s.unshown = Some(texture.clone());
        Frame::Unshown(texture)
    }

    pub(super) fn surface(&mut self, surface: u32) -> &mut Surface {
        let at = surface as usize;
        if self.surfaces.len() <= at {
            self.surfaces.resize_with(at + 1, Surface::default);
        }
        &mut self.surfaces[at]
    }

    pub(super) fn want_configured(&mut self, surface: u32) {
        let s = self.surface(surface);
        let Some(b) = s.bound.as_mut().filter(|_| s.asked.is_some()) else {
            return;
        };
        b.stale = true;
        s.frame = None;
        match self.configure_at {
            ConfigureAt::Here => self.configure_surfaces(),
            elsewhere => elsewhere.ask(),
        }
    }
}
