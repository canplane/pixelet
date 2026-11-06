//! The finish as the renderer draws it (gpu/finish.rs): its module as assembled, onto the
//! canvas alone, a preview's, and onto the canvas and the picture in one pass, the world's;
//! and a frame not drawn whole, the picture shown first, then its claimed tiles finished again.

mod gpu;

use gpu::{bytes, read_buffer, Gpu};
/// The screen's tiles' side, in the world's pixels.
use pxl_engine::screen::TILE as SIDE;
use pxl_render::frame::shader::record::{src, SHADER_SCREEN_FINISH, SHADER_SCREEN_SHOW};
use pxl_render::frame::shader::word::{frame, FRAME_FLOATS};

/// The world's pixels and the canvas's: the canvas twice the world, as a share of the
/// display's pixels draws it.
const WORLD: [u32; 2] = [32, 24];
const CANVAS: [u32; 2] = [64, 48];
const CANVAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

struct Finish<'a> {
    gpu: &'a Gpu,
    world: wgpu::TextureView,
    depth: wgpu::TextureView,
    frame: wgpu::Buffer,
    changed_at: wgpu::Buffer,
    tiles: wgpu::Buffer,
}

fn texture(
    gpu: &Gpu,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn module(gpu: &Gpu, shader: u32) -> wgpu::ShaderModule {
    gpu.device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(src(shader).into()),
        })
}

/// The pipeline of `shader` drawing into `targets` canvases, with the screen's depth where
/// `depth`.
fn pipeline(gpu: &Gpu, shader: u32, targets: usize, depth: bool) -> wgpu::RenderPipeline {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = module(gpu, shader);
    let target = Some(wgpu::ColorTargetState {
        format: CANVAS_FORMAT,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    });
    let colors = vec![target; targets];
    let p = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: depth.then(|| wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &colors,
            }),
            multiview_mask: None,
            cache: None,
        });
    if let Some(e) = pollster::block_on(scope.pop()) {
        panic!("shader {shader} with {targets} targets refused: {e}");
    }
    p
}

impl<'a> Finish<'a> {
    /// A world of colors that differ texel to texel and a depth cleared to one value.
    fn new(gpu: &'a Gpu) -> Finish<'a> {
        let use_ = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let world = texture(gpu, WORLD, wgpu::TextureFormat::Rgba16Float, use_);
        // Eighths, each a half float exactly: 0x3c00 is 1, and an eighth's exponent is 3
        // below, as each halving takes one.
        let half = |eighths: u32| -> u16 {
            match eighths {
                0 => 0,
                n => {
                    let e = 31 - n.leading_zeros();
                    let m = (n << (10 - e)) & 0x3ff;
                    (((15 + e - 3) << 10) | m) as u16
                }
            }
        };
        let texels: Vec<u16> = (0..WORLD[0] * WORLD[1])
            .flat_map(|k| {
                let (x, y) = (k % WORLD[0], k / WORLD[0]);
                [half(x % 9), half(y % 9), half((x + y) % 7), half(8)]
            })
            .collect();
        let words: Vec<u32> = texels
            .chunks(2)
            .map(|p| u32::from(p[0]) | u32::from(p[1]) << 16)
            .collect();
        gpu.queue.write_texture(
            world.as_image_copy(),
            bytes(&words),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WORLD[0] * 8),
                rows_per_image: None,
            },
            world.size(),
        );
        let depth_use =
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT;
        let depth = texture(gpu, WORLD, wgpu::TextureFormat::Depth32Float, depth_use);
        let depth = depth.create_view(&Default::default());
        let mut e = gpu.device.create_command_encoder(&Default::default());
        e.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.25),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        gpu.queue.submit([e.finish()]);
        let mut words = [0.0f32; FRAME_FLOATS];
        words[frame::VIEWPORT] = WORLD[0] as f32;
        words[frame::VIEWPORT + 1] = WORLD[1] as f32;
        words[frame::SCREEN] = CANVAS[0] as f32;
        words[frame::SCREEN + 1] = CANVAS[1] as f32;
        words[frame::VIGNETTE] = 0.5;
        let across = WORLD.map(|v| v.div_ceil(SIDE));
        // Every tile changed in frame 3 but the first, changed in frame 9, the frame's.
        let mut changed = vec![3u32; (across[0] * across[1]) as usize];
        changed[0] = 9;
        let uniform = wgpu::BufferUsages::UNIFORM;
        Finish {
            gpu,
            world: world.create_view(&Default::default()),
            depth,
            frame: gpu.buffer(&words.map(f32::to_bits), uniform),
            changed_at: gpu.buffer(&changed, wgpu::BufferUsages::STORAGE),
            tiles: gpu.buffer(&[0; 8], uniform),
        }
    }

    /// The tiles' uniform: every tile, or those changed fewer than `claimed_for` frames
    /// before frame 9.
    fn claim(&self, claimed_for: u32) {
        let across = WORLD.map(|v| v.div_ceil(SIDE));
        let grow = WORLD[0] as f32 / CANVAS[0] as f32;
        let words = [
            across[0],
            across[1],
            claimed_for,
            9,
            (WORLD[0] as f32).to_bits(),
            (WORLD[1] as f32).to_bits(),
            grow.to_bits(),
            (SIDE as f32).to_bits(),
        ];
        self.gpu.queue.write_buffer(&self.tiles, 0, bytes(&words));
    }

    /// The finish of `p` onto `canvases`, each loaded where `load`, cleared else.
    fn draw(&self, p: &wgpu::RenderPipeline, canvases: &[&wgpu::TextureView], load: bool) {
        let g = &self.gpu.device;
        let group = g.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &p.get_bind_group_layout(0),
            entries: &[
                entry(0, self.frame.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(&self.world)),
                entry(2, wgpu::BindingResource::TextureView(&self.depth)),
                entry(30, self.changed_at.as_entire_binding()),
                entry(31, self.tiles.as_entire_binding()),
            ],
        });
        let screen_depth = texture(
            self.gpu,
            CANVAS,
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let screen_depth = screen_depth.create_view(&Default::default());
        let colors: Vec<_> = canvases.iter().map(|v| attachment(v, load)).collect();
        let mut e = g.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &colors,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &screen_depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let across = WORLD.map(|v| v.div_ceil(SIDE));
            pass.set_pipeline(p);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..6, 0..across[0] * across[1]);
        }
        self.gpu.queue.submit([e.finish()]);
    }

    /// `picture` shown whole on `canvas` (screen_show.wgsl).
    fn show(
        &self,
        p: &wgpu::RenderPipeline,
        picture: &wgpu::TextureView,
        canvas: &wgpu::TextureView,
    ) {
        let g = &self.gpu.device;
        let group = g.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &p.get_bind_group_layout(0),
            entries: &[entry(1, wgpu::BindingResource::TextureView(picture))],
        });
        let mut e = g.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[attachment(canvas, false)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(p);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.gpu.queue.submit([e.finish()]);
    }
}

fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

fn attachment(view: &wgpu::TextureView, load: bool) -> Option<wgpu::RenderPassColorAttachment<'_>> {
    Some(wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: if load {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
            },
            store: wgpu::StoreOp::Store,
        },
    })
}

/// A canvas the finish draws into and the test reads.
fn canvas(gpu: &Gpu) -> (wgpu::Texture, wgpu::TextureView) {
    let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_SRC;
    let t = texture(gpu, CANVAS, CANVAS_FORMAT, usage);
    let v = t.create_view(&Default::default());
    (t, v)
}

/// A canvas's texels, a word each.
fn read(gpu: &Gpu, t: &wgpu::Texture) -> Vec<u32> {
    let row = CANVAS[0] * 4;
    let out = gpu.buffer(
        &vec![0; (CANVAS[0] * CANVAS[1]) as usize],
        wgpu::BufferUsages::empty(),
    );
    let mut e = gpu.device.create_command_encoder(&Default::default());
    e.copy_texture_to_buffer(
        t.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &out,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        t.size(),
    );
    gpu.queue.submit([e.finish()]);
    read_buffer(&gpu.device, &gpu.queue, &out)
}

#[test]
fn the_world_s_canvas_and_picture_are_the_preview_s_one_canvas() {
    // A canvas's row is read whole, as a copy takes rows of 256 bytes.
    const _: () = assert!((CANVAS[0] * 4).is_multiple_of(256));
    let Some(gpu) = Gpu::new() else {
        eprintln!("skipped: no adapter");
        return;
    };
    let f = Finish::new(&gpu);
    let one = pipeline(&gpu, SHADER_SCREEN_FINISH, 1, true);
    let two = pipeline(&gpu, SHADER_SCREEN_FINISH, 2, true);
    f.claim(u32::MAX);
    let (alone, alone_view) = canvas(&gpu);
    f.draw(&one, &[&alone_view], false);
    let (shown, shown_view) = canvas(&gpu);
    let (picture, picture_view) = canvas(&gpu);
    f.draw(&two, &[&shown_view, &picture_view], false);
    let alone = read(&gpu, &alone);
    assert!(
        alone.iter().any(|&w| w != alone[0]),
        "the world drawn differs pixel to pixel"
    );
    assert_eq!(
        read(&gpu, &shown),
        alone,
        "the canvas as the canvas alone holds it"
    );
    assert_eq!(read(&gpu, &picture), alone, "the picture as the canvas");

    // A frame not drawn whole: the picture shown on a new canvas, then the first tile alone,
    // the one changed this frame, finished into both. Both hold the whole frame's.
    let show = pipeline(&gpu, SHADER_SCREEN_SHOW, 1, false);
    let (next, next_view) = canvas(&gpu);
    f.show(&show, &picture_view, &next_view);
    f.claim(1);
    f.draw(&two, &[&next_view, &picture_view], true);
    assert_eq!(
        read(&gpu, &next),
        alone,
        "the canvas of a frame not drawn whole"
    );
    assert_eq!(read(&gpu, &picture), alone, "its picture");
}
