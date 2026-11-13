// xpute-gpu-native-host/surface.rs

//! These are the surfaces a guest names by number, and the window's surface
//! that a program binds to each.

use xpute_gpu_link::handle::{handle_slot, SLOT_BITS};

use super::configure::ConfigureAt;
use super::GpuHost;

/// This is what the guest asked of a surface. It holds whether or not a window's
/// surface is bound, so a window bound later is configured as asked.
pub(super) struct Surface {
    /// This holds CONFIGURE's format, usage and whether alpha is premultiplied.
    pub(super) cfg: Option<(wgpu::TextureFormat, wgpu::TextureUsages, bool)>,
    /// This is RESIZE's size, at least a pixel each way.
    pub(super) size: [u32; 2],
    bound: Option<Bound>,
    /// This is the handle bound here. A slot outlives its handles, so an old
    /// handle let go must not release the one bound since.
    pub(super) handle: Option<u32>,
    /// This is the newest handle bound here, kept after it is let go, so that
    /// an older handle's late bind is refused.
    newest_handle: Option<u32>,
    /// This run's texture is here, and every CURRENT of the run shares it.
    pub(super) frame: Option<Frame>,
    /// This is the texture drawn into where the window gives none. It is kept
    /// until the format, usage or size asked for changes, so a hidden window
    /// costs no new texture each frame.
    offscreen: Option<wgpu::Texture>,
}

struct Bound {
    surface: wgpu::Surface<'static>,
    /// This surface is not yet configured as asked, so no texture is taken from
    /// it until it is.
    stale: bool,
}

/// A frame is the window's texture, shown after the run; or, where the
/// window gives none, a texture never shown, since WebGPU's getCurrentTexture
/// always gives one.
pub(super) enum Frame {
    Surface(wgpu::SurfaceTexture),
    Offscreen(wgpu::Texture),
}

impl Default for Surface {
    fn default() -> Self {
        Surface {
            cfg: None,
            size: [1, 1],
            bound: None,
            handle: None,
            newest_handle: None,
            frame: None,
            offscreen: None,
        }
    }
}

impl Frame {
    pub(super) fn texture(&self) -> &wgpu::Texture {
        match self {
            Frame::Surface(f) => &f.texture,
            Frame::Offscreen(t) => t,
        }
    }
}

/// This says whether handle `a`'s generation is behind `b`'s. Generations wrap
/// in 16 bits.
fn older(a: u32, b: u32) -> bool {
    (((a >> SLOT_BITS) as u16).wrapping_sub((b >> SLOT_BITS) as u16) as i16) < 0
}

impl GpuHost {
    /// Binds the window's surface `s` to the guest's surface `handle`, and
    /// configures it once the guest has asked for a configuration. It answers
    /// false, binding nothing, where a newer handle has bound the slot since.
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

    /// Lets `handle`'s surface go, if `handle` is still the one bound in its
    /// slot, and says whether it was. The guest's surface is then drawn into
    /// textures never shown.
    pub fn unbind(&mut self, handle: u32) -> bool {
        let Some(slot) = self.release(handle) else {
            return false;
        };
        let s = self.surface(slot);
        s.bound = None;
        s.frame = None;
        true
    }

    /// Takes `handle`'s slot for it. It answers none where the slot has seen a
    /// newer handle.
    pub(super) fn claim(&mut self, handle: u32) -> Option<u32> {
        let slot = handle_slot(handle);
        let s = self.surface(slot);
        if s.newest_handle.is_some_and(|newest| older(handle, newest)) {
            return None;
        }
        s.newest_handle = Some(handle);
        s.handle = Some(handle);
        Some(slot)
    }

    /// Frees `handle`'s slot, if `handle` is the one bound there.
    fn release(&mut self, handle: u32) -> Option<u32> {
        let slot = handle_slot(handle);
        let s = self
            .surfaces
            .get_mut(slot as usize)
            .filter(|s| s.handle == Some(handle))?;
        s.handle = None;
        Some(slot)
    }

    /// Configures every bound surface that is not configured as asked. Run it
    /// where `configure_at` says, while nothing else uses the host.
    pub fn configure_surfaces(&mut self) {
        let frame_latency = self.frame_latency;
        for (n, s) in self.surfaces.iter_mut().enumerate() {
            let (Some(b), Some((format, usage, premultiplied))) =
                (s.bound.as_mut().filter(|b| b.stale), s.cfg)
            else {
                continue;
            };
            let caps = b.surface.get_capabilities(&self.adapter);
            assert!(
                caps.formats.contains(&format),
                "[gpu] surface {n} cannot show {format:?} on this adapter"
            );
            // A premultiplied surface shows what lies behind it where it is
            // clear, as a premultiplied canvas does. Metal offers only opaque
            // and post-multiplied, so post-multiplied stands in there. An
            // opaque surface takes opaque or inherit, whichever it offers
            // (Adreno 740 offers inherit), as `Auto` picks.
            let alpha_mode = if premultiplied {
                [
                    wgpu::CompositeAlphaMode::PreMultiplied,
                    wgpu::CompositeAlphaMode::PostMultiplied,
                ]
                .into_iter()
                .find(|m| caps.alpha_modes.contains(m))
                .unwrap_or_else(|| panic!("[gpu] surface {n} cannot show through on this adapter"))
            } else {
                wgpu::CompositeAlphaMode::Auto
            };
            let cfg = wgpu::SurfaceConfiguration {
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
            b.surface.configure(&self.device, &cfg);
            b.stale = false;
        }
    }

    /// This takes `surface`'s texture for this run: the window's where it
    /// gives one, else one never shown. An outdated surface is configured and
    /// asked once more. A suboptimal one is used this run and configured
    /// before the next.
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
                wgpu::CurrentSurfaceTexture::Success(f) => return Frame::Surface(f),
                wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                    b.stale = true;
                    self.configure_at.ask();
                    return Frame::Surface(f);
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
        let Some((format, usage, _)) = s.cfg else {
            panic!("[gpu] surface {surface} drawn before it is configured")
        };
        let usage = usage | wgpu::TextureUsages::RENDER_ATTACHMENT;
        let size = wgpu::Extent3d {
            width: s.size[0],
            height: s.size[1],
            depth_or_array_layers: 1,
        };
        let reused = s
            .offscreen
            .take()
            .filter(|t| t.format() == format && t.usage() == usage && t.size() == size);
        let texture = reused.unwrap_or_else(|| {
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
        s.offscreen = Some(texture.clone());
        Frame::Offscreen(texture)
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
        let Some(b) = s.bound.as_mut().filter(|_| s.cfg.is_some()) else {
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
