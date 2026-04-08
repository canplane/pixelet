// @pxl/render/lib.rs
//
// Offscreen WebGPU renderer for the pxl engine (WASM worker context).
//
// Bootstrap sequence (driven by JS):
//   1. engine.init_render(offscreenCanvas)  — async; acquires adapter/device/surface
//   2. engine.tick()                         — called from requestAnimationFrame; advances one frame

// ============ Resources ============

/// Current camera state, updated each frame before tick().
#[derive(Default, Clone)]
pub struct CameraState {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub view_proj: [f32; 16],
}

/// A terrain chunk queued for GPU upload.
pub struct TerrainChunk {
    pub page_idx: u64,
    pub data: Vec<u8>,
}

// ============ RenderSurface ============

/// Live GPU handles for a configured wgpu surface.
struct RenderSurface {
    _canvas: web_sys::OffscreenCanvas, // keep canvas alive for the surface's lifetime
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

impl RenderSurface {
    async fn new(canvas: web_sys::OffscreenCanvas) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = canvas;
            Err("pxl-render: OffscreenCanvas surface requires wasm32".to_string())
        }

        #[cfg(target_arch = "wasm32")]
        {
            let width = canvas.width();
            let height = canvas.height();

            let instance_desc = wgpu::InstanceDescriptor {
                backends: wgpu::Backends::BROWSER_WEBGPU,
                ..Default::default()
            };
            let instance = wgpu::Instance::new(instance_desc);

            // SAFETY: canvas is stored in the same struct, keeping the JS object alive
            // as long as the surface exists.
            let surface = unsafe {
                let s = instance
                    .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
                    .map_err(|e| e.to_string())?;
                std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(s)
            };

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .ok_or("pxl-render: no WebGPU adapter")?;

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .map_err(|e| e.to_string())?;

            let caps = surface.get_capabilities(&adapter);
            let format = caps.formats[0];

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: width.max(1),
                height: height.max(1),
                present_mode: wgpu::PresentMode::AutoNoVsync,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            Ok(Self {
                _canvas: canvas,
                surface,
                device,
                queue,
                config,
            })
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Hello world: clear to a solid colour.
    fn draw(&self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.1, b: 0.2, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        self.queue.submit([encoder.finish()]);
        output.present();
        Ok(())
    }
}

// ============ RenderApp ============

/// Offscreen render context. Created synchronously; call `init(canvas).await` before `tick()`.
pub struct RenderApp {
    surface: Option<RenderSurface>,
    pub camera: CameraState,
    pub terrain_pending: Vec<TerrainChunk>,
}

impl Default for RenderApp {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderApp {
    pub fn new() -> Self {
        Self {
            surface: None,
            camera: CameraState::default(),
            terrain_pending: Vec::new(),
        }
    }

    /// Async init — acquires WebGPU adapter/device and configures the surface.
    /// Must be awaited before calling tick().
    pub async fn init(&mut self, canvas: web_sys::OffscreenCanvas) -> Result<(), String> {
        let surface = RenderSurface::new(canvas).await?;
        self.surface = Some(surface);
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(s) = &mut self.surface {
            s.resize(width, height);
        }
    }

    pub fn update_camera(&mut self, eye: [f32; 3], target: [f32; 3], view_proj: [f32; 16]) {
        self.camera = CameraState { eye, target, view_proj };
    }

    pub fn submit_terrain(&mut self, page_idx: u64, data: Vec<u8>) {
        self.terrain_pending.push(TerrainChunk { page_idx, data });
    }

    /// Advance one frame. No-op if init() has not been called yet.
    pub fn tick(&mut self) {
        let Some(surface) = &self.surface else { return; };
        if let Err(e) = surface.draw() {
            web_sys::console::warn_1(&format!("pxl-render: draw error: {e:?}").into());
        }
    }
}
