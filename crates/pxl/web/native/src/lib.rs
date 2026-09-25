// pxl-native/lib.rs

//! The kernel on a device: pxl-kernel linked into the app, where pxl-wasm
//! instantiates it in a page. For now the one thing it does is the chain the
//! rest will stand on: the app's view hands over its layer, wgpu makes a
//! surface on it, and a frame reaches the screen.
//!
//! Called from the app's Swift through the C ABI (pxl_native.h), on the main
//! thread, one call at a time — the single thread `Global` is sound for.

use std::ffi::c_void;

use xpute_runtime::global::Global;

/// A surface over one view's layer, and the device drawing into it.
struct Surface {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

static SURFACE: Global<Option<Surface>> = Global::new();

/// A surface on `layer`, a `CAMetalLayer`, `width` × `height` pixels. 0 when
/// it is drawn into, -1 when no device would take it.
///
/// # Safety
/// `layer` is a live `CAMetalLayer` that outlives the surface: the view that
/// owns it closes the surface before it goes.
#[no_mangle]
pub unsafe extern "C" fn pxl_surface_open(layer: *mut c_void, width: u32, height: u32) -> i32 {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    // SAFETY: the caller's layer, alive for as long as the surface.
    let Ok(surface) = (unsafe { instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer)) }) else {
        return -1;
    };
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        ..Default::default()
    })) else {
        return -1;
    };
    let Ok((device, queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        // What the adapter has, not WebGPU's defaults: the simulator's Metal
        // falls short of some of those and refuses the device outright.
        required_limits: adapter.limits(),
        ..Default::default()
    })) else {
        return -1;
    };
    let Some(mut config) = surface.get_default_config(&adapter, width.max(1), height.max(1)) else {
        return -1;
    };
    config.present_mode = wgpu::PresentMode::Fifo;
    surface.configure(&device, &config);
    SURFACE.set(Some(Surface { surface, device, queue, config }));
    0
}

/// The layer is now `width` × `height` pixels.
#[no_mangle]
pub extern "C" fn pxl_surface_resize(width: u32, height: u32) {
    let Some(s) = SURFACE.get(|| None) else { return };
    s.config.width = width.max(1);
    s.config.height = height.max(1);
    s.surface.configure(&s.device, &s.config);
}

/// One frame: the surface cleared to the ground's color.
#[no_mangle]
pub extern "C" fn pxl_surface_draw() {
    let Some(s) = SURFACE.get(|| None) else { return };
    let frame = match s.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) => frame,
        // Drawn, and configured again for the next: the layer changed under
        // the swapchain.
        wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
            s.surface.configure(&s.device, &s.config);
            frame
        }
        wgpu::CurrentSurfaceTexture::Outdated => {
            s.surface.configure(&s.device, &s.config);
            return;
        }
        // Backgrounded, or no drawable in time: the next frame asks again.
        _ => return,
    };
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = s.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.28, g: 0.43, b: 0.22, a: 1.0 }),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    s.queue.submit([encoder.finish()]);
    s.queue.present(frame);
}

/// The view is going: the surface and its device with it.
#[no_mangle]
pub extern "C" fn pxl_surface_close() {
    SURFACE.set(None);
}
